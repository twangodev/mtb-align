/// Bits per word. Ward counts 32 or 64 at a time; this crate always takes 64.
pub(crate) const WORD_BITS: usize = u64::BITS as usize;

/// A one-bit-per-pixel image packed into `u64` words, each row starting on a
/// word boundary.
///
/// Ward instead flattens the whole image into one bit array, so a
/// two-dimensional shift becomes a single one-dimensional shift plus a clear
/// along one or two exposed edges. Word-aligned rows cost at most 63 padding
/// bits per row — under one percent at sensor widths — and in exchange a
/// vertical shift is just a row index and a horizontal shift never crosses a
/// row boundary.
///
/// Bits run least-significant first, so pixel `x` lives in bit `x % 64` of word
/// `x / 64` and moving right in the image is a left shift in the word.
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    words: Vec<u64>,
    width: usize,
    height: usize,
    words_per_row: usize,
}

impl Bitmap {
    pub fn zeros(width: usize, height: usize) -> Self {
        let words_per_row = width.div_ceil(WORD_BITS);
        Self {
            words: vec![0; words_per_row * height],
            width,
            height,
            words_per_row,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Panics unless the coordinate is inside the bitmap.
    pub fn get(&self, x: usize, y: usize) -> bool {
        let (word, bit) = self.locate(x, y);
        self.words[word] >> bit & 1 == 1
    }

    /// Panics unless the coordinate is inside the bitmap.
    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        let (word, bit) = self.locate(x, y);
        if value {
            self.words[word] |= 1 << bit;
        } else {
            self.words[word] &= !(1 << bit);
        }
    }

    /// Ward's `BitmapTotal`. The padding bits past `width` are held clear, so
    /// this can count whole words without masking.
    pub fn count_ones(&self) -> u64 {
        self.words.iter().map(|word| word.count_ones() as u64).sum()
    }

    /// Builds a bitmap by testing every pixel, accumulating whole words before
    /// storing them.
    ///
    /// Setting one bit at a time costs a bounds check, a division and a
    /// read-modify-write per pixel, which at sensor resolution is most of the
    /// time the whole alignment takes. Here the inner loop has no branch and no
    /// memory traffic until a word is finished.
    pub(crate) fn packed(
        width: usize,
        height: usize,
        mut bit: impl FnMut(usize, usize) -> bool,
    ) -> Self {
        let mut bitmap = Self::zeros(width, height);
        let words_per_row = bitmap.words_per_row;

        for y in 0..height {
            let row = &mut bitmap.words[y * words_per_row..(y + 1) * words_per_row];

            for (index, word) in row.iter_mut().enumerate() {
                let base = index * WORD_BITS;
                // The last word of a row stops at the width, which is what
                // keeps the padding clear.
                let span = WORD_BITS.min(width - base);

                let mut packed = 0;
                for offset in 0..span {
                    packed |= (bit(base + offset, y) as u64) << offset;
                }

                *word = packed;
            }
        }

        bitmap
    }

    /// How many words each row occupies, padding included.
    ///
    /// Exposed alongside [`Bitmap::row`] because the whole point of packing is
    /// to let callers work a word at a time; see the type's note on bit order.
    pub fn words_per_row(&self) -> usize {
        self.words_per_row
    }

    pub fn row(&self, y: usize) -> &[u64] {
        &self.words[y * self.words_per_row..(y + 1) * self.words_per_row]
    }

    fn locate(&self, x: usize, y: usize) -> (usize, usize) {
        assert!(
            x < self.width && y < self.height,
            "({x}, {y}) is outside a {}x{} bitmap",
            self.width,
            self.height
        );
        (y * self.words_per_row + x / WORD_BITS, x % WORD_BITS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_new_bitmap_holds_no_bits() {
        let bitmap = Bitmap::zeros(100, 40);

        assert_eq!(bitmap.width(), 100);
        assert_eq!(bitmap.height(), 40);
        assert_eq!(bitmap.count_ones(), 0);
    }

    #[test]
    fn a_set_bit_reads_back() {
        let mut bitmap = Bitmap::zeros(100, 40);

        bitmap.set(70, 3, true);

        assert!(bitmap.get(70, 3));
        assert!(!bitmap.get(69, 3));
        assert!(!bitmap.get(70, 2));
        assert_eq!(bitmap.count_ones(), 1);
    }

    #[test]
    fn clearing_a_bit_removes_it() {
        let mut bitmap = Bitmap::zeros(8, 2);

        bitmap.set(5, 1, true);
        bitmap.set(5, 1, false);

        assert!(!bitmap.get(5, 1));
        assert_eq!(bitmap.count_ones(), 0);
    }

    /// A row narrower than a word leaves 61 bits of slack. Counting whole words
    /// only works because nothing ever sets them.
    #[test]
    fn the_padding_past_the_width_never_counts() {
        let mut bitmap = Bitmap::zeros(3, 4);

        for y in 0..4 {
            for x in 0..3 {
                bitmap.set(x, y, true);
            }
        }

        assert_eq!(bitmap.count_ones(), 12);
    }

    /// Every row starts a fresh word, so a narrow bitmap spends one word per
    /// row rather than packing rows end to end.
    #[test]
    fn each_row_starts_on_a_word_boundary() {
        let bitmap = Bitmap::zeros(3, 4);

        assert_eq!(bitmap.words_per_row(), 1);
        assert_eq!(bitmap.row(0).len(), 1);
        assert_eq!(bitmap.row(3).len(), 1);
    }

    #[test]
    fn a_row_wider_than_one_word_spans_several() {
        let mut bitmap = Bitmap::zeros(130, 2);

        assert_eq!(bitmap.words_per_row(), 3);

        bitmap.set(0, 0, true);
        bitmap.set(64, 0, true);
        bitmap.set(129, 0, true);

        assert_eq!(bitmap.row(0), &[1, 1, 1 << 1]);
        assert_eq!(bitmap.count_ones(), 3);
    }

    #[test]
    #[should_panic(expected = "(3, 0) is outside a 3x4 bitmap")]
    fn reading_past_the_width_panics() {
        Bitmap::zeros(3, 4).get(3, 0);
    }

    #[test]
    #[should_panic(expected = "(0, 4) is outside a 3x4 bitmap")]
    fn writing_past_the_height_panics() {
        Bitmap::zeros(3, 4).set(0, 4, true);
    }

    proptest! {
        /// Packing whole words has to agree with setting the bits one at a
        /// time, at every width — a word that ran past the end of a row would
        /// leave padding set and be counted.
        #[test]
        fn packing_a_word_at_a_time_matches_setting_bit_by_bit(
            width in 1usize..200,
            height in 1usize..8,
            seed in any::<u64>(),
        ) {
            let bit = |x: usize, y: usize| {
                (seed ^ (x as u64) << 7 ^ (y as u64)).wrapping_mul(0x2545F491_4F6CDD1D).count_ones().is_multiple_of(3)
            };

            let mut one_at_a_time = Bitmap::zeros(width, height);
            for y in 0..height {
                for x in 0..width {
                    one_at_a_time.set(x, y, bit(x, y));
                }
            }

            prop_assert!(Bitmap::packed(width, height, bit) == one_at_a_time);
        }

        /// How much slack a row carries depends on `width % 64`, so the padding
        /// invariant has to hold at every width rather than the one a unit test
        /// happens to pick.
        #[test]
        fn the_count_matches_the_bits_actually_set(
            width in 1usize..200,
            height in 1usize..20,
            positions in prop::collection::vec((0usize..200, 0usize..20), 0..50),
        ) {
            let mut bitmap = Bitmap::zeros(width, height);
            let inside: std::collections::BTreeSet<_> = positions
                .into_iter()
                .filter(|&(x, y)| x < width && y < height)
                .collect();

            for &(x, y) in &inside {
                bitmap.set(x, y, true);
            }

            prop_assert_eq!(bitmap.count_ones(), inside.len() as u64);
            for &(x, y) in &inside {
                prop_assert!(bitmap.get(x, y));
            }
        }

        /// Reading back every coordinate proves rows do not overlap, which is
        /// the failure mode if `words_per_row` and the row slice disagree.
        #[test]
        fn setting_one_bit_leaves_every_other_clear(
            width in 1usize..140,
            height in 1usize..10,
            index in 0usize..1400,
        ) {
            let (x, y) = (index % width, index / width % height);
            let mut bitmap = Bitmap::zeros(width, height);

            bitmap.set(x, y, true);

            for probe_y in 0..height {
                for probe_x in 0..width {
                    prop_assert_eq!(bitmap.get(probe_x, probe_y), (probe_x, probe_y) == (x, y));
                }
            }
        }
    }
}
