# mtb-align

[![crates.io](https://img.shields.io/crates/v/mtb-align.svg)](https://crates.io/crates/mtb-align)
[![docs.rs](https://img.shields.io/docsrs/mtb-align)](https://docs.rs/mtb-align)
[![build](https://img.shields.io/github/actions/workflow/status/twangodev/mtb-align/rust.yml?branch=main)](https://github.com/twangodev/mtb-align/actions/workflows/rust.yml)
[![license](https://img.shields.io/crates/l/mtb-align)](LICENSE)

Median threshold bitmap alignment for handheld HDR exposures.

Integer translation, recovered without knowing the camera response.

```rust
use mtb_align::{Gray, Options, align_stack, common_crop};

let exposures: Vec<Gray> = frames
    .iter()
    .map(|frame| Gray::from_rgb(frame.as_raw(), width, height))
    .collect();

let shifts = align_stack(&exposures, exposures.len() / 2, &Options::default());
let crop = common_crop(&shifts, width, height);
```

Thresholding at the median gives a bitmap that survives any monotonic change of
exposure, so frames can be registered before the response curve is solved, which
otherwise needs them registered. Pixels near the threshold are noise and get
excluded; comparison is an XOR down a pyramid, a pixel of search per level.

No rotation: Ward measures 84% success, with 10% failing for want of it. A frame
more than half clipped has no median worth thresholding at, and wants a lower
`Percentile` instead.

```sh
cargo run --example align                 # a synthetic five-frame bracket
cargo run --release --example bench       # throughput at sensor resolution
```

`Shrink::Average` takes Ward's "filter it down" and is the default;
`Options::opencv()` picks `AlignMTB`'s conventions, which the fixtures record.

The `rayon` feature parallelises the row passes. A 51 MP pair aligns in 211 ms
on one core and 50 ms across 48. Almost all of that is building the bitmaps —
streaming the 8-bit pyramid — rather than searching them; the nine XOR passes
over a 1-bit image are 5% of the time.

## Acknowledgements

Ward, G. (2003). "Fast, Robust Image Registration for Compositing High Dynamic
Range Photographs from Hand-held Exposures." *Journal of Graphics Tools*, 8(2),
17-30. [doi:10.1080/10867651.2003.10487583](https://doi.org/10.1080/10867651.2003.10487583)

Reinhard, Ward, Pattanaik and Debevec, *High Dynamic Range Imaging* (Morgan
Kaufmann), for the book chapter version.

Evangelidis, G. D. and Psarakis, E. Z. (2008). "Parametric Image Alignment Using
Enhanced Correlation Coefficient Maximization." *IEEE TPAMI*, 30(10), 1858-1865.
[doi:10.1109/TPAMI.2008.113](https://doi.org/10.1109/TPAMI.2008.113) — for the
sequences that do need rotation. OpenCV has it as `findTransformECC`.

OpenCV's `AlignMTB`, whose conventions this follows and whose output the fixture
tests are recorded from.
