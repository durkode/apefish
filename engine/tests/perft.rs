//! Move generation correctness tests, run purely against the public [`Engine`]
//! trait (`set_position`, `legal_moves`, `make_move`, `unmake_move`). Nothing
//! here touches `Position`, `MoveGen`, or any other internal type - if a
//! future change here needs to reach into engine internals, that's a sign
//! the `Engine` trait is missing a method, not a reason to import them.
//!
//! Positions and expected node counts are taken from two independently
//! verified, widely-used sources rather than hand-rolled, so the numbers
//! themselves are trustworthy:
//!
//! - The "Perft Results" position set - <https://www.chessprogramming.org/Perft_Results>:
//!   six positions with node counts at multiple depths, exercising ordinary
//!   move generation, sliding-piece blockers/pins, castling, en passant and
//!   promotions together.
//! - Martin Sedlak's move generator test positions -
//!   <https://www.talkchess.com/forum3/viewtopic.php?t=47318>: small
//!   positions that each isolate one specific rule - illegal en passant,
//!   en passant that discovers a check, castling through/out of check,
//!   castling rights lost via rook capture, promotion (including out of and
//!   into check), stalemate, and double check.
//!
//! ## Incremental perft
//!
//! Each node is reached by `make_move`-ing one ply deeper and `unmake_move`-ing
//! back out, rather than replaying the whole path from the root at every
//! node - fast enough that even the largest cases below run by default.
//!
//! ## Debugging a failure
//!
//! On a node-count mismatch, `assert_perft` auto-bisects against a local
//! Stockfish binary (`common::bisect_mismatch`) before failing, descending
//! ply by ply into the first diverging branch until it isolates the exact
//! node and move that disagrees. That trace is printed as part of the
//! failing test's output - `cargo test` alone is enough, no need to
//! separately copy the FEN into another tool. Requires `stockfish` on PATH
//! (or `STOCKFISH_BIN=/path/to/it`); if it's not found, bisection is
//! skipped with a note instead of failing the run.

use apefish_engine::Apefish;

mod common;
use common::checked_perft;

/// On a mismatch, auto-bisects against Stockfish (if installed - see
/// `common::bisect_mismatch`) before asserting, so the failing test's
/// captured output already contains the exact node/move that diverges
/// instead of just the top-level node count.
///
/// This also covers the case where the engine doesn't just miscount but
/// outright records something as an illegal move partway through (a
/// rejected `make_move`, or an internal panic a few plies downstream of an
/// earlier bad move): `checked_perft` catches that instead of letting it
/// panic straight out of the test, so it can still be routed into the same
/// bisect/dissect flow below rather than just printing a bare panic.
fn assert_perft(fen: &str, depth: u32, expected: u64) {
    let mut engine = Apefish::new();
    match checked_perft(&mut engine, fen, depth) {
        Ok(actual) => {
            if actual != expected {
                println!("perft mismatch - auto-bisecting against stockfish to find the exact divergence:\n");
                common::bisect_mismatch(&mut engine, fen, Vec::new(), depth);
            }
            assert_eq!(
                actual, expected,
                "perft({depth}) mismatch for fen `{fen}`: expected {expected}, got {actual}"
            );
        }
        Err(failure) => {
            println!(
                "perft({depth}) hit an engine failure partway through - the engine recorded an illegal \
                 move rather than just miscounting ({failure})\n\
                 auto-bisecting against stockfish to pin down the exact divergence:\n"
            );
            common::bisect_mismatch(&mut engine, fen, Vec::new(), depth);
            panic!("perft({depth}) for fen `{fen}` hit an engine failure: {failure}");
        }
    }
}

/// The six-position "Perft Results" suite: general move generation, blocker
/// combinations, pins, castling, en passant, and promotions.
/// Source: <https://www.chessprogramming.org/Perft_Results>
mod cpw_perft_results {
    use super::assert_perft;

    const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    const POSITION_3: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    const POSITION_4: &str = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
    const POSITION_4_MIRRORED: &str = "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1";
    const POSITION_5: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
    const POSITION_6: &str = "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10";

    #[test]
    fn startpos_depth_1() {
        assert_perft(STARTPOS, 1, 20);
    }
    #[test]
    fn startpos_depth_2() {
        assert_perft(STARTPOS, 2, 400);
    }
    #[test]
    fn startpos_depth_3() {
        assert_perft(STARTPOS, 3, 8_902);
    }
    #[test]
    fn startpos_depth_4() {
        assert_perft(STARTPOS, 4, 197_281);
    }
    #[test]
    fn startpos_depth_5() {
        assert_perft(STARTPOS, 5, 4_865_609);
    }

