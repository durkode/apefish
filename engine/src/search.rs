//! Search: choosing a move under time/depth constraints.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::zobrist::ZobristKey;
use crate::{EngineEvent, EventSink};
use crate::basetypes::{MATE, MAX_PLY, Move, Score};
use crate::board::Position;
use crate::movegen::MoveGen;
use crate::time_management::TimeCutoffs;
use crate::transposition_table::{ScoreBound, TT};


const STATS_EVENT_INTERVAL: Duration = Duration::from_millis(200);
const TT_WALK_MAX_LENGTH: usize = 16; // Make length to go through TT to find the PV line

#[derive(Debug, Clone)]
pub enum SearchCommand{
    Stop,
    /// The opponent played the move we were pondering on. Leave ponder mode and
    /// start enforcing the time budget.
    PonderHit,
}

struct PVTriangle {
    pv: [[Move; MAX_PLY]; MAX_PLY], // The PV for a given ply
    pv_length: [usize; MAX_PLY] // The length of a PV for a given ply
}

impl PVTriangle {

    pub fn new() -> Self {
        PVTriangle { 
            pv: [[Move::default(); MAX_PLY]; MAX_PLY], 
            pv_length: [0; MAX_PLY],
        }
    }

    pub fn start_negamax(&mut self, ply: usize) {
        self.pv_length[ply] = 0;
    }

    pub fn new_best_move(&mut self, mv: Move, ply: usize) {
        self.pv[ply][0] = mv;
        for i in 0..self.pv_length[ply + 1] {
            self.pv[ply][i + 1] = self.pv[ply + 1][i];
        }
        self.pv_length[ply] = self.pv_length[ply + 1] + 1;
    }

    pub fn get_pv(&self, pos: &mut Position, tt: &dyn TT) -> Vec<Move> {
        // TODO: I thought this would consume self.pv, but given it is not mut it can't
        //       Investigate this.
        // TODO: probably should move this to the searcher, rather than doing this on the pvtriangle struct
        let mut pv = Vec::from(&self.pv[0][..self.pv_length[0]]);
        
        // First, walk the pv line as far as possible
        // Note that the pv line might be corrupted due to a search aborted halfway through,
        // So treat each move as a possible failure. Verify against the board
        let mut valid_length = 0;
        for &mv in &pv {
            if pos.make_move(mv).is_err() {
                break
            }
            valid_length += 1;
        }        
        pv.truncate(valid_length);

        // Now we have a valid pv line from the triangle, walk the TT
        // Note: we must check for loop detection (pos can return back to old pos)
        // And we want to cap the length at a reasonable walk (TT_WALK_MAX_LENGTH)
        let mut visited_zobrists = [ZobristKey::default(); TT_WALK_MAX_LENGTH];
        let mut walk_counter = 0;
        let mut curr_zobrist = pos.get_zobrist();

        while let Some(tt_hit) = tt.fetch(curr_zobrist, pv.len()) {
            if pos.make_move(tt_hit.mv).is_err() {
                break
            }
            pv.push(tt_hit.mv);
            visited_zobrists[walk_counter] = curr_zobrist;
            walk_counter += 1;
            curr_zobrist = pos.get_zobrist();
            if walk_counter >= TT_WALK_MAX_LENGTH || visited_zobrists.contains(&curr_zobrist) {
                break
            }
        };

        // Now unwind all the made moves used in validation
        for _ in 0..pv.len() {
            pos.unmake_move();
        }

        pv
    }
}


/// Constraints on a single search call. All fields optional; interpretation
/// (e.g. how clock time maps to a time budget) is up to the search implementation.
#[derive(Debug, Clone, Default)]
pub struct SearchLimits {
    pub depth: Option<u8>,
    pub movetime: Option<Duration>,
    pub wtime: Option<Duration>,
    pub btime: Option<Duration>,
    pub winc: Option<Duration>,
    pub binc: Option<Duration>,
    /// Search on the opponent's clock. Time cutoffs are ignored until a
    /// [`SearchCommand::PonderHit`] arrives; until then only `Stop` ends the
    /// search, and no `BestMove` is emitted.
    pub ponder: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    pub score: Score,
    /// Principal variation, starting with `best_move`.
    pub pv: Vec<Move>,
    pub nodes: u64,
}

#[derive(Debug, Clone)]
pub struct NegamaxResult {
    pub best_move: Option<Move>,
    pub score: Score,
}

