//! Search: choosing a move under time/depth constraints.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration};

use crate::basetypes::{MATE, Move, Score};
use crate::board::Position;
use crate::movegen::MoveGen;
use crate::time_management::TimeCutoffs;
use crate::transposition_table::{ScoreBound, TT};

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

#[derive(Debug, Clone)]
pub struct NegamaxResult {
    pub best_move: Option<Move>,
    pub score: Score,
}

#[derive(Debug)]
pub struct Searcher {
    movegen: Arc<MoveGen>,
    tt: Arc<dyn TT>,
    stop: Arc<AtomicBool>,
}

impl Searcher {

    pub fn new(movegen: Arc<MoveGen>, transposition_table: Arc<dyn TT>, stop: Arc<AtomicBool>) -> Self {
        Searcher {
            movegen: movegen,
            tt: transposition_table,
            stop: stop
        }
    }


    // Main search entrypoint.
    // From this should branch into opening book, closing tables, or negamax.
    pub fn search(&mut self, pos: &mut Position, limits: &SearchLimits) -> SearchResult {
        // Clear the stop flag from previous search
        self.stop.store(false, Relaxed);
        
        // For now just call iterative deepening. In future add opening book calls to here as well
        self.iterative_deepening(pos, limits)
    }

    fn iterative_deepening(&mut self, pos: &mut Position, limits: &SearchLimits) -> SearchResult {
        // For now, no limit means 1. In future this should be an extremely large number.
        let max_depth = limits.depth.unwrap_or(u8::MAX);
        let ply = 0;
        let alpha = -MATE; // Starting bounds for best move for current side
        let beta = MATE; // Starting bounds for best move for opponent side

        let time_cutoffs = TimeCutoffs::from_search_limit(limits, pos.side_to_move());

        self.tt.new_search(); // Set the correct search iteration now we are starting a new search

        let mut score: Score = 0;
        let mut best_move = None;
        let mut nodes_searched = 0; // Running total mutated from within negamax search
        
        for depth in 0..max_depth {
            if let Some(ref cutoffs) = time_cutoffs {
                if cutoffs.exceeded_soft() {
                    break;
                }
            }
            if let Some(search_result) = self.negamax(pos, depth, ply, alpha, beta, time_cutoffs.as_ref(), &mut nodes_searched) {
                score = search_result.score;
                best_move = search_result.best_move;
            }
        }

        // If we can't check to any depth, just return the first legal move to remain valid
        if best_move.is_none() {
            best_move = pos.legal_moves().first().copied();
        }

        SearchResult { 
            best_move, 
            score, 
            pv: vec![], 
            nodes: nodes_searched
        }
    }

    // For now return move as well as score, remove move once we have that in the TT.
    // Alpha: best score for active side achieved so far.
    // Beta: best score for opponent achieved so far.
    // Returns:
    //   - score
    //   - best_move
    //   - nodes_searched
    //   - Search complete (not aborted)
    fn negamax(&mut self, pos: &mut Position, depth: u8, ply: usize, mut alpha: Score, beta: Score, cutoffs: Option<&TimeCutoffs>, nodes_searched: &mut u64) -> Option<NegamaxResult> {
        *nodes_searched += 1;

        // Check if search is aborted or timed out
        // Only do every 2048 nodes as no need for more often
        if *nodes_searched & 2047 == 0 {
            if let Some(cutoffs) = cutoffs {
                if cutoffs.exceeded_hard() {
                    return None
                }
            }
            if self.stop.load(Ordering::Relaxed) {
                return None
            }
        }

        if depth == 0 {
            return Some(NegamaxResult {best_move: None, score: pos.evaluate()})
        }

        // Check the TT to see if this position is stored already
        let mut tt_move = None;
        if let Some(tt_hit) = self.tt.fetch(pos.get_zobrist(), ply) {
            tt_move = Some(tt_hit.mv);
            if tt_hit.depth >= depth {
                // This position has already been searched at a depth >= requested depth
                // Return early if possible
                match tt_hit.bound {
                    ScoreBound::Exact => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    ScoreBound::Lower if tt_hit.score >= beta => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    ScoreBound::Upper if tt_hit.score <= alpha => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    _ => {}
                }
            }
        }

        let starting_alpha = alpha;
        let mut best_score = Score::MIN;
        let mut best_move = None;

        let mut pseudo_legal_moves = self.movegen.pseudo_legal_moves(pos);
        // If present, take the next move retrieved from the TT and make sure it is searched first
        if let Some(m) = tt_move {
            if let Some(i) = pseudo_legal_moves.iter().position(|&x| x == m) {
                pseudo_legal_moves.swap(0, i);
            }
        }

        for cm in pseudo_legal_moves {
            // Make move and get score
            if pos.make_move(cm).is_err() {
                continue
            }
            let subsearch = self.negamax(pos, depth - 1, ply + 1, -beta, -alpha, cutoffs, nodes_searched);
            pos.unmake_move();

            if subsearch.is_none() {
                // Search was aborted or timed out
                return None
            }

            let score = -1 * subsearch.unwrap().score;

            // Use fail-soft version of alpha beta pruning.
            if score > best_score {
                best_score = score;
                best_move = Some(cm);
            }
            if best_score > alpha {
                alpha = best_score;
            }
            if alpha >= beta {
                break;
            }
        }

        // TODO: we don't currently check for repetitions, do we need to? Unsure right now.
        // Not worried about sorting for insufficient material as assume end game tables will be used.
        if best_move.is_none() {
            // No move found, so game is over
            if pos.in_check() {
                // Need to add ply in order to get the quickest path to checkmate (penalise longer paths)
                return Some(NegamaxResult{best_move: None, score: -MATE + ply as Score})
            } else {
                return Some(NegamaxResult { best_move: None, score: 0});
            }
        } else {
            // Store the Score + Move in the TT
            // The bound is based on the original alpha when invoking the function,
            // not what it was refined to during execution.
            let bound = if best_score >= beta { ScoreBound::Lower }
                        else if best_score > starting_alpha { ScoreBound::Exact }
                        else { ScoreBound::Upper };
            self.tt.store(
                pos.get_zobrist(),
                best_move.unwrap(),
                best_score,
                bound,
                depth,
                ply as i32,
            );
        }

        Some(NegamaxResult{best_move: best_move, score: best_score})
    }
}
