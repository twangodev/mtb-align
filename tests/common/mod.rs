//! A synthetic scene for the validation tests to photograph.
//!
//! Every exposure is read out of the same infinite scene through a window, so a
//! translated pair are both complete images. Padding a shifted copy instead
//! would leave a border of invented pixels along one edge, and an alignment
//! that locked onto that border would look like a success.

// Each integration test uses its own subset of these.
#![allow(dead_code)]

use mtb_align::{Gray, Shift};

/// Value noise summed over three cell sizes, so the coarse levels of the
/// pyramid carry structure and the finest carries detail sharp enough that a
/// one-pixel error costs something.
///
/// A smooth analytic scene makes a poor fixture: gentle gradients cross the
/// median in the same place under a small shift, so the bitmaps come out
/// bit-identical and every candidate offset ties at zero.
pub fn scene(x: i64, y: i64) -> u8 {
    let value = 0.5 * octave(x, y, 64, 0) + 0.3 * octave(x, y, 16, 1) + 0.2 * octave(x, y, 4, 2);

    quantise(value)
}

/// Reads the scene through a window moved by `offset`.
pub fn window(width: usize, height: usize, offset: Shift) -> Gray {
    map_coordinates(width, height, offset, scene)
}

/// A window over the scene whose lower two thirds is replaced by a plateau
/// sitting on the median, textured by a pattern fixed to the *sensor* rather
/// than to the scene.
///
/// This is Ward's Figure 3 with teeth. Every pixel of the plateau is within a
/// couple of levels of the threshold, so which side it lands on carries no
/// information about the scene — and because the pattern does not move with
/// the camera, it argues for an offset of zero just as loudly as the real
/// scene argues for the truth. Fixed-pattern sensor noise, dust and amp glow
/// all behave this way.
///
/// Noise fixed to the *scene* instead would not make this point: it costs
/// every candidate offset about the same, so it raises the whole error surface
/// without moving its minimum.
pub fn window_with_fixed_pattern_plateau(width: usize, height: usize, offset: Shift) -> Gray {
    let horizon = height / 3;
    let mut samples = window(width, height, offset).as_slice().to_vec();

    for y in horizon..height {
        for x in 0..width {
            // Straddling the threshold is the whole point: a plateau that sits
            // entirely to one side of it produces the same bit everywhere and
            // costs the comparison nothing. Blocks rather than single pixels so
            // that halving the image averages the pattern down instead of away,
            // and it still misleads the coarse levels where the search commits.
            let wobble = (lattice(x as i64 / 4, y as i64 / 4) * 9.0) as i64 - 4;
            samples[y * width + x] = (128 + wobble).clamp(0, 255) as u8;
        }
    }

    Gray::from_vec(samples, width, height)
}

/// A monotonic tone change of the kind a different shutter speed produces:
/// scale the linear signal, then put it back through the display gamma.
///
/// The median survives any monotonic change, which is the property the whole
/// algorithm rests on. Highlights clip once `stops` is positive, and that part
/// is not monotonic — it is also exactly what Ward means by staying "within the
/// usable range of the camera".
pub fn reexpose(gray: &Gray, stops: f64) -> Gray {
    const GAMMA: f64 = 2.2;
    let gain = 2f64.powf(stops);

    map_samples(gray, |sample| {
        let linear = (sample as f64 / 255.0).powf(GAMMA) * gain;

        quantise(linear.min(1.0).powf(1.0 / GAMMA))
    })
}

/// Adds deterministic pseudo-random noise of up to `amplitude` levels.
pub fn noisy(gray: &Gray, amplitude: u8, seed: u64) -> Gray {
    let width = gray.width() as i64;

    let noised = gray
        .as_slice()
        .iter()
        .enumerate()
        .map(|(index, &sample)| {
            let (x, y) = ((index as i64) % width, (index as i64) / width);
            let swing = lattice(x.wrapping_add(seed as i64), y) * 2.0 - 1.0;
            let offset = (swing * amplitude as f64).round() as i64;

            (sample as i64 + offset).clamp(0, 255) as u8
        })
        .collect();

    Gray::from_vec(noised, gray.width(), gray.height())
}

fn map_coordinates(
    width: usize,
    height: usize,
    offset: Shift,
    sample: impl Fn(i64, i64) -> u8,
) -> Gray {
    let samples = (0..width * height)
        .map(|index| {
            let x = (index % width) as i64 + offset.x as i64;
            let y = (index / width) as i64 + offset.y as i64;
            sample(x, y)
        })
        .collect();

    Gray::from_vec(samples, width, height)
}

fn map_samples(gray: &Gray, map: impl Fn(u8) -> u8) -> Gray {
    let mapped = gray.as_slice().iter().map(|&sample| map(sample)).collect();

    Gray::from_vec(mapped, gray.width(), gray.height())
}

fn quantise(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0) as u8
}

/// One octave of value noise: hashed lattice corners, smoothstepped between.
fn octave(x: i64, y: i64, cell: i64, salt: i64) -> f64 {
    let (i, j) = (x.div_euclid(cell), y.div_euclid(cell));
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let across = smooth(x as f64 / cell as f64 - i as f64);
    let down = smooth(y as f64 / cell as f64 - j as f64);

    let corner = |dx, dy| lattice(i + dx, j + dy + salt * 4096);
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
