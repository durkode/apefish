//! Search: choosing a move under time/depth constraints.

use std::sync::Arc;
use std::time::Duration;

use crate::basetypes::Move;
use crate::board::Position;
use crate::eval::Score;
use crate::movegen::MoveGen;

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

#[derive(Debug)]
pub struct Searcher {
    movegen: Arc<MoveGen>
}

impl Searcher {

    pub fn new(movegen: Arc<MoveGen>) -> Self {
        Searcher { movegen: movegen }
    }

    pub fn search(&mut self, pos: &mut Position, limits: &SearchLimits) -> SearchResult {

        for mc in self.movegen.pseudo_legal_moves(pos) {
            if let Ok(_) = pos.make_move(mc) {
                pos.unmake_move();
                return SearchResult { best_move: Some(mc), score: 0, pv: vec![], nodes: 1 }
            }
        }

        SearchResult { 
            best_move: None, 
            score: 0, 
            pv: vec![], 
            nodes: 1, 
        }
    }
}
