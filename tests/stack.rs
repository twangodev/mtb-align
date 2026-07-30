//! Aligning a whole bracket: five frames a stop apart, each from a slightly
//! different position.

mod common;

use common::{reexpose, window};
use mtb_align::{Gray, Options, Shift, align_stack, common_crop};

const WIDTH: usize = 400;
const HEIGHT: usize = 300;

/// Where each frame was pointing.
const HANDHELD: [Shift; 5] = [
    Shift { x: 0, y: 0 },
    Shift { x: 3, y: -2 },
    Shift { x: -4, y: 5 },
    Shift { x: 9, y: 7 },
    Shift { x: -6, y: -11 },
];

/// Each frame moved by the shake above and metered a stop from the last.
fn bracket() -> Vec<Gray> {
    HANDHELD
        .iter()
        .enumerate()
        .map(|(index, &offset)| {
            let stops = index as f64 - 2.0;
            reexpose(&window(WIDTH, HEIGHT, offset), stops)
        })
        .collect()
}

#[test]
fn a_stack_of_one_never_moves() {
    let single = vec![window(WIDTH, HEIGHT, Shift::ZERO)];

    assert_eq!(align_stack(&single, 0, &Options::default()), [Shift::ZERO]);
}

#[test]
fn every_frame_is_placed_against_the_reference() {
    let shifts = align_stack(&bracket(), 0, &Options::default());

    assert_eq!(shifts, HANDHELD, "the first frame is the reference");
}

/// A different reference moves every offset by the same constant.
#[test]
fn the_reference_can_sit_anywhere_in_the_stack() {
    let stack = bracket();

    for reference in 0..HANDHELD.len() {
        let shifts = align_stack(&stack, reference, &Options::default());
        let expected: Vec<Shift> = HANDHELD
            .iter()
            .map(|offset| {
                Shift::new(
                    offset.x - HANDHELD[reference].x,
                    offset.y - HANDHELD[reference].y,
                )
            })
            .collect();

        assert_eq!(shifts, expected, "with frame {reference} as the reference");
        assert_eq!(shifts[reference], Shift::ZERO);
    }
}

/// Line the frames up, then keep only what all five saw.
#[test]
fn the_aligned_stack_shares_a_crop_every_frame_covers() {
    let stack = bracket();
    let shifts = align_stack(&stack, 0, &Options::default());

    let crop = common_crop(&shifts, WIDTH, HEIGHT);
    assert_eq!(
        crop,
        mtb_align::Rect {
            x: 9,
            y: 7,
            width: WIDTH - 9 - 6,
            height: HEIGHT - 7 - 11,
        }
    );

    // Inside the crop, every aligned frame has real data rather than padding.
    for (frame, &shift) in stack.iter().zip(&shifts) {
        let aligned = frame.shifted(shift);

        for y in [crop.y, crop.y + crop.height - 1] {
            for x in [crop.x, crop.x + crop.width - 1] {
                assert_ne!(aligned.sample(x, y), 0, "padding at ({x}, {y})");
            }
        }
    }
}

#[test]
#[should_panic(expected = "no exposure 5 in a stack of 5")]
fn a_reference_outside_the_stack_is_rejected() {
    align_stack(&bracket(), 5, &Options::default());
}

#[test]
#[should_panic(expected = "no exposure 0 in a stack of 0")]
fn an_empty_stack_is_rejected() {
    align_stack(&[], 0, &Options::default());
}
