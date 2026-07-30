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

Each exposure is thresholded at its own median, which is a rank and so survives
any monotonic change of exposure: the same scene gives the same bitmap at any
shutter speed. That is what lets the frames be registered *before* the response
curve is solved, which otherwise needs them registered first. Pixels within a
few levels of the threshold are noise rather than scene, and are excluded.
Comparison is then an XOR down an image pyramid, a pixel of search per level.

Translation only. Ward reports about one sequence in ten needing rotation that
this cannot give it.

```sh
cargo run --example align                 # a synthetic five-frame bracket
cargo run --release --example bench       # throughput at sensor resolution
```

Ward's table calls the pyramid step a subsample and his text calls it a filter;
`Shrink::Average` takes the second reading and is the default, `Shrink::Subsample`
takes the first and reproduces OpenCV. `Options::opencv()` selects the whole set
of its conventions, which is what the fixture tests are recorded against.

The `rayon` feature parallelises the row passes. A 51 MP pair aligns in 211 ms
on one core and 50 ms across 48, so it scales to about four cores' worth and
then runs out of memory bandwidth — a threshold bitmap is one bit per pixel, and
there is very little arithmetic per byte to hide the loads behind.

## Acknowledgements

Ward, G. (2003). "Fast, Robust Image Registration for Compositing High Dynamic
Range Photographs from Hand-held Exposures." *Journal of Graphics Tools*, 8(2),
17-30. [doi:10.1080/10867651.2003.10487583](https://doi.org/10.1080/10867651.2003.10487583)

Reinhard, E., Ward, G., Pattanaik, S. and Debevec, P. *High Dynamic Range
Imaging*. Morgan Kaufmann. The book chapter version, with more context.

Evangelidis, G. D. and Psarakis, E. Z. (2008). "Parametric Image Alignment Using
Enhanced Correlation Coefficient Maximization." *IEEE TPAMI*, 30(10), 1858-1865.
[doi:10.1109/TPAMI.2008.113](https://doi.org/10.1109/TPAMI.2008.113) — where to
go when the sequence does need rotation. OpenCV implements it as
`findTransformECC`.

OpenCV's `AlignMTB`, whose conventions this follows where the paper is silent or
ambiguous, and whose output the fixture tests are recorded from.

Hugin's `align_image_stack` and pfstools also implement this, and both are GPL.
Neither was consulted.
