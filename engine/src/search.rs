//! Search: choosing a move under time/depth constraints.

use std::any;
use std::sync::Arc;
use std::time::Duration;

use crate::basetypes::Move;
use crate::board::Position;
use crate::eval::Score;
use crate::movegen::MoveGen;

const MATE: Score = i32::MAX;

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


    // Main search entrypoint.
    // From this should branch into opening book, closing tables, or negamax.
    pub fn search(&mut self, pos: &mut Position, limits: &SearchLimits) -> SearchResult {

        // Run off constants for now.
        let depth = 3;
        let ply = 0;
        let alpha = -MATE; // Starting bounds for best move for current side
        let beta = MATE; // Starting bounds for best move for opponent side

        // For now, just delegate to Negamax. In future, add opening and closing books
        let (score, best_move) = self.negamax(pos, depth, ply, alpha, beta);

        SearchResult { 
            best_move, 
            score, 
            pv: vec![], 
            nodes: 1 
        }
    }

    // For now return move as well as score, remove move once we have that in the TT.
    fn negamax(&mut self, pos: &mut Position, depth: u8, ply: usize, mut alpha: Score, beta: Score) -> (Score, Option<Move>) {
        if depth == 0 {
            return (pos.evaluate(), None)
        }

        let mut best_move = None;
        let mut any_moves = false;
        for cm in self.movegen.pseudo_legal_moves(pos) {
            if pos.make_move(cm).is_err() {
                continue
            }
            any_moves = true;
            let score = self.negamax(pos, depth - 1, ply + 1, -beta, -alpha).0 * -1;
            pos.unmake_move();
            if score >= beta { return (beta, Some(cm)); }
            if score > alpha {
                alpha = score;
                best_move = Some(cm)
            }
        }

        if !any_moves {
            if pos.in_check() {
                return (-MATE, None)
            } else {
                return (0, None)
            }
        }

        (alpha, best_move)
    }
}
