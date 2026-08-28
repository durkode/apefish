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

use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread::{self, JoinHandle}};

pub use basetypes::{GenericErr, UnvalidatedMove, Move, Piece, PieceKind, Side, Square};
pub use board::{Position};

use crate::{basetypes::{DrawReason, GameStatus, WinReason}, fen::to_fen, movegen::MoveGen, search::{SearchCommand, SearchLimits, SearchResult, Searcher}, transposition_table::{ActiveTranspositionTable, NoopTranspositionTable, TT}, zobrist::ZobristRandoms};


/// Event that the engine emits into a channel read by the client
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// One completed iterative-deepening iteration. Emitted zero or more times
    /// per search, in increasing `depth` order.
    Info { depth: u8, result: SearchResult },

    /// Periodic progress counters emitted mid-search, independent of iteration
    /// completion. Carries no `score`/`pv` — those are only stable at an
    /// iteration boundary (see [`EngineEvent::Info`]).
    Stats {
        /// Nominal depth of the iteration currently running.
        depth: u8,
        /// Total nodes searched so far this `go`.
        nodes: u64,
        /// Transposition table fill, in per-mille (0–1000).
        hashfull: u16,
        /// Tablebase probe hits so far this `go`.
        tbhits: u64,
    },

    /// End of search result.
    BestMove(SearchResult),
}

#[derive(Debug)]
pub struct SearchHandle {
    search_thread: JoinHandle<()>,
    send_command: mpsc::Sender<SearchCommand>,
    suppress_events: Arc<AtomicBool>,
}

/// Sink the engine calls to publish [`EngineEvent`]s. It is invoked from the
/// engine's internal search thread, so it must be `Send` and must not block —
/// a non-blocking channel send is the intended use.
pub type EventSink = Box<dyn Fn(EngineEvent) + Send>;

/// Interface every client targets. Search events are emitted async.
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

    /// Start searching from the current position under `limits` and return
    /// immediately. Emits Engine Events indicating status or final result of
    /// search. Search may be stopped early by [`Engine::stop`].
    ///
    /// Any search already running is stopped (via the same path as
    /// [`Engine::stop`]) and awaited before the new one starts, so only one
    /// search thread is ever live.
    fn go(&mut self, limits: SearchLimits, events: EventSink);

    /// Halt the search started by [`Engine::go`] and block until it has fully
    /// stopped, joining the search thread. The search's final
    /// [`EngineEvent::BestMove`] is delivered through the sink before this
    /// returns. A no-op if no search is running.
    fn stop(&mut self);
}

/// The concrete apefish engine implementing [`Engine`].
/// Make pub now for debugging purposes
#[derive(Debug)]
pub struct Apefish {
    position: Position,
    movegen: Arc<MoveGen>,
    zobrist_randoms: Arc<ZobristRandoms>,
    search_in_progress: Option<SearchHandle>,
    tt: Arc<dyn TT>,
}

impl Apefish {
    pub fn new(tt_size_in_mb: usize) -> Self {
        let zobrists = Arc::new(ZobristRandoms::new());
        let movegen = Arc::new(MoveGen::init());
        let tt: Arc<dyn TT> = match tt_size_in_mb {
            0 => Arc::new(NoopTranspositionTable),
            _ => Arc::new(ActiveTranspositionTable::new(16)),
        };
        let pos = Position::new(zobrists.clone(), movegen.clone());
        Apefish {
            position: pos,
            movegen:  movegen,
            zobrist_randoms: zobrists,
            search_in_progress: None,
            tt: tt
        }
    }

    fn finish_search(&mut self, suppress_events: bool) {
        // Check we don't have an existing search running, and clean up if it is.
        if let Some(in_progress) = self.search_in_progress.take() {
            if suppress_events {
                in_progress.suppress_events.store(true, Ordering::Relaxed);
            }
            let _ = in_progress.send_command.send(SearchCommand::Stop);
            let _ = in_progress.search_thread.join();
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

    fn go(&mut self, limits: SearchLimits, send_event: EventSink) {
        self.finish_search(true);

        let mut position = self.position.clone();
        let suppress_events = Arc::new(AtomicBool::new(false));
        let gate = suppress_events.clone();
        let gated_send_event: EventSink = Box::new(move |x| {
            if !gate.load(Ordering::Relaxed) {
                send_event(x)
            }
        });

        let (send_command, commands) = mpsc::channel();
        let mut searcher = Searcher::new(
            self.movegen.clone(),
            self.tt.clone(),
            commands,
        );

        self.search_in_progress = Some(SearchHandle {
            search_thread: thread::spawn(move || {
                searcher.search(&mut position, &limits, &gated_send_event);
            }),
            send_command,
            suppress_events,
        });
    }

    fn stop(&mut self) {
        self.finish_search(false);
    }
}
