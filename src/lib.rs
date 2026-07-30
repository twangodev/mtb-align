//! Median threshold bitmap alignment for handheld HDR exposures.

mod align;
mod bitmap;
mod difference;
mod geometry;
mod gray;
mod shrink;
mod threshold;

pub use align::{Options, shift};
pub use bitmap::Bitmap;
pub use difference::disagreement;
pub use geometry::Shift;
pub use gray::Gray;
pub use shrink::{Shrink, shrink2};
pub use threshold::{Bitmaps, Percentile, compute_bitmaps, threshold};
