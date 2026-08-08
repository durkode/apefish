// //! Move generation, legality, and game-end detection.

// use crate::basetypes::{Move, Side};
// use crate::board::Position;

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum GameStatus {
//     Ongoing,
//     /// The side that has been checkmated.
//     Checkmate(Side),
//     Stalemate,
//     DrawByRepetition,
//     DrawByFiftyMoveRule,
//     DrawByInsufficientMaterial,
// }

// /// Moves following piece movement rules, without filtering for king safety.
// pub fn pseudo_legal_moves(pos: &Position) -> Vec<Move> {
//     unimplemented!()
// }

// /// Legal moves for the side to move in this position.
// pub fn legal_moves(pos: &Position) -> Vec<Move> {
//     unimplemented!()
// }

// pub fn is_in_check(pos: &Position, side: Side) -> bool {
//     unimplemented!()
// }

// /// The game's current status, derived from `legal_moves` and draw conditions.
// pub fn game_status(pos: &Position) -> GameStatus {
//     unimplemented!()
// }
