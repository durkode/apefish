use core::time;
use std::{cmp::min, ops::Add, time::{Duration, Instant}};

use crate::{Side, search::SearchLimits};


pub struct TimeCutoffs {
    pub soft_limit: Instant,
    pub hard_limit: Instant
}

impl TimeCutoffs {
    // TODO: inject Instant compatible object for testing purposes.
    pub fn from_search_limit(limits: &SearchLimits, active_side: Side) -> Option<Self> {
        let now = Instant::now();

        let (time_remaining, increment) = match active_side {
            Side::White => {
                if limits.wtime.is_none() {
                    return None
                }
                (limits.wtime.unwrap(), limits.winc)
            },
            Side::Black => {
                if limits.btime.is_none() {
                    return None
                }
                (limits.btime.unwrap(), limits.btime)
            }
        };

        // Some sensible defaults for time. 
        // TODO: Tune this in the future
        // TODO: UCI also seems to support a "movestogo" with formats that use a move based time control.
        //       Add support for this later.
        let soft_limit = time_remaining / 20 + increment.unwrap_or(Duration::from_millis(0)) * 3/4;
        let hard_limit = min(time_remaining / 2, soft_limit * 4);

        Some(TimeCutoffs { soft_limit: now.add(soft_limit), hard_limit: now.add(hard_limit) })

    }

    pub fn exceeded_soft(&self) -> bool {
        Instant::now() > self.soft_limit
    }

    pub fn exceeded_hard(&self) -> bool {
        Instant::now() > self.hard_limit
    }
}