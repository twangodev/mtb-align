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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubling_carries_a_shift_to_the_next_level_down() {
        assert_eq!(Shift::new(-3, 5).doubled(), Shift::new(-6, 10));
        assert_eq!(Shift::ZERO.doubled(), Shift::ZERO);
    }

    #[test]
    fn offsetting_moves_by_the_candidate_step() {
        assert_eq!(Shift::new(4, -2).offset(-1, 1), Shift::new(3, -1));
    }
}
