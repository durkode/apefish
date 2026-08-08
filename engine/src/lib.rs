//! apefish-engine: the pure chess engine core (board, move generation, evaluation, search).
//!
//! This crate performs no I/O. Frontend adapters — local CLI, UCI, Lichess, and any
//! future chess server — live in separate crates and drive the [`Engine`] trait below;
//! none of them talk to board/movegen/search directly.

pub mod basetypes;
pub mod board;
pub mod eval;
pub mod movegen;
pub mod search;

pub use basetypes::{Move, Piece, PieceType, Side, Square};
pub use board::{FenError, Position};
// pub use movegen::GameStatus;
// pub use search::{SearchLimits, SearchResult};

/// The interface every frontend adapter (local CLI, UCI, Lichess, ...) is built against.
pub trait Engine {
    /// Reset to a fresh game at the standard starting position.
    fn new_game(&mut self);

    /// Set the current position, optionally from a FEN (default: startpos), then
    /// apply `moves` in order.
    fn set_position(&mut self, fen: Option<&str>, moves: &[Move]);

    /// Legal moves for the side to move in the current position.
    fn legal_moves(&self) -> Vec<Move>;

    // /// Whether the game has ended, and how.
    // fn game_status(&self) -> GameStatus;

    // /// Search from the current position under the given limits and return the result.
    // fn go(&mut self, limits: SearchLimits) -> SearchResult;

    // /// Ask an in-progress `go` to return its best move so far as soon as possible.
    // fn stop(&mut self);
}

/// The concrete apefish engine implementing [`Engine`].
#[derive(Debug)]
pub struct Apefish {
    position: Position
}

// Make pub now for debugging purposes
impl Apefish {
    pub fn new() -> Self {
        Apefish { position: Position::new() }
    }

    pub fn print_debug_state(&self) {
        self.position.print_state();
    }
}

impl Engine for Apefish {
    fn new_game(&mut self) {
        unimplemented!()
    }

    fn set_position(&mut self, _fen: Option<&str>, _moves: &[Move]) {
        unimplemented!()
    }

    fn legal_moves(&self) -> Vec<Move> {
        unimplemented!()
    }

    // fn game_status(&self) -> GameStatus {
    //     unimplemented!()
    // }

    // fn go(&mut self, limits: SearchLimits) -> SearchResult {
    //     unimplemented!()
    // }

    // fn stop(&mut self) {
    //     unimplemented!()
    // }
}
