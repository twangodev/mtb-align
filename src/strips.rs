//! Splitting row work across threads, when the `rayon` feature asks for it.

/// Runs `fill` over each row of `buffer`, a row being `stride` elements.
pub(crate) fn fill_rows<T: Send>(
    buffer: &mut [T],
    stride: usize,
    fill: impl Fn(usize, &mut [T]) + Send + Sync,
) {
    // Chunking by zero panics rather than yielding nothing.
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

/// Adds up something counted once per row. Associative, so the total does not
/// depend on the order threads finish in.
pub(crate) fn sum_rows(rows: usize, of: impl Fn(usize) -> u64 + Send + Sync) -> u64 {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;

        (0..rows).into_par_iter().map(of).sum()
    }

    #[cfg(not(feature = "rayon"))]
    (0..rows).map(of).sum()
}
