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

    /// A synthetic scene: value noise summed over three cell sizes, so the
    /// coarse levels of the pyramid carry structure and the finest carries
    /// detail sharp enough that a one-pixel error costs something.
    ///
    /// A smooth analytic scene makes a poor fixture. Gentle gradients cross the
    /// median in the same place under a small shift, so the bitmaps come out
    /// bit-identical and every candidate ties at zero — which says more about
    /// the fixture than the search.
    fn scene(x: i64, y: i64) -> u8 {
        let value = 0.5 * octave(x, y, 64) + 0.3 * octave(x, y, 16) + 0.2 * octave(x, y, 4);

        (value.clamp(0.0, 1.0) * 255.0) as u8
    }

    /// One octave of value noise: hashed lattice corners, smoothstepped between.
    fn octave(x: i64, y: i64, cell: i64) -> f64 {
        let (i, j) = (x.div_euclid(cell), y.div_euclid(cell));
        let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
        let across = smooth(x as f64 / cell as f64 - i as f64);
        let down = smooth(y as f64 / cell as f64 - j as f64);

        let corner = |dx, dy| lattice(i + dx, j + dy);
        let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * across;
        let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * across;

        top + (bottom - top) * down
    }

    fn lattice(x: i64, y: i64) -> f64 {
        let mut hash = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        hash ^= hash >> 29;
        hash = hash.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        hash ^= hash >> 32;

        (hash >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Reads the scene through a window moved by `offset`, so both exposures are
    /// complete images. Padding a translated copy instead would invent an edge
    /// that the alignment could cheat off.
    fn window(width: usize, height: usize, offset: Shift) -> Gray {
        let samples = (0..width * height)
            .map(|i| {
                let x = (i % width) as i64 + offset.x as i64;
                let y = (i / width) as i64 + offset.y as i64;
                scene(x, y)
            })
            .collect();

        Gray::from_vec(samples, width, height)
    }

    #[test]
    fn an_exposure_needs_no_shift_against_itself() {
        let image = window(160, 120, Shift::ZERO);

        assert_eq!(
            shift(&image, &image, &Options::default()),
            Shift::ZERO,
            "an image is already aligned with itself"
        );
    }

    #[test]
    fn a_known_translation_comes_back_exactly() {
        let reference = window(400, 300, Shift::ZERO);

        for offset in [
            Shift::new(1, 0),
            Shift::new(0, -1),
            Shift::new(7, 5),
            Shift::new(-11, 3),
            Shift::new(-13, -9),
            Shift::new(21, -34),
        ] {
            let target = window(400, 300, offset);

            assert_eq!(
                shift(&reference, &target, &Options::default()),
                offset,
                "failed to recover {offset:?}"
            );
        }
    }

    /// Six bits of shift reach 63 pixels, which is the last offset the default
    /// search can express.
    #[test]
    fn the_search_reaches_the_edge_of_its_range() {
        let reference = window(400, 300, Shift::ZERO);
        let offset = Shift::new(63, -63);

        assert_eq!(
            shift(&reference, &window(400, 300, offset), &Options::default()),
            offset
        );
    }

    #[test]
    fn fewer_bits_search_a_smaller_range() {
        let reference = window(400, 300, Shift::ZERO);
        let options = Options {
            bits: 3,
            ..Options::default()
        };
        let reachable = Shift::new(7, -7);

        assert_eq!(
            shift(&reference, &window(400, 300, reachable), &options),
            reachable
        );

        let beyond = Shift::new(40, 0);
        assert_ne!(
            shift(&reference, &window(400, 300, beyond), &options),
            beyond
        );
    }

    #[test]
    fn both_shrink_conventions_recover_the_same_translation() {
        let reference = window(200, 150, Shift::ZERO);
        let offset = Shift::new(-9, 6);
        let target = window(200, 150, offset);

        for shrink in [Shrink::Average, Shrink::Subsample] {
            let options = Options {
                shrink,
                ..Options::default()
            };

            assert_eq!(shift(&reference, &target, &options), offset, "{shrink:?}");
        }
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
}
