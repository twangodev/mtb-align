use crate::Gray;

/// How far the target image has to move to sit on top of the reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Shift {
    pub x: i32,
    pub y: i32,
}

impl Shift {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Carries a shift from one pyramid level to the next finer one, where the
    /// same displacement spans twice as many pixels.
    pub fn doubled(self) -> Self {
        Self::new(self.x * 2, self.y * 2)
    }

    pub fn offset(self, x: i32, y: i32) -> Self {
        Self::new(self.x + x, self.y + y)
    }
}

/// A region of an image, in pixels from the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl Gray {
    /// Moves the image by `shift`, leaving zeros where it moved away from.
    ///
    /// Matches OpenCV's `shiftMat`, except that OpenCV cannot be asked for a
    /// shift larger than the image; here that simply returns an empty frame.
    pub fn shifted(&self, shift: Shift) -> Gray {
        let (width, height) = (self.width(), self.height());
        let mut moved = vec![0; width * height];

        for y in 0..height {
            let Some(source_y) = source(y, shift.y, height) else {
                continue;
            };

            for x in 0..width {
                if let Some(source_x) = source(x, shift.x, width) {
                    moved[y * width + x] = self.sample(source_x, source_y);
                }
            }
        }

        Gray::from_vec(moved, width, height)
    }
}

/// The region every exposure covers once each has been moved by its own shift.
///
/// This is what OpenCV's `cut` option trims to: outside it at least one frame
/// contributes nothing but the zeros it was padded with, which would show up in
/// a composite as a band of dead pixels along an edge.
pub fn common_crop(shifts: &[Shift], width: usize, height: usize) -> Rect {
    // The frame pushed furthest one way decides how much comes off that edge,
    // and a frame that did not move at all still holds its own edge in place,
    // which is why both ends start the fold at zero.
    let span = |extent: usize, of: fn(Shift) -> i32| {
        let furthest = |pick: fn(i32, i32) -> i32| {
            shifts
                .iter()
                .fold(0, |carried, &shift| pick(carried, of(shift)))
        };

        let start = furthest(i32::max).max(0) as usize;
        let lost_from_the_far_edge = furthest(i32::min).unsigned_abs() as usize;

        (
            start,
            extent
                .saturating_sub(lost_from_the_far_edge)
                .saturating_sub(start),
        )
    };

    let (x, width) = span(width, |shift| shift.x);
    let (y, height) = span(height, |shift| shift.y);

    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Where a destination pixel reads from, or `None` if that is off the image.
fn source(destination: usize, shift: i32, extent: usize) -> Option<usize> {
    let source = destination as i64 - shift as i64;

    (0..extent as i64)
        .contains(&source)
        .then_some(source as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counting(width: usize, height: usize) -> Gray {
        Gray::from_vec((1..=(width * height) as u8).collect(), width, height)
    }

    #[test]
    fn doubling_carries_a_shift_to_the_next_level_down() {
        assert_eq!(Shift::new(-3, 5).doubled(), Shift::new(-6, 10));
        assert_eq!(Shift::ZERO.doubled(), Shift::ZERO);
    }

    #[test]
    fn offsetting_moves_by_the_candidate_step() {
        assert_eq!(Shift::new(4, -2).offset(-1, 1), Shift::new(3, -1));
    }

    #[test]
    fn shifting_by_nothing_leaves_the_image_alone() {
        let image = counting(3, 2);

        assert_eq!(image.shifted(Shift::ZERO).as_slice(), image.as_slice());
    }

    /// Moving right and down pushes the top-left corner in and pads behind it.
    #[test]
    fn shifting_pads_the_edge_it_moved_away_from() {
        let shifted = counting(3, 2).shifted(Shift::new(1, 1));

        assert_eq!(shifted.as_slice(), &[0, 0, 0, 0, 1, 2]);
    }

    #[test]
    fn shifting_the_other_way_pads_the_far_edge() {
        let shifted = counting(3, 2).shifted(Shift::new(-1, 0));

        assert_eq!(shifted.as_slice(), &[2, 3, 0, 5, 6, 0]);
    }

    /// OpenCV would reject this outright, so there is no convention to match.
    #[test]
    fn shifting_further_than_the_image_leaves_nothing_behind() {
        let shifted = counting(3, 2).shifted(Shift::new(9, -9));

        assert_eq!(shifted.as_slice(), &[0; 6]);
    }

    #[test]
    fn a_stack_that_never_moved_keeps_the_whole_frame() {
        let crop = common_crop(&[Shift::ZERO, Shift::ZERO], 100, 80);

        assert_eq!(
            crop,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 80
            }
        );
        assert!(!crop.is_empty());
    }

    /// One frame moved three right, so the leftmost three columns are padding
    /// in that frame and cannot be part of a composite.
    #[test]
    fn a_positive_shift_trims_the_leading_edge() {
        assert_eq!(
            common_crop(&[Shift::ZERO, Shift::new(3, 2)], 100, 80),
            Rect {
                x: 3,
                y: 2,
                width: 97,
                height: 78
            }
        );
    }

    #[test]
    fn a_negative_shift_trims_the_trailing_edge() {
        assert_eq!(
            common_crop(&[Shift::ZERO, Shift::new(-4, -1)], 100, 80),
            Rect {
                x: 0,
                y: 0,
                width: 96,
                height: 79
            }
        );
    }

    /// Shifts either side of zero eat into both edges at once.
    #[test]
    fn shifts_in_both_directions_trim_both_edges() {
        assert_eq!(
            common_crop(
                &[Shift::new(5, 0), Shift::new(-4, -1), Shift::new(2, 3)],
                100,
                80
            ),
            Rect {
                x: 5,
                y: 3,
                width: 91,
                height: 76
            }
        );
    }

    #[test]
    fn a_stack_that_moved_further_than_it_is_wide_has_nothing_in_common() {
        let crop = common_crop(&[Shift::new(-60, 0), Shift::new(60, 0)], 100, 80);

        assert!(crop.is_empty(), "{crop:?}");
    }
}
