//! Splitting row work across threads, when the `rayon` feature asks for it.
//!
//! Every pass in the crate is row-independent, so the split is always the same
//! shape and the two versions of each helper differ only in which iterator they
//! reach for.

/// Runs `fill` over each row of `buffer`, a row being `stride` elements.
pub(crate) fn fill_rows<T: Send>(
    buffer: &mut [T],
    stride: usize,
    fill: impl Fn(usize, &mut [T]) + Send + Sync,
) {
    // An image with no width has no rows to hand out, and chunking by zero is
    // a panic rather than an empty iterator.
    if stride == 0 {
        return;
    }

    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        buffer
            .par_chunks_mut(stride)
            .enumerate()
            .for_each(|(y, row)| fill(y, row));
    }

    #[cfg(not(feature = "rayon"))]
    buffer
        .chunks_mut(stride)
        .enumerate()
        .for_each(|(y, row)| fill(y, row));
}

/// Adds up something counted once per row.
///
/// Addition of the per-row counts is associative and the counts themselves do
/// not depend on each other, so the total does not depend on the order the
/// threads finish in.
pub(crate) fn sum_rows(rows: usize, of: impl Fn(usize) -> u64 + Send + Sync) -> u64 {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        (0..rows).into_par_iter().map(of).sum()
    }

    #[cfg(not(feature = "rayon"))]
    (0..rows).map(of).sum()
}
