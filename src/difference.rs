use crate::bitmap::WORD_BITS;
use crate::{Bitmaps, Shift};

/// How many pixels the two exposures disagree about at one candidate offset.
///
/// This is Ward's error term, `BitmapTotal` of
///
/// ```text
/// (tb1 XOR shift(tb2)) AND eb1 AND shift(eb2)
/// ```
///
/// counted without materialising any of it. Ward's own formulation allocates
/// three bitmaps and makes six whole-image passes per candidate, and the search
/// tries nine candidates at every level.
///
/// The exclusion terms mask the *result* of the XOR, never its operands. Ward's
/// footnote is explicit about the difference: masking each threshold bitmap
/// first and comparing what survives would count a disagreement about which
/// pixels are noise as though it were a disagreement about the image.
///
/// Panics unless every bitmap has the same dimensions.
pub fn disagreement(reference: &Bitmaps, target: &Bitmaps, shift: Shift) -> u64 {
    let (width, height) = (reference.threshold.width(), reference.threshold.height());
    assert!(
        [&reference.exclusion, &target.threshold, &target.exclusion]
            .iter()
            .all(|bitmap| bitmap.width() == width && bitmap.height() == height),
        "cannot compare a {width}x{height} exposure with a {}x{} one",
        target.threshold.width(),
        target.threshold.height()
    );

    let words_per_row = reference.threshold.words_per_row();
    let mut total = 0;

    for y in 0..height {
        // A row drawn from outside the target contributes nothing: both of the
        // target's bitmaps are zero there, and the exclusion term rules the
        // whole row out. Ward gets the same effect by clearing the exposed
        // border of the shifted bitmaps.
        let Ok(source_y) = usize::try_from(y as i64 - shift.y as i64) else {
            continue;
        };
        if source_y >= height {
            continue;
        }

        let reference_threshold = reference.threshold.row(y);
        let reference_exclusion = reference.exclusion.row(y);
        let target_threshold = target.threshold.row(source_y);
        let target_exclusion = target.exclusion.row(source_y);

        for word in 0..words_per_row {
            // Bits past the width can pick up whatever the shift drags in, but
            // the reference exclusion bitmap holds its padding clear, so they
            // are masked away before they can be counted.
            let disagrees = (reference_threshold[word]
                ^ shifted_word(target_threshold, word, shift.x))
                & reference_exclusion[word]
                & shifted_word(target_exclusion, word, shift.x);

            total += disagrees.count_ones() as u64;
        }
    }

    total
}