#[derive(Debug)]
pub struct Searcher {
    movegen: Arc<MoveGen>,
    tt: Arc<dyn TT>,
    commands: Receiver<SearchCommand>,
    stop: bool,
    /// When this search must stop, and whether it is currently pondering.
    /// `None` for unbounded searches (`go`, `go depth N`, `go infinite`).
    /// Owns all ponder state; see [`TimeCutoffs`].
    time_cutoffs: Option<TimeCutoffs>,
    last_stats_event_emitted: Option<Instant>,
}

// TODO: restructure so there is 1 searcher per search running that is recreated from scratch, rather than reusing the same one.
// It is just conceptually cleaner.
impl Searcher {

    pub fn new(movegen: Arc<MoveGen>, transposition_table: Arc<dyn TT>, commands: Receiver<SearchCommand>) -> Self {
        Searcher {
            movegen: movegen,
            tt: transposition_table,
            commands: commands,
            stop: false,
            time_cutoffs: None,
            last_stats_event_emitted: None,
        }
    }

    // Drain all pending commands from the channel without blocking. Called
    // periodically from within the search.
    fn drain_commands(&mut self) {
        loop {
            match self.commands.try_recv() {
                Ok(SearchCommand::Stop) => self.stop = true,
                Ok(SearchCommand::PonderHit) => self.note_ponder_hit(),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stop = true;
                    break
                }
            }
        }
    }

    /// Opponent played the pondered move: hand it to the cutoffs, which leave
    /// ponder mode and start the clock.
    fn note_ponder_hit(&mut self) {
        if let Some(cutoffs) = self.time_cutoffs.as_mut() {
            cutoffs.ponder_hit();
        }
    }

    fn pondering(&self) -> bool {
        self.time_cutoffs.as_ref().is_some_and(|c| c.is_pondering())
    }

    // Main search entrypoint.
    // From this should branch into opening book, closing tables, or negamax.
    pub fn search(&mut self, pos: &mut Position, limits: &SearchLimits, send_event: &EventSink) {

        // For now just call iterative deepening. In future add opening book calls to here as well
        let search_result = self.iterative_deepening(pos, limits, send_event);

        // UCI forbids emitting `bestmove` while pondering. If the search
        // exhausted its depth on its own before `ponderhit`/`stop` arrived,
        // block until one of them does.
        while !self.stop && self.pondering() {
            match self.commands.recv() {
                Ok(SearchCommand::Stop) => self.stop = true,
                Ok(SearchCommand::PonderHit) => self.note_ponder_hit(),
                Err(_) => break, // command channel dropped: fall through and finish
            }
        }

        send_event(EngineEvent::BestMove(search_result));
    }

    fn iterative_deepening(&mut self, pos: &mut Position, limits: &SearchLimits, send_event: &EventSink) -> SearchResult {
        // For now, no limit means 1. In future this should be an extremely large number.
        let max_depth = limits.depth.unwrap_or(u8::MAX);
        let ply = 0;
        let alpha = -MATE; // Starting bounds for best move for current side
        let beta = MATE; // Starting bounds for best move for opponent side

        self.time_cutoffs = TimeCutoffs::from_search_limit(limits, pos.side_to_move());

        self.tt.new_search(); // Set the correct search iteration now we are starting a new search

        let mut pv_triangle: Box<PVTriangle> = Box::new(PVTriangle::new());

        let mut score: Score = 0;
        let mut best_move = None;
        let mut nodes_searched = 0; // Running total mutated from within negamax search

        for depth in 0..max_depth {
            if self.time_cutoffs.as_ref().is_some_and(|c| c.exceeded_soft()) {
                break;
            }
            if let Some(negamax_result) = self.negamax(
                pos,
                depth,
                ply,
                alpha,
                beta,
                &mut nodes_searched,
                &mut pv_triangle,
                send_event,
            ) {
                score = negamax_result.score;
                best_move = negamax_result.best_move;

                // Emit a status update
                send_event(EngineEvent::Info { 
                    depth, 
                    result: SearchResult { 
                        best_move, 
                        score, 
                        pv: pv_triangle.get_pv(pos, self.tt.as_ref()), 
                        nodes: nodes_searched
                    } 
                });
            } else {
                // Aborted either by stop request or new search.
                break;
            }
        }

        // If we can't check to any depth, just return the first legal move to remain valid
        if best_move.is_none() {
            best_move = pos.legal_moves().first().copied();
        }

        SearchResult { 
            best_move, 
            score, 
            pv: pv_triangle.get_pv(pos, self.tt.as_ref()), 
            nodes: nodes_searched
        }
    }

    // For now return move as well as score, remove move once we have that in the TT.
    // Alpha: best score for active side achieved so far.
    // Beta: best score for opponent achieved so far.
    // Returns:
    //   - score
    //   - best_move
    //   - nodes_searched
    //   - Search complete (not aborted)
    fn negamax(
        &mut self, pos:
        &mut Position,
        depth: u8,
        ply: usize,
        mut alpha: Score,
        beta: Score,
        nodes_searched: &mut u64,
        pv: &mut Box<PVTriangle>,
        send_event: &EventSink
    ) -> Option<NegamaxResult> {

        pv.start_negamax(ply);

        *nodes_searched += 1;

        // Check if search is aborted or timed out
        // Only do every 2048 nodes as no need for more often
        if *nodes_searched & 2047 == 0 {
            self.drain_commands();
            if self.stop {
                return None
            }
            // `exceeded_hard` is always false while pondering, so this keeps
            // running until `ponderhit` (which starts the clock) or `stop`.
            if self.time_cutoffs.as_ref().is_some_and(|c| c.exceeded_hard()) {
                return None
            }

            // Emit an info event if time has elapsed
            let now = Instant::now();
            if self.last_stats_event_emitted.is_none() || self.last_stats_event_emitted.unwrap() + STATS_EVENT_INTERVAL < now {
                // TODO: populate the hashfull and tbhits fields for better data once we track these.
                send_event(
                    EngineEvent::Stats { 
                        depth: depth + ply as u8, // TODO: this will break with future search extensions, store root depth on the search object
                        nodes: *nodes_searched, 
                        hashfull: self.tt.hashfull(), 
                        tbhits: 0 
                    }
                );
                self.last_stats_event_emitted = Some(now);
            }
        }

        if depth == 0 || ply >= MAX_PLY {
            return Some(NegamaxResult {best_move: None, score: pos.evaluate()})
        }

        // Check the TT to see if this position is stored already
        let mut tt_move = None;
        if let Some(tt_hit) = self.tt.fetch(pos.get_zobrist(), ply) {
            tt_move = Some(tt_hit.mv);
            if tt_hit.depth >= depth {
                // This position has already been searched at a depth >= requested depth
                // Return early if possible
                match tt_hit.bound {
                    ScoreBound::Exact => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    ScoreBound::Lower if tt_hit.score >= beta => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    ScoreBound::Upper if tt_hit.score <= alpha => return Some(NegamaxResult{best_move: tt_move, score: tt_hit.score}),
                    _ => {}
                }
            }
        }

        let starting_alpha = alpha;
        let mut best_score = Score::MIN;
        let mut best_move = None;

        let mut pseudo_legal_moves = self.movegen.pseudo_legal_moves(pos);
        // If present, take the next move retrieved from the TT and make sure it is searched first
        if let Some(m) = tt_move {
            if let Some(i) = pseudo_legal_moves.iter().position(|&x| x == m) {
                pseudo_legal_moves.swap(0, i);
            }
        }

        for cm in pseudo_legal_moves {
            // Make move and get score
            if pos.make_move(cm).is_err() {
                continue
            }
            let subsearch = self.negamax(pos, depth - 1, ply + 1, -beta, -alpha, nodes_searched, pv, send_event);
            pos.unmake_move();

            if subsearch.is_none() {
                // Search was aborted or timed out
                return None
            }

            let score = -1 * subsearch.unwrap().score;

            // Use fail-soft version of alpha beta pruning.
            if score > best_score {
                best_score = score;
                best_move = Some(cm);
            }
            if best_score > alpha {
                alpha = best_score;
                // I assume best_move must be set here, I guess we will find out
                pv.new_best_move(best_move.unwrap(), ply);
            }
            if alpha >= beta {
                break;
            }
        }

        // TODO: we don't currently check for repetitions, do we need to? Unsure right now.
        // Not worried about sorting for insufficient material as assume end game tables will be used.
        if best_move.is_none() {
            // No move found, so game is over
            if pos.in_check() {
                // Need to add ply in order to get the quickest path to checkmate (penalise longer paths)
                return Some(NegamaxResult{best_move: None, score: -MATE + ply as Score})
            } else {
                return Some(NegamaxResult { best_move: None, score: 0});
            }
        } else {
            // Store the Score + Move in the TT
            // The bound is based on the original alpha when invoking the function,
            // not what it was refined to during execution.
            let bound = if best_score >= beta { ScoreBound::Lower }
                        else if best_score > starting_alpha { ScoreBound::Exact }
                        else { ScoreBound::Upper };
            self.tt.store(
                pos.get_zobrist(),
                best_move.unwrap(),
                best_score,
                bound,
                depth,
                ply as i32,
            );
        }

        Some(NegamaxResult{best_move: best_move, score: best_score})
    }
}
