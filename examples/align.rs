//! Aligns a bracketed exposure sequence and writes the registered frames out.
//!
//! ```text
//! cargo run --example align                      # a synthetic five-frame bracket
//! cargo run --example align -- a.jpg b.jpg c.jpg
//! ```
//!
//! The offsets are found on greyscale copies and then applied to the colour
//! frames, which is the usual shape of this: alignment only ever needs one
//! channel, and it has to happen before the camera response is known.

use std::path::{Path, PathBuf};

use image::RgbImage;
use mtb_align::{Gray, Options, Rect, Shift, align_stack, common_crop};

/// Where the synthetic bracket was pointed, so the recovered offsets have
/// something to be checked against.
const HANDHELD: [Shift; 5] = [
    Shift { x: 0, y: 0 },
    Shift { x: 4, y: -3 },
    Shift { x: -6, y: 8 },
    Shift { x: 11, y: 5 },
    Shift { x: -9, y: -13 },
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (frames, truth) = if paths.is_empty() {
        (synthetic(), Some(HANDHELD))
    } else {
        (paths.iter().map(|path| load(path)).collect(), None)
    };

    let (width, height) = (frames[0].width() as usize, frames[0].height() as usize);
    assert!(
        frames
            .iter()
            .all(|frame| frame.dimensions() == frames[0].dimensions()),
        "every exposure has to be the same size"
    );

    let grays: Vec<Gray> = frames
        .iter()
        .map(|frame| Gray::from_rgb(frame.as_raw(), width, height))
        .collect();

    // The middle of a bracket is the frame most likely to share detail with
    // both ends of it.
    let reference = grays.len() / 2;
    let shifts = align_stack(&grays, reference, &Options::default());
    let crop = common_crop(&shifts, width, height);

    println!(
        "{width}x{height}, {} exposures, frame {reference} as reference",
        frames.len()
    );
    for (index, shift) in shifts.iter().enumerate() {
        let against_truth = truth.map_or(String::new(), |truth| {
            let expected = Shift::new(
                truth[index].x - truth[reference].x,
                truth[index].y - truth[reference].y,
            );
            if *shift == expected {
                "   ok".to_string()
            } else {
                format!("   <- shot at ({}, {})", expected.x, expected.y)
            }
        });

        println!(
            "  frame {index}  ({:>4}, {:>4}){against_truth}",
            shift.x, shift.y
        );
    }
    println!(
        "  common crop {}x{} at ({}, {})",
        crop.width, crop.height, crop.x, crop.y
    );

    let output = PathBuf::from("target/aligned");
    std::fs::create_dir_all(&output).expect("create output directory");

    for (index, (frame, &shift)) in frames.iter().zip(&shifts).enumerate() {
        cropped(frame, Shift::ZERO, crop)
            .save(output.join(format!("{index}-before.png")))
            .expect("write png");
        cropped(frame, shift, crop)
            .save(output.join(format!("{index}-after.png")))
            .expect("write png");
    }

    println!("  written to {}", output.display());
}

/// Moves a frame by its offset and keeps the region every frame covers.
///
/// Inside the common crop every frame has real pixels, so this never has to
/// invent one.
fn cropped(frame: &RgbImage, shift: Shift, crop: Rect) -> RgbImage {
    RgbImage::from_fn(crop.width as u32, crop.height as u32, |x, y| {
        let source_x = (crop.x + x as usize) as i64 - shift.x as i64;
        let source_y = (crop.y + y as usize) as i64 - shift.y as i64;

        *frame.get_pixel(source_x as u32, source_y as u32)
    })
}

fn load(path: &str) -> RgbImage {
    image::open(Path::new(path))
        .unwrap_or_else(|failure| panic!("decode {path}: {failure}"))
        .to_rgb8()
}

/// Five frames a stop apart, each shot from a slightly different position.
fn synthetic() -> Vec<RgbImage> {
    const SIZE: (u32, u32) = (640, 480);

    HANDHELD
        .iter()
        .enumerate()
        .map(|(index, offset)| {
            let stops = index as f64 - 2.0;

            RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| {
                let (x, y) = (x as i64 + offset.x as i64, y as i64 + offset.y as i64);
                let value = expose(scene(x, y), stops);
                let warm = expose(scene(x, y + 4096), stops);

                image::Rgb([warm, value, value.saturating_sub(12)])
            })
        })
        .collect()
}

/// A monotonic tone change, the way a different shutter speed reads out.
fn expose(sample: u8, stops: f64) -> u8 {
    const GAMMA: f64 = 2.2;
    let linear = (sample as f64 / 255.0).powf(GAMMA) * 2f64.powf(stops);

    (linear.min(1.0).powf(1.0 / GAMMA) * 255.0) as u8
}

/// Value noise at three scales, so the pyramid has structure at every level.
fn scene(x: i64, y: i64) -> u8 {
    let value = 0.5 * octave(x, y, 64) + 0.3 * octave(x, y, 16) + 0.2 * octave(x, y, 4);

    (value.clamp(0.0, 1.0) * 255.0) as u8
}

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