    #[test]
    fn kiwipete_depth_1() {
        assert_perft(KIWIPETE, 1, 48);
    }
    #[test]
    fn kiwipete_depth_2() {
        assert_perft(KIWIPETE, 2, 2_039);
    }
    #[test]
    fn kiwipete_depth_3() {
        assert_perft(KIWIPETE, 3, 97_862);
    }
    #[test]
    fn kiwipete_depth_4() {
        assert_perft(KIWIPETE, 4, 4_085_603);
    }

    #[test]
    fn position_3_depth_1() {
        assert_perft(POSITION_3, 1, 14);
    }
    #[test]
    fn position_3_depth_2() {
        assert_perft(POSITION_3, 2, 191);
    }
    #[test]
    fn position_3_depth_3() {
        assert_perft(POSITION_3, 3, 2_812);
    }
    #[test]
    fn position_3_depth_4() {
        assert_perft(POSITION_3, 4, 43_238);
    }
    #[test]
    fn position_3_depth_5() {
        assert_perft(POSITION_3, 5, 674_624);
    }

    #[test]
    fn position_4_depth_1() {
        assert_perft(POSITION_4, 1, 6);
    }
    #[test]
    fn position_4_depth_2() {
        assert_perft(POSITION_4, 2, 264);
    }
    #[test]
    fn position_4_depth_3() {
        assert_perft(POSITION_4, 3, 9_467);
    }
    #[test]
    fn position_4_depth_4() {
        assert_perft(POSITION_4, 4, 422_333);
    }

    #[test]
    fn position_4_mirrored_depth_1() {
        assert_perft(POSITION_4_MIRRORED, 1, 6);
    }
    #[test]
    fn position_4_mirrored_depth_2() {
        assert_perft(POSITION_4_MIRRORED, 2, 264);
    }
    #[test]
    fn position_4_mirrored_depth_3() {
        assert_perft(POSITION_4_MIRRORED, 3, 9_467);
    }

    #[test]
    fn position_5_depth_1() {
        assert_perft(POSITION_5, 1, 44);
    }
    #[test]
    fn position_5_depth_2() {
        assert_perft(POSITION_5, 2, 1_486);
    }
    #[test]
    fn position_5_depth_3() {
        assert_perft(POSITION_5, 3, 62_379);
    }
    #[test]
    fn position_5_depth_4() {
        assert_perft(POSITION_5, 4, 2_103_487);
    }

    #[test]
    fn position_6_depth_1() {
        assert_perft(POSITION_6, 1, 46);
    }
    #[test]
    fn position_6_depth_2() {
        assert_perft(POSITION_6, 2, 2_079);
    }
    #[test]
    fn position_6_depth_3() {
        assert_perft(POSITION_6, 3, 89_890);
    }
    #[test]
    fn position_6_depth_4() {
        assert_perft(POSITION_6, 4, 3_894_594);
    }
}

/// Martin Sedlak's targeted move generator test positions - each one isolates
/// a single tricky rule rather than mixing everything together like the
/// six-position suite above. Every case is given as a pair: the position and
/// its color-flipped mirror, both expected to produce the same node count.
/// Source: <https://www.talkchess.com/forum3/viewtopic.php?t=47318>
mod sedlak_movegen_positions {
    use super::assert_perft;

    /// Pawn must not be capturable en passant once it's no longer the move
    /// immediately after the double push.
    #[test]
    fn illegal_en_passant_capture_white() {
        assert_perft("8/5bk1/8/2Pp4/8/1K6/8/8 w - d6 0 1", 6, 824_064);
    }
    #[test]
    fn illegal_en_passant_capture_black() {
        assert_perft("8/8/1k6/8/2pP4/8/5BK1/8 b - d3 0 1", 6, 824_064);
    }

    /// An en passant capture can itself deliver check.
    #[test]
    fn en_passant_capture_checks_opponent_white() {
        assert_perft("8/5k2/8/2Pp4/2B5/1K6/8/8 w - d6 0 1", 6, 1_440_467);
    }
    #[test]
    fn en_passant_capture_checks_opponent_black() {
        assert_perft("8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1", 6, 1_440_467);
    }

