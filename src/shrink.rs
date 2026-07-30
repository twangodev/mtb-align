use crate::Gray;

/// How a level of the pyramid is halved.
///
/// Ward's table calls `ImageShrink2` a subsample, but his text says the
/// greyscale is "filtered down by a factor of two", and only the bitmaps are
/// singled out as unsafe to subsample. The two readings disagree, so both are
/// available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shrink {
    /// The mean of each 2x2 block. Ward's "filter it down", and the default:
    /// averaging costs one pass and keeps high-frequency detail from aliasing
    /// into the coarse levels, which is one of the failure modes he reports.
    #[default]
    Average,
    /// Every other row and column, which is what OpenCV's `AlignMTB` does.
    Subsample,
}

/// Halves a plane, dropping an odd final row or column.
///
/// Flooring rather than rounding up keeps both modes on the same grid and
/// matches OpenCV. This is deliberately unlike a reconstructing pyramid such as
/// imgpyr's, where every sample has to survive for the collapse to work; here a
/// dropped fringe costs a strip of one image edge and keeps the shift grid a
/// clean power of two.
pub fn shrink2(gray: &Gray, shrink: Shrink) -> Gray {
    let width = gray.width() / 2;
    let height = gray.height() / 2;
    let source = gray.as_slice();
    let stride = gray.width();

    let mut shrunk = Vec::with_capacity(width * height);

    for y in 0..height {
        let top = 2 * y * stride;
        let bottom = top + stride;

        for x in 0..width {
            let left = 2 * x;
            shrunk.push(match shrink {
                Shrink::Average => {
                    // Summed as u16: four saturated samples overflow a u8 three
                    // times over. The +2 rounds the mean to nearest.
                    let total = source[top + left] as u16
                        + source[top + left + 1] as u16
                        + source[bottom + left] as u16
                        + source[bottom + left + 1] as u16;
                    ((total + 2) / 4) as u8
                }
                Shrink::Subsample => source[top + left],
            });
        }
    }

    Gray::from_vec(shrunk, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn counting(width: usize, height: usize) -> Gray {
        Gray::from_vec((0..(width * height) as u8).collect(), width, height)
    }

    #[test]
    fn averaging_takes_the_mean_of_each_block() {
        let shrunk = shrink2(&counting(4, 4), Shrink::Average);

        assert_eq!((shrunk.width(), shrunk.height()), (2, 2));
        assert_eq!(shrunk.as_slice(), &[3, 5, 11, 13]);
    }

    /// Truncating instead would bias every level downward. The bias would
    /// largely cancel out of a median threshold, but rounding is free.
    #[test]
    fn averaging_rounds_to_nearest() {
        let block = Gray::from_vec(vec![1, 2, 3, 4], 2, 2);

        assert_eq!(shrink2(&block, Shrink::Average).as_slice(), &[3]);
    }

    #[test]
    fn subsampling_takes_the_top_left_of_each_block() {
        let shrunk = shrink2(&counting(4, 4), Shrink::Subsample);

        assert_eq!((shrunk.width(), shrunk.height()), (2, 2));
        assert_eq!(shrunk.as_slice(), &[0, 2, 8, 10]);
    }

    #[test]
    fn an_odd_final_row_or_column_is_dropped() {
        let source = counting(5, 3);

        let averaged = shrink2(&source, Shrink::Average);
        assert_eq!((averaged.width(), averaged.height()), (2, 1));
        assert_eq!(averaged.as_slice(), &[3, 5]);

        let subsampled = shrink2(&source, Shrink::Subsample);
        assert_eq!((subsampled.width(), subsampled.height()), (2, 1));
        assert_eq!(subsampled.as_slice(), &[0, 2]);
    }

    /// Averaging must not overflow on the way to the mean, which it would if
    /// the four samples were summed in a `u8`.
    #[test]
    fn a_saturated_block_averages_to_saturation() {
        let block = Gray::from_vec(vec![255; 4], 2, 2);

        assert_eq!(shrink2(&block, Shrink::Average).as_slice(), &[255]);
    }

    #[test]
    fn a_plane_too_small_to_halve_shrinks_away() {
        let shrunk = shrink2(&counting(1, 1), Shrink::Average);

        assert_eq!((shrunk.width(), shrunk.height()), (0, 0));
        assert!(shrunk.as_slice().is_empty());
    }

    proptest! {
        /// The pyramid bookkeeping assumes the two modes stay the same shape as
        /// each other, whatever the input size.
        #[test]
        fn both_modes_halve_to_the_same_size(width in 0usize..40, height in 0usize..40) {
            let source = Gray::from_vec(vec![9; width * height], width, height);

            let averaged = shrink2(&source, Shrink::Average);
            let subsampled = shrink2(&source, Shrink::Subsample);

            prop_assert_eq!(averaged.width(), width / 2);
            prop_assert_eq!(averaged.height(), height / 2);
            prop_assert_eq!(subsampled.width(), width / 2);
            prop_assert_eq!(subsampled.height(), height / 2);
        }

        #[test]
        fn a_constant_plane_survives_either_mode(
            value in 0u8..=255,
            width in 2usize..40,
            height in 2usize..40,
        ) {
            let source = Gray::from_vec(vec![value; width * height], width, height);

            for mode in [Shrink::Average, Shrink::Subsample] {
                for &sample in shrink2(&source, mode).as_slice() {
                    prop_assert_eq!(sample, value);
                }
            }
        }
    }
}