/// The word landing on destination word `index` once `row` moves right by
/// `shift` pixels, with zeros arriving from outside the row.
fn shifted_word(row: &[u64], index: usize, shift: i32) -> u64 {
    let base = index as i64 * WORD_BITS as i64 - shift as i64;
    let word = base.div_euclid(WORD_BITS as i64);
    let bit = base.rem_euclid(WORD_BITS as i64) as u32;

    let fetch = |index: i64| match usize::try_from(index) {
        Ok(index) if index < row.len() => row[index],
        _ => 0,
    };

    // A shift that is a whole number of words needs no funnelling, and asking
    // for a 64-bit shift below would be undefined anyway.
    if bit == 0 {
        fetch(word)
    } else {
        (fetch(word) >> bit) | (fetch(word + 1) << (WORD_BITS as u32 - bit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bitmap;
    use proptest::prelude::*;

    fn bitmap_from(width: usize, height: usize, set: impl Fn(usize, usize) -> bool) -> Bitmap {
        let mut bitmap = Bitmap::zeros(width, height);
        for y in 0..height {
            for x in 0..width {
                bitmap.set(x, y, set(x, y));
            }
        }
        bitmap
    }

    fn bitmaps_from(
        width: usize,
        height: usize,
        threshold: impl Fn(usize, usize) -> bool,
        exclusion: impl Fn(usize, usize) -> bool,
    ) -> Bitmaps {
        Bitmaps {
            threshold: bitmap_from(width, height, threshold),
            exclusion: bitmap_from(width, height, exclusion),
        }
    }

    /// Ward's formulation, written out operation by operation exactly as the
    /// paper lists it, as something for the fused version to be checked
    /// against. Slow and obviously correct is the whole point.
    mod reference {
        use super::*;

        /// `BitmapShift`, clearing exposed border areas to zero.
        fn shifted(bitmap: &Bitmap, shift: Shift) -> Bitmap {
            let (width, height) = (bitmap.width(), bitmap.height());
            bitmap_from(width, height, |x, y| {
                let source_x = x as i64 - shift.x as i64;
                let source_y = y as i64 - shift.y as i64;
                (0..width as i64).contains(&source_x)
                    && (0..height as i64).contains(&source_y)
                    && bitmap.get(source_x as usize, source_y as usize)
            })
        }

        fn combine(a: &Bitmap, b: &Bitmap, op: impl Fn(bool, bool) -> bool) -> Bitmap {
            bitmap_from(a.width(), a.height(), |x, y| op(a.get(x, y), b.get(x, y)))
        }

        pub fn disagreement(reference: &Bitmaps, target: &Bitmaps, shift: Shift) -> u64 {
            let shifted_tb2 = shifted(&target.threshold, shift);
            let shifted_eb2 = shifted(&target.exclusion, shift);

            let diff = combine(&reference.threshold, &shifted_tb2, |a, b| a ^ b);
            let diff = combine(&diff, &reference.exclusion, |a, b| a & b);
            let diff = combine(&diff, &shifted_eb2, |a, b| a & b);

            diff.count_ones()
        }
    }

    #[test]
    fn an_exposure_never_disagrees_with_itself() {
        let bitmaps = bitmaps_from(70, 9, |x, y| (x + y).is_multiple_of(3), |_, _| true);

        assert_eq!(disagreement(&bitmaps, &bitmaps, Shift::ZERO), 0);
    }

    #[test]
    fn an_inverted_threshold_disagrees_everywhere() {
        let all = bitmaps_from(70, 9, |_, _| true, |_, _| true);
        let none = bitmaps_from(70, 9, |_, _| false, |_, _| true);

        assert_eq!(disagreement(&all, &none, Shift::ZERO), 70 * 9);
    }

    /// Either exposure can veto a pixel, so a disagreement only counts where
    /// both exclusion bitmaps allow it.
    #[test]
    fn a_pixel_counts_only_where_both_exposures_allow_it() {
        let all = bitmaps_from(70, 9, |_, _| true, |_, _| true);
        let inverted_but_mostly_excluded = bitmaps_from(
            70,
            9,
            |_, _| false,
            |x, y| (x, y) == (5, 5) || (x, y) == (6, 5),
        );

        assert_eq!(
            disagreement(&all, &inverted_but_mostly_excluded, Shift::ZERO),
            2
        );

        let reference_excludes_one = bitmaps_from(70, 9, |_, _| true, |x, y| (x, y) == (5, 5));
        assert_eq!(
            disagreement(
                &reference_excludes_one,
                &inverted_but_mostly_excluded,
                Shift::ZERO
            ),
            1
        );
    }

    /// The whole search rests on this: the offset that undoes the translation
    /// is the one that scores zero.
    #[test]
    fn the_offset_that_undoes_a_translation_scores_zero() {
        let pattern = |x: usize, y: usize| (x / 3 + y / 2).is_multiple_of(2);
        let reference = bitmaps_from(70, 20, pattern, |_, _| true);
        let target = bitmaps_from(70, 20, |x, y| pattern(x + 4, y + 3), |_, _| true);

        assert_eq!(disagreement(&reference, &target, Shift::new(4, 3)), 0);
        assert!(disagreement(&reference, &target, Shift::new(3, 3)) > 0);
        assert!(disagreement(&reference, &target, Shift::new(4, 2)) > 0);
    }

    /// Shifting the target off the reference entirely leaves no overlap, and
    /// the exposed border of the shifted exclusion bitmap clears the rest.
    /// Ward relies on this to bound the offset: without it the emptiest overlap
    /// would always look like the best match.
    #[test]
    fn a_shift_clear_of_the_image_leaves_nothing_to_disagree_about() {
        let all = bitmaps_from(70, 9, |_, _| true, |_, _| true);
        let none = bitmaps_from(70, 9, |_, _| false, |_, _| true);

        assert_eq!(disagreement(&all, &none, Shift::new(70, 0)), 0);
        assert_eq!(disagreement(&all, &none, Shift::new(0, 9)), 0);
        assert_eq!(disagreement(&all, &none, Shift::new(-70, -9)), 0);
        assert_eq!(disagreement(&all, &none, Shift::new(5000, -5000)), 0);
    }

    #[test]
    #[should_panic(expected = "cannot compare a 70x9 exposure with a 8x8 one")]
    fn comparing_mismatched_sizes_is_rejected() {
        let big = bitmaps_from(70, 9, |_, _| true, |_, _| true);
        let small = bitmaps_from(8, 8, |_, _| true, |_, _| true);

        disagreement(&big, &small, Shift::ZERO);
    }

    proptest! {
        /// The fused count must agree with Ward's literal sequence for every
        /// width, every shift, and both the aligned and wildly-off cases.
        /// Widths either side of 64 are what exercise the word arithmetic.
        #[test]
        fn the_fused_count_matches_wards_literal_sequence(
            width in 1usize..140,
            height in 1usize..12,
            shift_x in -150i32..150,
            shift_y in -15i32..15,
            reference_seed in any::<u64>(),
            target_seed in any::<u64>(),
        ) {
            let bit = |seed: u64, salt: u64| move |x: usize, y: usize| {
                let mixed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add((x as u64) << 17 ^ (y as u64) << 3 ^ salt);
                mixed.rotate_left(29).count_ones().is_multiple_of(2)
            };

            let reference = bitmaps_from(width, height, bit(reference_seed, 1), bit(reference_seed, 2));
            let target = bitmaps_from(width, height, bit(target_seed, 3), bit(target_seed, 4));
            let shift = Shift::new(shift_x, shift_y);

            prop_assert_eq!(
                disagreement(&reference, &target, shift),
                reference::disagreement(&reference, &target, shift)
            );
        }
    }
}
