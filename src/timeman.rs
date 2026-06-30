//! Time management.
//!
//! Converts a clock state (time left, increment, moves until the next time
//! control) into a per-move budget in milliseconds. The policy is deliberately
//! simple and conservative so the engine never flags: spend a fraction of the
//! remaining time plus most of the increment, clamped to leave a safety margin.

/// A clock configuration for one side.
#[derive(Clone, Copy, Debug, Default)]
pub struct TimeControl {
    /// Milliseconds remaining on the clock.
    pub time_left_ms: u64,
    /// Increment added after each move, in milliseconds.
    pub increment_ms: u64,
    /// Moves remaining until the next time control, if known.
    pub moves_to_go: Option<u32>,
}

/// A small safety margin (ms) subtracted so we never overshoot the flag.
const OVERHEAD_MS: u64 = 50;

impl TimeControl {
    /// Compute how long to think about the current move, in milliseconds.
    ///
    /// * With a known `moves_to_go`, divide the remaining time evenly across
    ///   those moves (plus the increment).
    /// * Otherwise assume ~30 moves remain.
    /// * Always keep at least a small reserve and never return 0.
    pub fn budget_ms(&self) -> u64 {
        if self.time_left_ms == 0 {
            return self.increment_ms.saturating_sub(OVERHEAD_MS).max(1);
        }
        let moves = self.moves_to_go.unwrap_or(30).max(1) as u64;
        let usable = self.time_left_ms.saturating_sub(OVERHEAD_MS);
        let base = usable / moves;
        // Spend most of the increment on top of the share.
        let budget = base + (self.increment_ms * 3) / 4;
        // Never commit more than half of what's left in one move.
        let cap = usable / 2;
        budget.clamp(1, cap.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divides_time_across_moves() {
        let tc = TimeControl {
            time_left_ms: 60_000,
            increment_ms: 0,
            moves_to_go: Some(30),
        };
        let b = tc.budget_ms();
        assert!(b > 1500 && b < 2200, "got {}", b);
    }

    #[test]
    fn never_zero_and_capped() {
        let tc = TimeControl {
            time_left_ms: 100,
            increment_ms: 0,
            moves_to_go: None,
        };
        let b = tc.budget_ms();
        assert!(b >= 1);
        assert!(b <= 50); // capped to half of usable (which is tiny here)
    }

    #[test]
    fn uses_increment_when_flat() {
        let tc = TimeControl {
            time_left_ms: 0,
            increment_ms: 2000,
            moves_to_go: None,
        };
        assert!(tc.budget_ms() >= 1900);
    }
}
