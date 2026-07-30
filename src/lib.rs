//! Median threshold bitmap alignment for handheld HDR exposures.

mod bitmap;
mod gray;
mod threshold;

pub use bitmap::Bitmap;
pub use gray::Gray;
pub use threshold::{Percentile, compute_bitmaps, threshold};
