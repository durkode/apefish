//! apefish-engine: the pure chess engine core (board, move generation, evaluation, search).
//!
//! This crate performs no I/O. Frontend adapters — local CLI, UCI, Lichess, and any
//! future chess server — live in separate crates and drive the [`Engine`] trait below;
//! none of them talk to board/movegen/search directly.

pub mod basetypes;
pub mod fen;
pub mod board;
pub mod eval;
pub mod psqt;
pub mod movegen;
pub mod search;
mod zobrist;
mod multiset;
mod phase;
#[cfg(test)]
mod tests;
mod transposition_table;
mod time_management;

use std::{sync::Arc};

pub use basetypes::{GenericErr, UnvalidatedMove, Move, Piece, PieceKind, Side, Square};
pub use board::{Position};

use crate::{basetypes::{DrawReason, GameStatus, WinReason}, fen::to_fen, movegen::MoveGen, search::{SearchLimits, SearchResult, Searcher}, transposition_table::{ActiveTranspositionTable, NoopTranspositionTable, TT}, zobrist::ZobristRandoms};
// pub use movegen::GameStatus;
// pub use search::{SearchLimits, SearchResult};

/// The interface every frontend adapter (local CLI, UCI, Lichess, ...) is built against.
pub trait Engine {
    /// Reset to a fresh game at the standard starting position.
    fn new_game(&mut self);

    /// Set the current position, optionally from a FEN (default: startpos), then
    /// apply `moves` in order.
    fn set_position(&mut self, fen: Option<&str>, moves: &[Move]);

    // Get a fen string representing the current position
    fn fen(&self) -> String;

    /// Legal moves for the side to move in the current position.
    fn legal_moves(&mut self) -> Vec<Move>;

    // Make move
    fn make_move(&mut self, to_make: UnvalidatedMove) -> Result<(), GenericErr>;

    /// Undo the most recent `make_move`, restoring the prior position exactly.
    fn unmake_move(&mut self);

    /// Whether the game has ended, and how.
    fn game_status(&mut self) -> GameStatus;

    /// Search from the current position under the given limits and return the result.
    fn go(&mut self, limits: SearchLimits) -> SearchResult;

    /// Ask an in-progress `go` to return its best move so far as soon as possible.
    fn stop(&mut self);
}

/// The concrete apefish engine implementing [`Engine`].
/// Make pub now for debugging purposes
#[derive(Debug)]
pub struct Apefish {
    position: Position,
    movegen: Arc<MoveGen>,
    zobrist_randoms: Arc<ZobristRandoms>,
    searcher: Searcher,
}

impl Apefish {
    pub fn new(tt_size_in_mb: usize) -> Self {
        let zobrists = Arc::new(ZobristRandoms::new());
        let movegen = Arc::new(MoveGen::init());
        let tt: Arc<dyn TT> = match tt_size_in_mb {
            0 => Arc::new(NoopTranspositionTable),
            _ => Arc::new(ActiveTranspositionTable::new(16)),
        };
        let searcher = Searcher::new(movegen.clone(), tt);
        let pos = Position::new(zobrists.clone(), movegen.clone());
        Apefish { 
            position: pos, 
            movegen:  movegen,
            zobrist_randoms: zobrists,
            searcher: searcher,
        }
    }

    pub fn print_debug_state(&self) {
        self.position.print_debug_state();
    }

    pub fn print_board(&self) {
        self.position.print_board();
    }
}

impl Engine for Apefish {
    fn new_game(&mut self) {
        self.position = Position::new(self.zobrist_randoms.clone(), self.movegen.clone());
    }

    fn set_position(&mut self, fen: Option<&str>, moves: &[Move]) {
        if let Some(fen_string) = fen {
            self.position.fen_setup(fen_string).unwrap();
        }
        for m in moves {
            self.position.make_move(*m).unwrap();
        }
    }

    fn fen(&self) -> String {
        to_fen(&self.position)
    }

    fn legal_moves(&mut self) -> Vec<Move> {
        self.position.legal_moves()
    }

    fn make_move(&mut self, to_make: UnvalidatedMove) -> Result<(), GenericErr> {
        let validated_move = self.position.validate_move(to_make)?;
        self.position.make_move(validated_move)?;
        Ok(())
    }

    fn unmake_move(&mut self) {
        self.position.unmake_move();
    }

    fn game_status(&mut self) -> GameStatus {
        if self.legal_moves().is_empty() {
             if self.position.in_check() {
                return GameStatus::Won { side: self.position.side_to_move().other(), reason: WinReason::Checkmate };
             }
             return GameStatus::Drawn { reason: DrawReason::Stalemate };
        };

        if self.position.half_move_clock() >= 100 {
            return GameStatus::Drawn { reason: DrawReason::FiftyMoveRule };
        };

        if self.position.times_position_reached() >= 3 {
            return GameStatus::Drawn { reason: DrawReason::ThreefoldRepetition };
        };

        if self.position.insufficient_material() {
            return GameStatus::Drawn { reason: DrawReason::InsufficientMaterial };
        };

        GameStatus::Ongoing
    }

    fn go(&mut self, limits: SearchLimits) -> SearchResult {
        // Return the first legal move just to get it operational before we actually search.
        // let best_move = self.legal_moves().first().cloned();
        // SearchResult { 
        //     best_move: best_move, 
        //     score: 0, 
        //     pv: vec![], 
        //     nodes: 1, 
        // }
        self.searcher.search(&mut self.position, &limits)
    }

    fn stop(&mut self) {
        // Do nothing currently, but revisit when search is implemented
    }
}
