//! apefish-cli: local play. A terminal REPL for a human to play a game against
//! apefish directly, with no network involved — the same `Engine` trait that
//! this drives is what the UCI and Lichess adapters will drive later.

// use apefish_engine::{Apefish, Move};

use apefish_engine::{Apefish, Engine, InputMove, Square};

fn main() {
    if std::env::args().skip(1).any(|a| a == "--uci") {
        apefish_cli::uci::run();
        return;
    }

    let mut af = Apefish::new();

    let moves: Vec<InputMove> = vec![
        InputMove{
            from: Square::E2,
            to: Square::E4,
            promotion: None
        },
        InputMove{
            from: Square::D7,
            to: Square::D5,
            promotion: None
        },
        InputMove{
            from: Square::F1,
            to: Square::A6,
            promotion: None
        },
        InputMove{
            from:Square::B7,
            to: Square::A6,
            promotion: None
        }
    ];

    for m in moves {
        af.print_debug_state();
        println!("\n\nMove: {m:?}");
        af.make_move(m).unwrap();
        // af.print_board();
    }

    af.print_debug_state();
    // for m in af.legal_moves() {
    //     println!("{m}");
    // }

    let fen_out = af.fen();
    println!("FEN: {fen_out}");

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
