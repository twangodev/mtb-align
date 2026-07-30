//! Photographs one synthetic scene twice a known distance apart and asks for
//! that distance back, degrading the second exposure the way a real bracket
//! would — noise, a change of shutter speed, both at once.

mod common;

use common::{noisy, reexpose, window, window_with_fixed_pattern_plateau};
use mtb_align::{Options, Shift, Shrink, shift};
use proptest::prelude::*;

const WIDTH: usize = 400;
const HEIGHT: usize = 300;

fn recovered(offset: Shift, options: &Options) -> Shift {
    shift(
        &window(WIDTH, HEIGHT, Shift::ZERO),
        &window(WIDTH, HEIGHT, offset),
        options,
    )
}

#[test]
fn an_exposure_needs_no_shift_against_itself() {
    assert_eq!(recovered(Shift::ZERO, &Options::default()), Shift::ZERO);
}

#[test]
fn a_known_translation_comes_back_exactly() {
    for offset in [
        Shift::new(1, 0),
        Shift::new(0, -1),
        Shift::new(7, 5),
        Shift::new(-11, 3),
        Shift::new(-13, -9),
        Shift::new(21, -34),
        Shift::new(-40, -40),
    ] {
        assert_eq!(
            recovered(offset, &Options::default()),
            offset,
            "failed to recover {offset:?}"
        );
    }
}

/// Six bits reach 63 pixels, the last offset the default search can express.
#[test]
fn the_search_reaches_the_edge_of_its_range() {
    for offset in [
        Shift::new(63, -63),
        Shift::new(-63, 63),
        Shift::new(63, 63),
        Shift::new(-63, -63),
    ] {
        assert_eq!(recovered(offset, &Options::default()), offset);
    }
}

#[test]
fn fewer_bits_search_a_smaller_range() {
    let options = Options {
        bits: 3,
        ..Options::default()
    };

    let reachable = Shift::new(7, -7);
    assert_eq!(recovered(reachable, &options), reachable);

    let beyond = Shift::new(40, 0);
    assert_ne!(recovered(beyond, &options), beyond);
}

#[test]
fn both_shrink_conventions_recover_the_same_translation() {
    let offset = Shift::new(-9, 6);

    for shrink in [Shrink::Average, Shrink::Subsample] {
        let options = Options {
            shrink,
            ..Options::default()
        };

        assert_eq!(recovered(offset, &options), offset, "{shrink:?}");
    }
}

/// Noise moves pixels across the threshold; the exclusion band drops the ones
/// close enough for that.
#[test]
fn noise_does_not_move_the_answer() {
    let reference = window(WIDTH, HEIGHT, Shift::ZERO);
    let offset = Shift::new(-17, 11);

    for amplitude in [2, 5, 10] {
        let target = noisy(&window(WIDTH, HEIGHT, offset), amplitude, 0x51ED);

        assert_eq!(
            shift(&reference, &target, &Options::default()),
            offset,
            "noise of ±{amplitude} broke the alignment"
        );
    }
}

/// The claim the whole method rests on: a monotonic tone change cannot reorder
/// the population, so the median, and the bitmap, do not move.
#[test]
fn a_change_of_exposure_does_not_move_the_answer() {
    let reference = window(WIDTH, HEIGHT, Shift::ZERO);
    let offset = Shift::new(23, -19);

    for stops in [-2.0, -1.0, -0.5, 0.5, 1.0] {
        let target = reexpose(&window(WIDTH, HEIGHT, offset), stops);

        assert_eq!(
            shift(&reference, &target, &Options::default()),
            offset,
            "{stops:+} stops broke the alignment"
        );
    }
}

#[test]
fn noise_and_a_change_of_exposure_together_do_not_move_the_answer() {
    let reference = noisy(&window(WIDTH, HEIGHT, Shift::ZERO), 3, 0xA11E);
    let offset = Shift::new(-8, -22);
    let target = noisy(&reexpose(&window(WIDTH, HEIGHT, offset), -1.0), 3, 0xB0B);

    assert_eq!(shift(&reference, &target, &Options::default()), offset);
}

/// Ward's Figure 3: an area on the threshold decided by the sensor, not the
/// scene. It does not move with the camera, so it pulls the answer to zero.
/// Tolerance is the only difference between the two halves.
#[test]
fn the_exclusion_band_rescues_an_exposure_with_a_noisy_plateau() {
    let offset = Shift::new(9, -6);
    let reference = window_with_fixed_pattern_plateau(WIDTH, HEIGHT, Shift::ZERO);
    let target = window_with_fixed_pattern_plateau(WIDTH, HEIGHT, offset);

    let excluding = Options {
        tolerance: 4,
        ..Options::default()
    };
    assert_eq!(
        shift(&reference, &target, &excluding),
        offset,
        "the noisy plateau should have been excluded"
    );

    let trusting = Options {
        tolerance: 0,
        ..Options::default()
    };
    assert_ne!(
        shift(&reference, &target, &trusting),
        offset,
        "without the exclusion band the plateau should have swamped the signal"
    );
}

proptest! {
    // Two 400x300 scenes and a six-level search per case: a sweep, not a proof.
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn any_offset_within_range_comes_back(x in -63i32..=63, y in -63i32..=63) {
        let offset = Shift::new(x, y);

        prop_assert_eq!(recovered(offset, &Options::default()), offset);
    }
}
