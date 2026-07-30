//! Median threshold bitmap alignment for handheld HDR exposures.
//!
//! Ward's method, from "Fast, Robust Image Registration for Compositing High
//! Dynamic Range Photographs from Hand-held Exposures" (2003). Each exposure is
//! reduced to one bit per pixel by thresholding at its own median, and the
//! offset between two exposures is found by counting disagreements down an
//! image pyramid.
//!
//! The median is a rank, so it survives any monotonic change of exposure: the
//! same scene gives the same bitmap however it was metered. That is what lets
//! the frames be registered before the camera response is known, which is
//! otherwise circular, since solving the response needs them registered.
//!
//! Only integer translation is recovered. Ward reports around one handheld
//! sequence in ten wanting a rotation this cannot give it.
//!
//! ```
//! use mtb_align::{Gray, Options, Shift, shift};
//!
//! // A scene with detail at two scales, so every level of the pyramid has
//! // something to lock onto. A real photograph has this for free.
//! fn hash(x: i64, y: i64) -> u32 {
//!     let mut mixed = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
//!         ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
//!     mixed ^= mixed >> 29;
//!     (mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9) >> 56) as u32
//! }
//! let scene = |x, y| ((2 * hash(x / 32, y / 32) + hash(x / 4, y / 4)) / 3) as u8;
//!
//! // Two views of it, taken 5 across and 3 up from each other.
//! let read = |dx: i64, dy: i64| {
//!     let samples = (0..256 * 256).map(|i| scene(i % 256 + dx, i / 256 + dy)).collect();
//!     Gray::from_vec(samples, 256, 256)
//! };
//!
//! let options = Options { bits: 4, ..Options::default() };
//! let found = shift(&read(0, 0), &read(5, -3), &options);
//!
//! assert_eq!(found, Shift::new(5, -3));
//! ```
//!
//! [`align_stack`] does the same for a whole bracket, and [`common_crop`] gives
//! the region every frame still covers once they have been moved.

mod align;
mod bitmap;
mod difference;
mod geometry;
mod gray;
mod shrink;
mod strips;
mod threshold;

pub use align::{Options, align_stack, shift};
pub use bitmap::Bitmap;
pub use difference::disagreement;
pub use geometry::{Rect, Shift, common_crop};
pub use gray::Gray;
pub use shrink::{Shrink, shrink2};
pub use threshold::{Bitmaps, Percentile, compute_bitmaps, threshold};
