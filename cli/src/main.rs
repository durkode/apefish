//! apefish-cli: local play. A terminal REPL for a human to play a game against
//! apefish directly, with no network involved — the same `Engine` trait that
//! this drives is what the UCI and Lichess adapters will drive later.

// use apefish_engine::{Apefish, Move};

use apefish_engine::{Apefish, Engine, UnvalidatedMove, Square};

use apefish_cli::uci::DEFAULT_HASH_MB;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--uci") {
        let hash_mb = match parse_hash_arg(&args) {
            Ok(mb) => mb,
            Err(msg) => {
                eprintln!("apefish: {msg}");
                std::process::exit(2);
            }
        };
        apefish_cli::uci::run(hash_mb);
        return;
    }

    let mut af = Apefish::new(0);

    let moves: Vec<UnvalidatedMove> = vec![
        UnvalidatedMove{
            from: Square::E2,
            to: Square::E4,
            promotion: None
        },
        UnvalidatedMove{
            from: Square::D7,
            to: Square::D5,
            promotion: None
        },
        UnvalidatedMove{
            from: Square::F1,
            to: Square::A6,
            promotion: None
        },
        UnvalidatedMove{
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

/// Parse an optional `--hash <MB>` / `--hash=<MB>` flag, returning
/// [`DEFAULT_HASH_MB`] when it is absent. `0` disables the transposition table.
fn parse_hash_arg(args: &[String]) -> Result<usize, String> {
    for (i, arg) in args.iter().enumerate() {
        let raw = if let Some(val) = arg.strip_prefix("--hash=") {
            Some(val.to_string())
        } else if arg == "--hash" {
            Some(
                args.get(i + 1)
                    .ok_or_else(|| "--hash requires a value in MB".to_string())?
                    .clone(),
            )
        } else {
            None
        };
        if let Some(raw) = raw {
            return raw
                .parse::<usize>()
                .map_err(|_| format!("invalid --hash value: {raw}"));
        }
    }
    Ok(DEFAULT_HASH_MB)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn hash_arg_defaults_when_absent() {
        assert_eq!(parse_hash_arg(&args(&["--uci"])), Ok(DEFAULT_HASH_MB));
    }

    #[test]
    fn hash_arg_space_form() {
        assert_eq!(parse_hash_arg(&args(&["--uci", "--hash", "128"])), Ok(128));
    }

    #[test]
    fn hash_arg_equals_form() {
        assert_eq!(parse_hash_arg(&args(&["--hash=64", "--uci"])), Ok(64));
    }

    #[test]
    fn hash_arg_rejects_non_numeric() {
        assert!(parse_hash_arg(&args(&["--uci", "--hash", "abc"])).is_err());
    }

    #[test]
    fn hash_arg_rejects_missing_value() {
        assert!(parse_hash_arg(&args(&["--uci", "--hash"])).is_err());
    }
}