    /// Castling out of / through check must be excluded even though the
    /// resulting position (after castling) would itself give check.
    #[test]
    fn short_castling_gives_check_white() {
        assert_perft("5k2/8/8/8/8/8/8/4K2R w K - 0 1", 6, 661_072);
    }
    #[test]
    fn short_castling_gives_check_black() {
        assert_perft("4k2r/8/8/8/8/8/8/5K2 b k - 0 1", 6, 661_072);
    }

    #[test]
    fn long_castling_gives_check_white() {
        assert_perft("3k4/8/8/8/8/8/8/R3K3 w Q - 0 1", 6, 803_711);
    }
    #[test]
    fn long_castling_gives_check_black() {
        assert_perft("r3k3/8/8/8/8/8/8/3K4 b q - 0 1", 6, 803_711);
    }

    /// Castling rights must be lost when a rook is captured on its home
    /// square, not just when the rook or king moves.
    #[test]
    fn castling_rights_lost_to_rook_capture_white() {
        assert_perft("r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1", 4, 1_274_206);
    }
    #[test]
    fn castling_rights_lost_to_rook_capture_black() {
        assert_perft("r3k2r/7b/8/8/8/8/1B4BQ/R3K2R b KQkq - 0 1", 4, 1_274_206);
    }

    /// Castling out of check and castling through an attacked square must
    /// both be excluded, even with full castling rights and clear squares.
    #[test]
    fn castling_prevented_by_attacked_squares_white() {
        assert_perft("r3k2r/8/5Q2/8/8/3q4/8/R3K2R w KQkq - 0 1", 4, 1_720_476);
    }
    #[test]
    fn castling_prevented_by_attacked_squares_black() {
        assert_perft("r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1", 4, 1_720_476);
    }

    /// Promotion is legal as a way of escaping check.
    #[test]
    fn promote_out_of_check_white() {
        assert_perft("2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1", 6, 3_821_001);
    }
    #[test]
    fn promote_out_of_check_black() {
        assert_perft("3K4/8/8/8/8/8/4p3/2k2R2 b - - 0 1", 6, 3_821_001);
    }

    /// Moving a pinned/blocking piece can uncover a check from a sliding
    /// piece behind it.
    #[test]
    fn discovered_check_white() {
        assert_perft("5K2/8/1Q6/2N5/8/1p2k3/8/8 w - - 0 1", 5, 1_004_658);
    }
    #[test]
    fn discovered_check_black() {
        assert_perft("8/8/1P2K3/8/2n5/1q6/8/5k2 b - - 0 1", 5, 1_004_658);
    }

    /// Promoting can itself deliver check.
    #[test]
    fn promote_to_give_check_white() {
        assert_perft("4k3/1P6/8/8/8/8/K7/8 w - - 0 1", 6, 217_342);
    }
    #[test]
    fn promote_to_give_check_black() {
        assert_perft("8/k7/8/8/8/8/1p6/4K3 b - - 0 1", 6, 217_342);
    }

    /// Underpromotion (to knight/bishop/rook, not just queen) can deliver
    /// check.
    #[test]
    fn underpromote_to_check_white() {
        assert_perft("8/P1k5/K7/8/8/8/8/8 w - - 0 1", 6, 92_683);
    }
    #[test]
    fn underpromote_to_check_black() {
        assert_perft("8/8/8/8/8/k7/p1K5/8 b - - 0 1", 6, 92_683);
    }

    /// The side to move can stalemate itself.
    #[test]
    fn self_stalemate_white() {
        assert_perft("K1k5/8/P7/8/8/8/8/8 w - - 0 1", 6, 2_217);
    }
    #[test]
    fn self_stalemate_black() {
        assert_perft("8/8/8/8/8/p7/8/k1K5 b - - 0 1", 6, 2_217);
    }

    #[test]
    fn stalemate_and_checkmate_white() {
        assert_perft("8/k1P5/8/1K6/8/8/8/8 w - - 0 1", 7, 567_584);
    }
    #[test]
    fn stalemate_and_checkmate_black() {
        assert_perft("8/8/8/8/1k6/8/K1p5/8 b - - 0 1", 7, 567_584);
    }

    /// A single move can give check from two pieces at once (e.g. moving a
    /// piece off a line to uncover a second attacker).
    #[test]
    fn double_check_white() {
        assert_perft("8/5k2/8/5N2/5Q2/2K5/8/8 w - - 0 1", 4, 23_527);
    }
    #[test]
    fn double_check_black() {
        assert_perft("8/8/2k5/5q2/5n2/8/5K2/8 b - - 0 1", 4, 23_527);
    }
}
