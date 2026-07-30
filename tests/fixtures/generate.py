#!/usr/bin/env python3
"""Regenerate the OpenCV oracle fixtures.

    uv run --with opencv-python-headless --with numpy tests/fixtures/generate.py

mtb-align's conventions are chosen to match OpenCV's AlignMTB, so any divergence
here is a bug on our side rather than a difference of opinion. This records
OpenCV's answers instead of depending on it at test time.

All three of the interesting methods are public, so all three are recorded:
computeBitmaps and shiftMat pin the pieces, calculateShift pins the whole
search. When only the last of them disagrees, the conventions are right and the
argmin tipped a different way on a near-tie; when the first two disagree, the
conventions are wrong. That is the reason for recording the pieces separately.

Ward's paper is the specification, not OpenCV, and the two differ in one place
that matters: `AlignMTB` subsamples the pyramid where Ward filters it down. The
fixtures are therefore generated against `Options::opencv()`, not the defaults.
"""

import pathlib
import sys

import cv2
import numpy as np

MAX_BITS = 6
EXCLUDE_RANGE = 4

# Small enough to keep the fixture readable, odd enough to catch the padding.
BITMAP_CASES = [(16, 16), (13, 9), (40, 5), (8, 8), (65, 3)]

SHIFT_CASES = [(11, 7, 0, 0), (11, 7, 3, 2), (11, 7, -4, -1), (11, 7, 10, 6), (70, 5, -1, 4)]

# calculateShift needs enough image to have something to say. The bit count is
# cut to suit the size: six bits of a 96x72 frame would leave a coarsest level
# of three pixels by two, which is a coin toss rather than a convention.
ALIGN_CASES = [(96, 72, 4, (5, -3)), (96, 72, 4, (-7, 6)), (64, 64, 3, (2, 2))]

# Odd sizes, where the dropped fringe row or column shows up.
SHRINK_CASES = [(8, 8), (9, 7), (1, 1), (65, 3)]


def downsample(src):
    """OpenCV's `AlignMTB::downsample`, transcribed rather than recorded.

    It is a protected member, so unlike everything else in this file it cannot
    be called; this is the C++ written out in numpy:

        for(int y = 0; y < dst.rows; y++) {
            uchar *ptr = src_ptr;
            for(int x = 0; x < dst.cols; x++) { dst_ptr[0] = ptr[0]; dst_ptr++; ptr += 2; }
            src_ptr += offset;
        }

    with `dst = Mat(src.rows / 2, src.cols / 2, ...)`. Recording it pins the
    convention against a fixture instead of only against a comment. The
    end-to-end `align` cases cannot do that job: the search converges on the
    same answer whichever corner of each block is taken, which is a good
    property of the algorithm and a useless one for an oracle.
    """
    height, width = src.shape
    return np.ascontiguousarray(src[: height // 2 * 2 : 2, : width // 2 * 2 : 2])


def scene(width, height, seed):
    """A multi-scale random field, so alignment has something to lock onto at
    every level rather than only the finest.

    The per-pixel octave is what makes these fixtures able to tell the pyramid
    conventions apart. Without it the field is smooth enough that taking the
    other pixel of each 2x2 block changes almost nothing, and an implementation
    that subsampled the wrong corner would agree with OpenCV anyway.
    """
    rng = np.random.default_rng(seed)
    total = np.zeros((height, width), np.float32)

    for cell, weight in ((16, 0.45), (4, 0.25), (2, 0.15), (1, 0.15)):
        coarse = rng.random((max(height // cell, 2), max(width // cell, 2)), dtype=np.float32)
        total += weight * cv2.resize(coarse, (width, height), interpolation=cv2.INTER_CUBIC)

    return np.clip(total * 255.0, 0, 255).astype(np.uint8)


def windows(width, height, offset, seed):
    """Two views of one scene a known distance apart, so both are real images
    rather than one padded copy of the other."""
    margin = 32
    whole = scene(width + 2 * margin, height + 2 * margin, seed)
    dx, dy = offset

    def view(ox, oy):
        return np.ascontiguousarray(
            whole[margin + oy : margin + oy + height, margin + ox : margin + ox + width]
        )

    return view(0, 0), view(dx, dy)


def hexed(array):
    return array.tobytes().hex()


def bits(mask):
    return "".join("1" if value else "0" for value in mask.flatten())


def main():
    aligner = cv2.createAlignMTB(max_bits=MAX_BITS, exclude_range=EXCLUDE_RANGE, cut=True)
    out = []

    for seed, (width, height) in enumerate(BITMAP_CASES):
        # The last case is a uniform white frame: nothing is above the top of
        # the range, so the median runs off the end at 256 and both bitmaps come
        # back empty. It is the reason the threshold is not a u8.
        source = (
            np.full((height, width), 255, np.uint8)
            if (width, height) == (8, 8)
            else scene(width, height, seed)
        )
        tb, eb = aligner.computeBitmaps(source)
        out.append(
            f"CASE bitmaps {width} {height} {EXCLUDE_RANGE}\n"
            f"IN {hexed(source)}\nTB {bits(tb)}\nEB {bits(eb)}"
        )

    for seed, (width, height, dx, dy) in enumerate(SHIFT_CASES, start=100):
        source = scene(width, height, seed)
        # OpenCV computes the copied region as cols - abs(shift.x) and builds a
        # Rect from it, so a shift past the edge of the image is not something
        # it can be asked for. Those are covered by the unit tests instead.
        moved = aligner.shiftMat(source, (dx, dy))
        out.append(
            f"CASE shift {width} {height} {dx} {dy}\n"
            f"IN {hexed(source)}\nOUT {hexed(moved)}"
        )

    for seed, (width, height) in enumerate(SHRINK_CASES, start=150):
        source = scene(width, height, seed)
        shrunk = downsample(source)
        out.append(
            f"CASE shrink {width} {height} {shrunk.shape[1]} {shrunk.shape[0]}\n"
            f"IN {hexed(source)}\nOUT {hexed(shrunk)}"
        )

    for seed, (width, height, bits_, offset) in enumerate(ALIGN_CASES, start=200):
        reference, target = windows(width, height, offset, seed)
        sized = cv2.createAlignMTB(max_bits=bits_, exclude_range=EXCLUDE_RANGE, cut=True)
        dx, dy = sized.calculateShift(reference, target)
        out.append(
            f"CASE align {width} {height} {bits_} {EXCLUDE_RANGE}\n"
            f"REF {hexed(reference)}\nTGT {hexed(target)}\nOUT {dx} {dy}"
        )
        if (dx, dy) != offset:
            print(
                f"note: OpenCV read {(dx, dy)} where the scene was built at {offset}",
                file=sys.stderr,
            )

    destination = pathlib.Path(__file__).parent / "opencv.txt"
    destination.write_text(
        f"# generated by generate.py against OpenCV {cv2.__version__}\n" + "\n".join(out) + "\n"
    )

    print(f"{len(out)} cases -> {destination}")


if __name__ == "__main__":
    main()
