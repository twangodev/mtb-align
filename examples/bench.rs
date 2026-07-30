//! Times an alignment at sensor resolution.
//!
//! ```text
//! cargo run --release --example bench                    # GFX 50, 51 MP
//! cargo run --release --example bench -- 16384 12288     # 200 MP
//! ```

use std::time::Instant;

use mtb_align::{Gray, Options, Shift, shift};

const TRUTH: Shift = Shift { x: 37, y: -22 };

fn main() {
    let mut args = std::env::args().skip(1).map(|arg| {
        arg.parse::<usize>()
            .expect("usage: bench [<width> <height>]")
    });
    let (width, height) = match (args.next(), args.next()) {
        (Some(width), Some(height)) => (width, height),
        _ => (8256, 6192),
    };

    let megapixels = (width * height) as f64 / 1e6;
    let options = Options::default();

    let start = Instant::now();
    let reference = scene(width, height, Shift::ZERO);
    let target = scene(width, height, TRUTH);
    let build = start.elapsed();

    let start = Instant::now();
    let found = shift(&reference, &target, &options).expect("same size");
    let align = start.elapsed();

    println!(
        "{width}x{height} ({megapixels:.1} MP), {} bits, single channel",
        options.bits
    );
    println!("  synthesise  {:>7.0} ms", build.as_secs_f64() * 1e3);
    println!(
        "  align       {:>7.0} ms   {:>6.1} MP/s",
        align.as_secs_f64() * 1e3,
        megapixels / align.as_secs_f64()
    );
    println!(
        "  recovered ({}, {}), shifted by ({}, {}){}",
        found.x,
        found.y,
        TRUTH.x,
        TRUTH.y,
        if found == TRUTH { "" } else { "   <- MISSED" }
    );
}

/// Blocky detail at three scales. Value noise would look more like a photograph
/// and cost more than the thing being measured.
fn scene(width: usize, height: usize, offset: Shift) -> Gray {
    let samples = (0..width * height)
        .map(|index| {
            let x = ((index % width) as u64).wrapping_add_signed(offset.x as i64);
            let y = ((index / width) as u64).wrapping_add_signed(offset.y as i64);

            let coarse = hash(x / 64, y / 64) as u32;
            let middle = hash(x / 8, y / 8) as u32;
            let fine = hash(x, y) as u32;

            ((3 * coarse + 2 * middle + fine) / 6) as u8
        })
        .collect();

    Gray::from_vec(samples, width, height)
}

fn hash(x: u64, y: u64) -> u8 {
    let mut mixed = x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    mixed ^= mixed >> 29;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);

    (mixed >> 56) as u8
}
