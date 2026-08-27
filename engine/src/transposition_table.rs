use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use crate::basetypes::{MATE_BOUND, Move, Score};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScoreBound {
    // 1 index so 0 index is explicitly a missing entry when marshalled into a TTEntry.data field
    Lower = 1,
    Upper = 2,
    Exact = 3
}

impl ScoreBound {
    fn from_bits(bits: u64) -> Option<ScoreBound> {
        match bits {
            1 => Some(ScoreBound::Lower),
            2 => Some(ScoreBound::Upper),
            3 => Some(ScoreBound::Exact),
            _ => None
        }
    }
}

#[derive(Debug, Default)]
struct TTEntry {
    key: AtomicU64,
    data: AtomicU64
}

#[derive(Copy, Clone, Debug)]
pub struct TTHit {
    pub mv: Move,
    pub score: Score,
    pub depth: u8,
    pub bound: ScoreBound,
    pub search_iteration: u8,
}

impl TTHit {
    // Marshall a Table Hit into a single u64
    // bits  0-15: move
    // bits 16-31: score as i16
    // bits 32-39: depth
    // bits 40-41: bound
    // bits 42-47: search_iteration of the entry
    // 
    // Note: That leaves 16 bits for use later, likely as 

    pub fn marshall(&self) -> u64 {
        let score = self.score;
        debug_assert!(self.score.abs() <= i16::MAX as i32, "score {score} does not fit the TT field");
        debug_assert!(self.search_iteration <= SEARCH_ITERATION_NUM_MASK, "search iteration is outside bounds");

        (self.mv.bits() as u64)
            | ((self.score as u16 as u64) << 16)
            | ((self.depth as u64) << 32)
            | ((self.bound as u64) << 40)
            | ((self.search_iteration as u64) << 42)
    }

    // Unmarshall bits into a TTHit
    // Assume bits are safe
    pub fn unmarshall(bits: u64) -> Self {
        TTHit { 
            mv: Move::from_bits(bits as u16), 
            score: ((bits >> 16) & 0xffff) as Score, 
            depth: ((bits >> 32) & 0xff) as u8, 
            bound: ScoreBound::from_bits((bits >> 40) & 0x03).unwrap(), 
            search_iteration: (bits >> 42) as u8 & SEARCH_ITERATION_NUM_MASK 
        }
    }
}

const SEARCH_ITERATION_NUM_MASK: u8 = 0b0011_1111; // Take 6 LSB

pub trait TT: std::fmt::Debug {
    fn new_search(&self);
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit>;
    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32);
}

#[derive(Debug)]
pub struct ActiveTranspositionTable {
    // Use boxed slice rather than Vec to enforce it can't be resized
    entries: Box<[TTEntry]>,
    search_iteration: AtomicU8,
}

impl ActiveTranspositionTable {
    pub fn new(size_in_megabytes: usize) -> Self {
        if size_in_megabytes == 0 {
            panic!("TT size can't be 0")
        }
        let tt_entries = (size_in_megabytes * 1024 * 1024) / std::mem::size_of::<TTEntry>();

        let mut entry_vec = Vec::new();
        entry_vec.resize_with(tt_entries, TTEntry::default);
        Self { entries: entry_vec.into_boxed_slice(), search_iteration: AtomicU8::new(0)}
    }

    fn hash_zobrist(&self, zobrist: u64) -> usize {
        // TODO: More efficient ways to do this if we assume table size is a power of 2. Maybe investigate in the future.

        // Given zobrist should be randomly distributed across u64, multiplying by num slots should randomly distribute 64 MSB
        // of a u128 across 0..(num slots).
        // alternative think of as 1/z * num_slots = randomly distributed int in 0..num_slots.
        (((zobrist as u128) * (self.entries.len() as u128)) >> 64) as usize
    }
}

impl TT for ActiveTranspositionTable {

    fn new_search(&self) {
        // Increment by 1, wrapping at 64
        self.search_iteration.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.wrapping_add(1) & SEARCH_ITERATION_NUM_MASK)
        }).unwrap();
    }

    // Attempt to find an entry in the TT
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit> {
        let entry = &self.entries[self.hash_zobrist(zobrist)];
        let key = entry.key.load(Ordering::Relaxed);
        let data = entry.data.load(Ordering::Relaxed);
        if key ^ data != zobrist {
            return None;
        }

        // TODO: Investigate unmarshalling only needed fields, might be quicker than the whole thing.
        let mut hit = TTHit::unmarshall(data);
        // Take the ply out of the score
        // TODO: move this to a common function w/ eval and store.
        if hit.score >= MATE_BOUND {
            hit.score -= ply as Score;
        } else if hit.score <= -MATE_BOUND {
            hit.score += ply as Score;
        }

        Some(hit)
    }

    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32) {
        let entry = &self.entries[self.hash_zobrist(zobrist)];
        let old_data = entry.data.load(Ordering::Relaxed);
        let search_iteration = self.search_iteration.load(Ordering::Relaxed);

        // Only replace if newer search or greater depth at same search
        let replace_slot = {
            if old_data == 0 {
                true
            } else {
                let old_hit = TTHit::unmarshall(old_data);
                // Cast depth to i32 to deal with overflow
                old_hit.search_iteration != search_iteration || depth as i32 + 2 > old_hit.depth as i32
            }
        };
        if !replace_slot {
            return;
        }

        // TODO: Future search enhancements may store bounds in TT without providing a move. Likely will need to modify the signature to allow None Move
        // and if analysing the same position, use the found move instead. Must check the move is for the right position however
        // let old_key = entry.key.load(Ordering::Relaxed);
        // let table_hit_matches_position = old_key ^ old_data == zobrist;

        //TODO: as with fetch, factor out the mate bound + ply logic into common function
        let score = if score >= MATE_BOUND {
            score + ply
        } else if score <= -MATE_BOUND {
            score - ply
        } else {
            score
        };

        let data = TTHit::marshall(&TTHit {
            mv,
            score,
            depth,
            bound,
            search_iteration
        });
        entry.key.store(zobrist ^ data, Ordering::Relaxed);
        entry.data.store(data, Ordering::Relaxed);
    }

}


// No-op transposition table
#[derive(Debug)]
pub struct NoopTranspositionTable;
impl TT for NoopTranspositionTable {
    fn new_search(&self) {}
    #[allow(unused_variables)]
    fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit> {None}
    #[allow(unused_variables)]
    fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32) {}
}