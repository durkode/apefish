//! apefish-cli: local play. A terminal REPL for a human to play a game against
//! apefish directly, with no network involved — the same `Engine` trait that
//! this drives is what the UCI and Lichess adapters will drive later.

// use apefish_engine::{Apefish, Move};

use apefish_engine::Apefish;

fn main() {
    let af = Apefish::new();
    af.print_debug_state();
}

// /// Render the current position to the terminal.
// fn print_board(_engine: &Apefish) {
//     unimplemented!()
// }

// /// Block until the human enters a legal move (algebraic or UCI notation), returning it.
// fn read_human_move(_engine: &Apefish) -> Move {
//     unimplemented!()
// }

// /// Time/depth budget given to the engine for its reply in local play.
// fn local_search_limits() -> SearchLimits {
//     unimplemented!()
// }

// /// Print the game result once `status` is no longer `Ongoing`.
// fn print_result(status: GameStatus) {
//     unimplemented!()
// }
