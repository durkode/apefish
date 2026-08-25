//! Search: choosing a move under time/depth constraints.

use std::time::Duration;

use crate::basetypes::Move;
use crate::board::Position;
use crate::eval::Score;

/// Constraints on a single search call. All fields optional; interpretation
/// (e.g. how clock time maps to a time budget) is up to the search implementation.
#[derive(Debug, Clone, Default)]
pub struct SearchLimits {
    pub depth: Option<u8>,
    pub movetime: Option<Duration>,
    pub wtime: Option<Duration>,
    pub btime: Option<Duration>,
    pub winc: Option<Duration>,
    pub binc: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: Score,
    /// Principal variation, starting with `best_move`.
    pub pv: Vec<Move>,
    pub nodes: u64,
}

pub fn search(pos: &Position, limits: &SearchLimits) -> SearchResult {
    unimplemented!()
}
