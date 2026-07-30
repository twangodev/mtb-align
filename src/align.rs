use crate::{
    Bitmaps, Error, Gray, Percentile, Shift, Shrink, compute_bitmaps, disagreement, shrink2,
};

/// Ward's nine, centre first: he and OpenCV keep the first to beat the running
/// best, so `(-1, -1)` wins every tie. Centre first ties to zero instead.
const CANDIDATES: [(i32, i32); 9] = [
    (0, 0),
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// How the search is run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Ward's `shift_bits`, reaching `±(2^bits - 1)` pixels. Six is his figure for
    /// the three megapixel frames of 2003; the same shake spans four times the
    /// pixels at fifty, which wants eight. Each bit is another halving, and the
    /// coarsest level guesses with only ±1 to correct it after, so small images
    /// want fewer, not more.
    pub bits: u32,
    /// How far from the threshold a sample must sit before it is trusted.
    pub tolerance: u8,
    /// A frame more than half clipped puts the median past the top of the range
    /// and empties the bitmap; Ward's 17th or 83rd moves the cut back among the
    /// samples that survived.
    pub percentile: Percentile,
    pub shrink: Shrink,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            bits: 6,
            tolerance: 4,
            percentile: Percentile::MEDIAN,
            shrink: Shrink::Average,
        }
    }
}

impl Options {
    /// The conventions OpenCV's `AlignMTB` uses, for comparing against it.
    pub fn opencv() -> Self {
        Self {
            shrink: Shrink::Subsample,
            ..Self::default()
        }
    }
}

/// How far `target` has to move to sit on top of `reference`.
pub fn shift(reference: &Gray, target: &Gray, options: &Options) -> Result<Shift, Error> {
    let size = |gray: &Gray| (gray.width(), gray.height());
    if size(reference) != size(target) {
        return Err(Error::SizeMismatch {
            reference: size(reference),
            target: size(target),
        });
    }

    let levels = shrinks(reference.width(), reference.height(), options.bits);
    let reference_pyramid = pyramid(reference, levels, options.shrink);
    let target_pyramid = pyramid(target, levels, options.shrink);

    // Ward recurses; coarsest-first says the same and holds one level at a time.
    Ok((0..=levels).rev().fold(Shift::ZERO, |coarser, level| {
        let bitmaps = |pyramid: &[Gray]| {
            compute_bitmaps(&pyramid[level], options.percentile, options.tolerance)
        };

        // Twice the resolution, so twice the pixels for the same displacement.
        refine(
            &bitmaps(&reference_pyramid),
            &bitmaps(&target_pyramid),
            coarser.doubled(),
        )
    }))
}

/// How far each exposure has to move to sit on `images[reference]`, measured
/// between *adjacent* exposures and accumulated: one percentile fits two
/// neighbours well and the ends of a bracket badly.
///
/// So `images` has to be in exposure order, which is not always the order the
/// camera wrote the files in.
pub fn align_stack(
    images: &[Gray],
    reference: usize,
    options: &Options,
) -> Result<Vec<Shift>, Error> {
    if reference >= images.len() {
        return Err(Error::NoSuchReference {
            reference,
            exposures: images.len(),
        });
    }

    let mut shifts = vec![Shift::ZERO; images.len()];

    // Outwards from the reference, each frame placed against the last one placed.
    for index in (0..reference).rev() {
        let step = shift(&images[index + 1], &images[index], options)?;
        shifts[index] = shifts[index + 1].offset(step.x, step.y);
    }

    for index in reference + 1..images.len() {
        let step = shift(&images[index - 1], &images[index], options)?;
        shifts[index] = shifts[index - 1].offset(step.x, step.y);
    }

    Ok(shifts)
}

/// How many times the image is halved. Ward descends `shift_bits` times whatever
/// the size; this is OpenCV's cap on the longest side, plus one on the shortest.
fn shrinks(width: usize, height: usize, bits: u32) -> usize {
    if width == 0 || height == 0 {
        return 0;
    }

    let longest = width.max(height).ilog2().saturating_sub(1);
    let shortest = width.min(height).ilog2();

    bits.saturating_sub(1).min(longest).min(shortest) as usize
}

