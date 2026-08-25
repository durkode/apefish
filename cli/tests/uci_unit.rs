//! Tests for the UCI adapter's internals (move/limit parsing, command dispatch),
//! driven directly through `apefish_cli::uci`'s public functions.
//! See `uci_integration.rs` for the black-box test of the actual `--uci` binary.

use std::time::Duration;

use apefish_cli::uci::{handle_line, parse_go_limits, parse_uci_move, CommandOutcome};
use apefish_engine::{Apefish, Engine, PieceKind, Square};

#[test]
fn parse_uci_move_plain() {
    let mv = parse_uci_move("e2e4").expect("should parse");
    assert_eq!(mv.from, Square::E2);
    assert_eq!(mv.to, Square::E4);
    assert_eq!(mv.promotion, None);
}

#[test]
fn parse_uci_move_promotion() {
    let mv = parse_uci_move("e7e8q").expect("should parse");
    assert_eq!(mv.from, Square::E7);
    assert_eq!(mv.to, Square::E8);
    assert_eq!(mv.promotion, Some(PieceKind::Queen));
}

#[test]
fn parse_uci_move_too_short() {
    assert!(parse_uci_move("e2").is_none());
}

#[test]
fn parse_uci_move_garbage() {
    assert!(parse_uci_move("zz99").is_none());
}

#[test]
fn go_limits_depth() {
    let limits = parse_go_limits("depth 5");
    assert_eq!(limits.depth, Some(5));
    assert_eq!(limits.movetime, None);
}

#[test]
fn go_limits_movetime() {
    let limits = parse_go_limits("movetime 1500");
    assert_eq!(limits.movetime, Some(Duration::from_millis(1500)));
}

#[test]
fn go_limits_clock_params() {
    let limits = parse_go_limits("wtime 60000 btime 60000 winc 1000 binc 1000");
    assert_eq!(limits.wtime, Some(Duration::from_millis(60000)));
    assert_eq!(limits.btime, Some(Duration::from_millis(60000)));
    assert_eq!(limits.winc, Some(Duration::from_millis(1000)));
    assert_eq!(limits.binc, Some(Duration::from_millis(1000)));
}

#[test]
fn go_limits_ignores_unknown_tokens() {
    let limits = parse_go_limits("infinite ponder depth 3");
    assert_eq!(limits.depth, Some(3));
    assert_eq!(limits.movetime, None);
}

#[test]
fn handle_line_uci() {
    let mut engine = Apefish::new();
    match handle_line(&mut engine, "uci") {
        CommandOutcome::Continue(lines) => {
            assert_eq!(
                lines,
                vec![
                    "id name apefish".to_string(),
                    "id author Hamish Durkin".to_string(),
                    "uciok".to_string(),
                ]
            );
        }
        CommandOutcome::Quit => panic!("expected Continue"),
    }
}

#[test]
fn handle_line_isready() {
    let mut engine = Apefish::new();
    match handle_line(&mut engine, "isready") {
        CommandOutcome::Continue(lines) => assert_eq!(lines, vec!["readyok".to_string()]),
        CommandOutcome::Quit => panic!("expected Continue"),
    }
}

#[test]
fn handle_line_position_startpos_moves() {
    let mut engine = Apefish::new();
    handle_line(&mut engine, "position startpos moves e2e4 e7e5");
    assert_eq!(
        engine.fen(),
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
    );
}

#[test]
fn handle_line_position_fen_moves() {
    let mut engine = Apefish::new();
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    handle_line(&mut engine, &format!("position fen {fen} moves d2d4"));
    assert_eq!(
        engine.fen(),
        "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq d3 0 1"
    );
}

#[test]
fn handle_line_position_stops_on_illegal_move() {
    let mut engine = Apefish::new();
    handle_line(&mut engine, "position startpos moves e2e4 e2e4");
    // e2e4 applied once, then the second (now-illegal) e2e4 is rejected
    // without panicking and without altering the position further.
    assert_eq!(
        engine.fen(),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
}

#[test]
fn handle_line_go_returns_legal_move() {
    let mut engine = Apefish::new();
    let legal: Vec<String> = engine.legal_moves().iter().map(|m| m.to_string()).collect();
    match handle_line(&mut engine, "go depth 1") {
        CommandOutcome::Continue(lines) => {
            assert_eq!(lines.len(), 1);
            let mv = lines[0].strip_prefix("bestmove ").expect("bestmove prefix");
            assert!(legal.contains(&mv.to_string()));
        }
        CommandOutcome::Quit => panic!("expected Continue"),
    }
}

#[test]
fn handle_line_go_no_legal_moves() {
    let mut engine = Apefish::new();
    // Fool's mate: black has just delivered checkmate.
    handle_line(&mut engine, "position startpos moves f2f3 e7e5 g2g4 d8h4");
    match handle_line(&mut engine, "go") {
        CommandOutcome::Continue(lines) => assert_eq!(lines, vec!["bestmove 0000".to_string()]),
        CommandOutcome::Quit => panic!("expected Continue"),
    }
}

#[test]
fn handle_line_quit() {
    let mut engine = Apefish::new();
    match handle_line(&mut engine, "quit") {
        CommandOutcome::Quit => {}
        CommandOutcome::Continue(_) => panic!("expected Quit"),
    }
}

#[test]
fn handle_line_unknown_command_is_noop() {
    let mut engine = Apefish::new();
    let before = engine.fen();
    match handle_line(&mut engine, "foobar") {
        CommandOutcome::Continue(lines) => assert!(lines.is_empty()),
        CommandOutcome::Quit => panic!("expected Continue"),
    }
    assert_eq!(engine.fen(), before);
}
