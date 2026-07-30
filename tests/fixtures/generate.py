#!/usr/bin/env python3
"""Regenerate the OpenCV oracle fixtures.

    uv run --with opencv-python-headless --with numpy tests/fixtures/generate.py

Records OpenCV's answers rather than depending on it at test time. computeBitmaps
and shiftMat pin the pieces, calculateShift pins the search; separating them says
whether a failure is a wrong convention or a near-tie tipping the other way.

Ward's paper is the specification, not OpenCV, and they differ in one place that
matters: AlignMTB subsamples the pyramid where Ward filters it down. So these are
generated against `Options::opencv()`, not the defaults.
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

# Bits cut to suit the size: six of a 96x72 frame leaves a coarsest level of
# three pixels by two, which is a coin toss rather than a convention.
ALIGN_CASES = [(96, 72, 4, (5, -3)), (96, 72, 4, (-7, 6)), (64, 64, 3, (2, 2))]

# Odd sizes, where the dropped fringe row or column shows up.
SHRINK_CASES = [(8, 8), (9, 7), (1, 1), (65, 3)]


def downsample(src):
    """OpenCV's `AlignMTB::downsample`, transcribed rather than recorded: it is
    protected, so unlike everything else here it cannot be called. The C++ takes
    `src[2y][2x]` into a `Mat(rows / 2, cols / 2)`.

    The end-to-end `align` cases cannot pin this, because the search converges on
    the same answer whichever corner of each block is taken.
    """
    height, width = src.shape
    return np.ascontiguousarray(src[: height // 2 * 2 : 2, : width // 2 * 2 : 2])


def scene(width, height, seed):
    """A multi-scale random field. The per-pixel octave is what lets these
    fixtures tell the pyramid conventions apart: without it the field is smooth
    enough that subsampling the wrong corner still agrees with OpenCV.
    """
    rng = np.random.default_rng(seed)
    total = np.zeros((height, width), np.float32)

    for cell, weight in ((16, 0.45), (4, 0.25), (2, 0.15), (1, 0.15)):
        coarse = rng.random((max(height // cell, 2), max(width // cell, 2)), dtype=np.float32)
        total += weight * cv2.resize(coarse, (width, height), interpolation=cv2.INTER_CUBIC)

    return np.clip(total * 255.0, 0, 255).astype(np.uint8)


def windows(width, height, offset, seed):
    """Two views of one scene a known distance apart, both real images."""
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
        # A uniform white frame runs the median off the end at 256, which is
        # why the threshold is not a u8.
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
        # OpenCV builds a Rect from cols - abs(shift.x), so it cannot be asked
        # for a shift past the edge. Unit tests cover those.
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
