//! Median threshold bitmap alignment for handheld HDR exposures.
//!
//! Ward (2003). Thresholding each exposure at its own median gives a bitmap that
//! survives any monotonic change of exposure, so frames can be registered before
//! the camera response is solved. Integer translation only.
//!
//! ```
//! use mtb_align::{Gray, Options, Shift, shift};
//!
//! let scene = |x: i64, y: i64| (((x / 16) * 61) ^ ((y / 16) * 37)) as u8;
//! let read = |dx: i64, dy: i64| {
//!     let samples = (0..256 * 256).map(|i| scene(i % 256 + dx, i / 256 + dy)).collect();
//!     Gray::from_vec(samples, 256, 256)
//! };
//! let options = Options { bits: 4, ..Options::default() };
//!
//! assert_eq!(shift(&read(0, 0), &read(5, -3), &options), Ok(Shift::new(5, -3)));
//! ```
//!
//! [`align_stack`] does a whole bracket, [`common_crop`] the region left over.

mod align;
mod bitmap;
mod difference;
mod error;
mod geometry;
mod gray;
mod shrink;
mod strips;
mod threshold;

pub use align::{Options, align_stack, shift};
pub use bitmap::Bitmap;
pub use difference::disagreement;
pub use error::Error;
pub use geometry::{Rect, Shift, common_crop};
pub use gray::Gray;
pub use shrink::{Shrink, shrink2};
pub use threshold::{Bitmaps, Percentile, compute_bitmaps, threshold};