fn pyramid(gray: &Gray, levels: usize, shrink: Shrink) -> Vec<Gray> {
    let mut pyramid = Vec::with_capacity(levels + 1);
    pyramid.push(gray.clone());

    for _ in 0..levels {
        let coarser = shrink2(pyramid.last().expect("just pushed"), shrink);
        pyramid.push(coarser);
    }

    pyramid
}

/// The best of the nine offsets within a pixel of `around`.
fn refine(reference: &Bitmaps, target: &Bitmaps, around: Shift) -> Shift {
    let mut best = around;
    let mut least = u64::MAX;

    for (x, y) in CANDIDATES {
        let candidate = around.offset(x, y);
        let error = disagreement(reference, target, candidate);

        if error < least {
            least = error;
            best = candidate;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    // Recovering real offsets lives in `tests/recovery.rs`.
    #[test]
    fn the_defaults_are_wards_recommendations() {
        let options = Options::default();

        assert_eq!(options.bits, 6);
        assert_eq!(options.tolerance, 4);
        assert_eq!(options.percentile, Percentile::MEDIAN);
        assert_eq!(options.shrink, Shrink::Average);
        assert_eq!(Options::opencv().shrink, Shrink::Subsample);
    }

    #[test]
    fn the_level_count_follows_the_longest_side_until_the_bits_run_out() {
        assert_eq!(shrinks(8256, 6192, 6), 5);
        assert_eq!(shrinks(4, 4, 6), 1);
        assert_eq!(shrinks(3, 3, 6), 0);
        assert_eq!(shrinks(2, 2, 6), 0);
        assert_eq!(shrinks(1, 1, 6), 0);
        assert_eq!(shrinks(1024, 1024, 3), 2);
    }

    #[test]
    fn an_extreme_aspect_ratio_stops_before_a_level_vanishes() {
        assert_eq!(shrinks(1000, 3, 6), 1);
        assert_eq!(shrinks(1000, 1, 6), 0);
    }

    #[test]
    fn a_pyramid_keeps_the_source_and_one_plane_per_halving() {
        let levels = pyramid(
            &Gray::from_vec(vec![0; 64 * 64], 64, 64),
            3,
            Shrink::Average,
        );

        assert_eq!(levels.len(), 4);
        assert_eq!((levels[0].width(), levels[0].height()), (64, 64));
        assert_eq!((levels[3].width(), levels[3].height()), (8, 8));
    }

    /// Every pixel is excluded, so all nine candidates tie at zero.
    #[test]
    fn an_exposure_with_no_usable_signal_reports_no_shift() {
        let flat = Gray::from_vec(vec![128; 160 * 120], 160, 120);

        assert_eq!(shift(&flat, &flat, &Options::default()), Ok(Shift::ZERO));
    }

    #[test]
    fn a_single_pixel_image_reports_no_shift() {
        let dot = Gray::from_vec(vec![200], 1, 1);

        assert_eq!(shift(&dot, &dot, &Options::default()), Ok(Shift::ZERO));
    }

    #[test]
    fn aligning_mismatched_sizes_is_an_error() {
        let small = Gray::from_vec(vec![0; 16], 4, 4);
        let large = Gray::from_vec(vec![0; 25], 5, 5);

        assert_eq!(
            shift(&small, &large, &Options::default()),
            Err(Error::SizeMismatch {
                reference: (4, 4),
                target: (5, 5)
            })
        );
    }

    #[test]
    fn a_reference_outside_the_stack_is_an_error() {
        let one = [Gray::from_vec(vec![0; 4], 2, 2)];

        assert_eq!(
            align_stack(&one, 3, &Options::default()),
            Err(Error::NoSuchReference {
                reference: 3,
                exposures: 1
            })
        );
    }
}
