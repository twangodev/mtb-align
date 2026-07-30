use crate::{Bitmaps, Gray, Percentile, Shift, Shrink, compute_bitmaps, disagreement, shrink2};

/// The nine candidates, in Ward's order, with the centre pulled to the front.
///
/// Ward and OpenCV both scan `x` outermost and keep the first candidate to beat
/// the running best, which leaves `(-1, -1)` winning whenever everything ties.
/// That only happens when an exposure carries no usable signal at all — every
/// pixel excluded as noise — but then it drifts the answer by a pixel per
/// level rather than reporting the honest zero. Trying the centre first costs
/// nothing and makes a tie resolve to "no movement".
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
    /// Ward's `shift_bits`. The search reaches `±(2^bits - 1)` pixels, and he
    /// reports six working well in practice.
    ///
    /// Each bit is one more halving, and the coarsest level is where the offset
    /// is first guessed — every level below it can only correct by a pixel. Six
    /// bits of an eight megapixel frame leaves a coarsest level of a few hundred
    /// pixels, which is plenty; six bits of a thumbnail leaves a handful, and a
    /// guess made there is worth about as much as a coin toss multiplied by 32.
    /// Small images want fewer bits, not more.
    pub bits: u32,
    /// How far from the threshold a sample has to sit before it is trusted.
    pub tolerance: u8,
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
///
/// Panics unless the two images have the same dimensions.
pub fn shift(reference: &Gray, target: &Gray, options: &Options) -> Shift {
    assert!(
        reference.width() == target.width() && reference.height() == target.height(),
        "cannot align a {}x{} exposure against a {}x{} one",
        reference.width(),
        reference.height(),
        target.width(),
        target.height()
    );

    let levels = shrinks(reference.width(), reference.height(), options.bits);
    let reference_pyramid = pyramid(reference, levels, options.shrink);
    let target_pyramid = pyramid(target, levels, options.shrink);

    // Ward recurses down and refines on the way back up. Walking the levels
    // coarsest first says the same thing without building the stack, and keeps
    // one level's bitmaps alive at a time instead of all of them.
    (0..=levels).rev().fold(Shift::ZERO, |coarser, level| {
        let bitmaps = |pyramid: &[Gray]| {
            compute_bitmaps(&pyramid[level], options.percentile, options.tolerance)
        };

        // Each level down doubles the resolution, so the offset agreed at the
        // level above is worth twice as many pixels here.
        refine(
            &bitmaps(&reference_pyramid),
            &bitmaps(&target_pyramid),
            coarser.doubled(),
        )
    })
}

/// How far each exposure in a bracketed sequence has to move to sit on
/// `images[reference]`.
///
/// Offsets are measured between *adjacent* exposures and accumulated, which is
/// how Ward does it and not the same as aligning every frame against the
/// reference directly. Neighbouring frames are the closest in exposure, so one
/// percentile describes both populations well; the ends of a five-stop bracket
/// have far less in common, and asking them to agree on a threshold is asking
/// the most of the algorithm exactly where it has least to work with.
///
/// Panics unless `reference` indexes `images`, or if the images differ in size.
pub fn align_stack(images: &[Gray], reference: usize, options: &Options) -> Vec<Shift> {
    assert!(
        reference < images.len(),
        "no exposure {reference} in a stack of {}",
        images.len()
    );

    let mut shifts = vec![Shift::ZERO; images.len()];

    // Outwards from the reference in both directions, each frame placed against
    // the neighbour that has already been placed.
    for index in (0..reference).rev() {
        let step = shift(&images[index + 1], &images[index], options);
        shifts[index] = shifts[index + 1].offset(step.x, step.y);
    }

    for index in reference + 1..images.len() {
        let step = shift(&images[index - 1], &images[index], options);
        shifts[index] = shifts[index - 1].offset(step.x, step.y);
    }

    shifts
}

/// How many times the image is halved before the search starts.
///
/// Ward's recursion descends `shift_bits` times whatever the image size, which
/// would shrink a small one past a single pixel; the pseudocode quietly assumes
/// there is plenty of resolution to give away. OpenCV caps the descent by the
/// longest side instead, and this follows it, with a second cap on the shortest
/// side so an extreme aspect ratio cannot collapse a level to nothing.
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

    /// Recovering real offsets from a real scene lives in `tests/recovery.rs`;
    /// what is left here is the search's own bookkeeping.
    #[test]
    fn the_defaults_are_wards_recommendations() {
        let options = Options::default();

        assert_eq!(options.bits, 6);
        assert_eq!(options.tolerance, 4);
        assert_eq!(options.percentile, Percentile::MEDIAN);
        assert_eq!(options.shrink, Shrink::Average);
        assert_eq!(Options::opencv().shrink, Shrink::Subsample);
    }

    /// OpenCV's cap, which the level count is written to reproduce.
    #[test]
    fn the_level_count_follows_the_longest_side_until_the_bits_run_out() {
        assert_eq!(shrinks(8256, 6192, 6), 5);
        assert_eq!(shrinks(4, 4, 6), 1);
        assert_eq!(shrinks(3, 3, 6), 0);
        assert_eq!(shrinks(2, 2, 6), 0);
        assert_eq!(shrinks(1, 1, 6), 0);
        assert_eq!(shrinks(1024, 1024, 3), 2);
    }

    /// A very wide, very short image would otherwise be halved until it had no
    /// rows left.
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

    /// Every pixel sits on the threshold, so every pixel is excluded and all
    /// nine candidates tie at zero disagreement. The honest answer is that
    /// nothing can be said, which is what no movement means.
    #[test]
    fn an_exposure_with_no_usable_signal_reports_no_shift() {
        let flat = Gray::from_vec(vec![128; 160 * 120], 160, 120);

        assert_eq!(shift(&flat, &flat, &Options::default()), Shift::ZERO);
    }

    #[test]
    fn a_single_pixel_image_reports_no_shift() {
        let dot = Gray::from_vec(vec![200], 1, 1);

        assert_eq!(shift(&dot, &dot, &Options::default()), Shift::ZERO);
    }

    #[test]
    #[should_panic(expected = "cannot align a 4x4 exposure against a 5x5 one")]
    fn aligning_mismatched_sizes_is_rejected() {
        let small = Gray::from_vec(vec![0; 16], 4, 4);
        let large = Gray::from_vec(vec![0; 25], 5, 5);

        shift(&small, &large, &Options::default());
    }
}
