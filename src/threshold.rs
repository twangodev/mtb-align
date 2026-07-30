use crate::{Bitmap, Gray};

/// Distinct sample values an 8-bit image can take.
const LEVELS: usize = 256;

/// Which population split the threshold bitmap is cut at. Ward uses the median,
/// falling back to the 17th or 83rd for exposures too dark or light for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Percentile(u8);

impl Percentile {
    pub const MEDIAN: Self = Self(50);

    pub const SEVENTEENTH: Self = Self(17);
    pub const EIGHTY_THIRD: Self = Self(83);

    /// Panics unless `percent <= 100`.
    pub fn new(percent: u8) -> Self {
        assert!(percent <= 100, "{percent} is not a percentile");
        Self(percent)
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

impl Default for Percentile {
    fn default() -> Self {
        Self::MEDIAN
    }
}

/// The sample value the bitmaps are cut at. A `u16` because it can land one past
/// the range: an image of nothing but 255s gives 256, as it does in OpenCV.
pub fn threshold(gray: &Gray, percentile: Percentile) -> u16 {
    let histogram = histogram(gray.as_slice());

    // OpenCV's scan: the sum is tested before each bin, so the answer is one past
    // the bin that tipped it.
    let target = gray.as_slice().len() as u64 * percentile.percent() as u64 / 100;
    let mut sum = 0;
    let mut level = 0;
    while sum < target && level < LEVELS {
        sum += histogram[level];
        level += 1;
    }

    level as u16
}

/// Counts samples at each of the 256 levels. Counting is commutative, so
/// threads tally a slice each rather than locking one shared histogram.
fn histogram(samples: &[u8]) -> [u64; LEVELS] {
    let tally = |samples: &[u8]| {
        let mut bins = [0u64; LEVELS];
        for &sample in samples {
            bins[sample as usize] += 1;
        }
        bins
    };

    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        // Big enough to amortise a per-slice histogram, small enough not to
        // leave a long tail.
        const CHUNK: usize = 1 << 16;

        if samples.len() > CHUNK {
            return samples.par_chunks(CHUNK).map(tally).reduce(
                || [0; LEVELS],
                |mut carried, bins| {
                    for (total, count) in carried.iter_mut().zip(bins) {
                        *total += count;
                    }
                    carried
                },
            );
        }
    }

    tally(samples)
}

/// One exposure reduced to the two bitmaps the search compares. They travel
/// together: the exclusion bitmap only means anything against its own threshold.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmaps {
    /// Set where a sample is above the threshold.
    pub threshold: Bitmap,
    /// *Clear* within the noise tolerance, where a sample's side is untrustworthy.
    pub exclusion: Bitmap,
}

/// Reduces one exposure to its threshold and exclusion bitmaps.
pub fn compute_bitmaps(gray: &Gray, percentile: Percentile, tolerance: u8) -> Bitmaps {
    let cut = threshold(gray, percentile);
    let tolerance = tolerance as u16;

    let (width, height) = (gray.width(), gray.height());
    let sample = |x, y| gray.sample(x, y) as u16;

    Bitmaps {
        threshold: Bitmap::packed(width, height, |x, y| sample(x, y) > cut),
        exclusion: Bitmap::packed(width, height, |x, y| sample(x, y).abs_diff(cut) > tolerance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One pixel at every level, so the threshold has one place it can land.
    fn ramp() -> Gray {
        Gray::from_vec((0..=255).collect(), 16, 16)
    }

    #[test]
    fn the_median_splits_the_population() {
        assert_eq!(threshold(&ramp(), Percentile::MEDIAN), 128);
    }

    #[test]
    fn a_lower_percentile_cuts_lower() {
        assert_eq!(threshold(&ramp(), Percentile::SEVENTEENTH), 43);
        assert_eq!(threshold(&ramp(), Percentile::EIGHTY_THIRD), 212);
    }

    #[test]
    fn a_uniform_image_pushes_the_threshold_past_every_sample() {
        let white = Gray::from_vec(vec![255; 64], 8, 8);

        assert_eq!(threshold(&white, Percentile::MEDIAN), 256);
    }

    #[test]
    fn the_threshold_bitmap_marks_only_what_is_above() {
        let tb = compute_bitmaps(&ramp(), Percentile::MEDIAN, 4).threshold;

        assert_eq!(tb.count_ones(), 127);
        assert!(!tb.get(128 % 16, 128 / 16), "the threshold itself is below");
        assert!(tb.get(129 % 16, 129 / 16));
    }

    /// Ward zeroes it within the tolerance: here 124 to 132, nine values.
    #[test]
    fn the_exclusion_bitmap_clears_the_band_around_the_threshold() {
        let eb = compute_bitmaps(&ramp(), Percentile::MEDIAN, 4).exclusion;

        assert_eq!(eb.count_ones(), 256 - 9);
        for value in 124..=132usize {
            assert!(
                !eb.get(value % 16, value / 16),
                "{value} should be excluded"
            );
        }
        assert!(eb.get(123 % 16, 123 / 16));
        assert!(eb.get(133 % 16, 133 / 16));
    }

    #[test]
    fn a_zero_tolerance_still_excludes_the_threshold_value() {
        let eb = compute_bitmaps(&ramp(), Percentile::MEDIAN, 0).exclusion;

        assert_eq!(eb.count_ones(), 255);
        assert!(!eb.get(128 % 16, 128 / 16));
    }

    /// The threshold is 256, so every sample is within one step of it.
    #[test]
    fn a_uniform_image_excludes_itself_entirely() {
        let white = Gray::from_vec(vec![255; 64], 8, 8);

        let Bitmaps {
            threshold: tb,
            exclusion: eb,
        } = compute_bitmaps(&white, Percentile::MEDIAN, 4);

        assert_eq!(tb.count_ones(), 0);
        assert_eq!(eb.count_ones(), 0);
    }

    #[test]
    fn the_bitmaps_match_the_image_dimensions() {
        let Bitmaps {
            threshold: tb,
            exclusion: eb,
        } = compute_bitmaps(&Gray::from_vec(vec![7; 12], 4, 3), Percentile::MEDIAN, 4);

        assert_eq!((tb.width(), tb.height()), (4, 3));
        assert_eq!((eb.width(), eb.height()), (4, 3));
    }

    #[test]
    fn a_percentile_reports_what_it_was_built_with() {
        assert_eq!(Percentile::new(17), Percentile::SEVENTEENTH);
        assert_eq!(Percentile::MEDIAN.percent(), 50);
        assert_eq!(Percentile::default(), Percentile::MEDIAN);
    }

    #[test]
    #[should_panic(expected = "101 is not a percentile")]
    fn a_percentile_past_a_hundred_is_rejected() {
        Percentile::new(101);
    }
}
