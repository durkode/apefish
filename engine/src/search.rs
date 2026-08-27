//! Search: choosing a move under time/depth constraints.

use std::sync::Arc;
use std::time::Duration;

use crate::basetypes::{MATE, Move, Score};
use crate::board::Position;
use crate::movegen::MoveGen;
use crate::transposition_table::{ActiveTranspositionTable, ScoreBound, TT};

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
    movegen: Arc<MoveGen>,
    tt: Arc<dyn TT>,
}

impl Searcher {

    pub fn new(movegen: Arc<MoveGen>, transposition_table: Arc<dyn TT>) -> Self {
        Searcher {
            movegen: movegen,
            tt: transposition_table
        }
    }


    // Main search entrypoint.
    // From this should branch into opening book, closing tables, or negamax.
    pub fn search(&mut self, pos: &mut Position, limits: &SearchLimits) -> SearchResult {

        // Run off constants for now.
        let depth = 4;
        let ply = 0;
        let alpha = -MATE; // Starting bounds for best move for current side
        let beta = MATE; // Starting bounds for best move for opponent side

        self.tt.new_search(); // Set the correct search iteration now we are starting a new search

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
    // Alpha: best score for active side achieved so far.
    // Beta: best score for opponent achieved so far.
    fn negamax(&mut self, pos: &mut Position, depth: u8, ply: usize, mut alpha: Score, beta: Score) -> (Score, Option<Move>) {
        if depth == 0 {
            return (pos.evaluate(), None)
        }

        // Check the TT to see if this position is stored already
        let mut tt_move = None;
        if let Some(tt_hit) = self.tt.fetch(pos.get_zobrist(), ply) {
            tt_move = Some(tt_hit.mv);
            if tt_hit.depth >= depth {
                // This position has already been searched at a depth >= requested depth
                // Return early if possible
                match tt_hit.bound {
                    ScoreBound::Exact => return (tt_hit.score, tt_move),
                    ScoreBound::Lower if tt_hit.score >= beta => return (tt_hit.score, tt_move),
                    ScoreBound::Upper if tt_hit.score <= alpha => return (tt_hit.score, tt_move),
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
            let score = -1 * self.negamax(pos, depth - 1, ply + 1, -beta, -alpha).0;
            pos.unmake_move();

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
                return (-MATE + ply as Score, None)
            } else {
                return (0, None)
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

        (best_score, best_move)
    }
}
