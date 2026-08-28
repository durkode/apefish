use std::{cmp::min, ops::Add, time::{Duration, Instant}};
use crate::{Side, search::SearchLimits};


/// When the current search must stop.
///
/// While `pondering` (searching on the opponent's clock) nothing is ever
/// exceeded; [`TimeCutoffs::ponder_hit`] leaves that mode and starts the clock.
/// A `None` limit means "no bound" — a pondered search with no clock, e.g.
/// `go ponder infinite`.
#[derive(Debug)]
pub struct TimeCutoffs {
    soft_limit: Option<Instant>,
    hard_limit: Option<Instant>,
    pondering: bool,
    /// Search start, so `ponder_hit` can measure how long was spent pondering.
    started_at: Instant,
}

impl TimeCutoffs {
    // TODO: inject Instant compatible object for testing purposes.
    /// `None` when there is nothing to manage: not pondering, and no clock or
    /// movetime given (e.g. `go`, `go depth N`, `go infinite`).
    pub fn from_search_limit(limits: &SearchLimits, active_side: Side) -> Option<Self> {
        let now = Instant::now();
        const OVERHEAD: Duration = Duration::from_millis(50);

        let (soft, hard) = if let Some(movetime) = limits.movetime {
            // movetime overrides all other time control settings
            let budget = movetime.saturating_sub(OVERHEAD).max(Duration::from_millis(1));
            (Some(now + budget), Some(now + budget))
        } else {
            let clock = match active_side {
                Side::White => limits.wtime.map(|t| (t, limits.winc)),
                Side::Black => limits.btime.map(|t| (t, limits.binc)),
            };
            match clock {
                // Some sensible defaults for time.
                // TODO: Tune this in the future. Potentially adjust limits on the fly based on how contentious next move is.
                // TODO: UCI also seems to support a "movestogo" with formats that use a move based time control.
                //       Add support for this later.
                Some((time_remaining, increment)) => {
                    let soft = time_remaining / 20
                        + increment.unwrap_or(Duration::from_millis(0)) * 3 / 4;
                    let hard = min(time_remaining / 2, soft * 4);
                    (Some(now.add(soft)), Some(now.add(hard)))
                }
                None => (None, None),
            }
        };

        if !limits.ponder && soft.is_none() {
            return None;
        }

        Some(TimeCutoffs { soft_limit: soft, hard_limit: hard, pondering: limits.ponder, started_at: now })
    }

    /// Opponent played the pondered move. Leave ponder mode; since the time
    /// already spent was on their clock, push both deadlines out by that much so
    /// the move keeps its full budget on top of the free search.
    pub fn ponder_hit(&mut self) {
        if !self.pondering {
            return;
        }
        self.pondering = false;
        let pondered = self.started_at.elapsed();
        if let Some(limit) = &mut self.soft_limit {
            *limit += pondered;
        }
        if let Some(limit) = &mut self.hard_limit {
            *limit += pondered;
        }
    }

    pub fn is_pondering(&self) -> bool {
        self.pondering
    }

    pub fn exceeded_soft(&self) -> bool {
        !self.pondering && self.soft_limit.is_some_and(|limit| Instant::now() > limit)
    }

    pub fn exceeded_hard(&self) -> bool {
        !self.pondering && self.hard_limit.is_some_and(|limit| Instant::now() > limit)
    }
}
