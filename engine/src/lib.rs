//! apefish-engine: the pure chess engine core (board, move generation, evaluation, search).
//!
//! This crate performs no I/O. Frontend adapters — local CLI, UCI, Lichess, and any
//! future chess server — live in separate crates and drive the [`Engine`] trait below;
//! none of them talk to board/movegen/search directly.

pub mod basetypes;
pub mod fen;
pub mod board;
pub mod eval;
pub mod movegen;
pub mod search;
mod zobrist;

use std::sync::Arc;

pub use basetypes::{GenericErr, InputMove, Move, Piece, PieceKind, Side, Square};
pub use board::{Position};

use crate::{fen::to_fen, movegen::MoveGen, zobrist::ZobristRandoms};
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
    fn make_move(&mut self, to_make: InputMove) -> Result<(), GenericErr>;

    // /// Whether the game has ended, and how.
    // fn game_status(&self) -> GameStatus;

    // /// Search from the current position under the given limits and return the result.
    // fn go(&mut self, limits: SearchLimits) -> SearchResult;

    // /// Ask an in-progress `go` to return its best move so far as soon as possible.
    // fn stop(&mut self);
}

/// The concrete apefish engine implementing [`Engine`].
/// Make pub now for debugging purposes
#[derive(Debug)]
pub struct Apefish {
    position: Position,
    movegen: MoveGen,
    zobrist_randoms: Arc<ZobristRandoms>
}

impl Apefish {
    pub fn new() -> Self {
        let zobrists = Arc::new(ZobristRandoms::new());
        Apefish { 
            position: Position::new(zobrists.clone()), 
            movegen:  MoveGen::init(),
            zobrist_randoms: zobrists,
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
        self.position = Position::new(self.zobrist_randoms.clone());
    }

    fn set_position(&mut self, fen: Option<&str>, moves: &[Move]) {
        if let Some(fen_string) = fen {
            self.position.fen_setup(fen_string).unwrap();
        }
        for m in moves {
            self.position.make_move(&self.movegen, *m).unwrap();
        }
    }

    fn fen(&self) -> String {
        to_fen(&self.position)
    }

    fn legal_moves(&mut self) -> Vec<Move> {
        self.movegen.pseudo_legal_moves(&self.position).into_iter().filter(|cm| {
            match self.position.make_move(&self.movegen, *cm) {
                Ok(m) => {
                    self.position.unmake_move();
                    true
                },
                Err(_) => false
            }
        }).collect()
    }

    fn make_move(&mut self, to_make: InputMove) -> Result<(), GenericErr> {
        let validated_move = to_make.to_internal_move(&self.position)?;
        println!("{validated_move:?}");
        self.position.make_move(&self.movegen, validated_move)?;
        Ok(())
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
