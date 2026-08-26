use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use crate::basetypes::Move;

use crate::eval::Score;

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
struct TTHit {
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
            bound: ScoreBound::from_bits((bits >> 40) & 0x08).unwrap(), 
            search_iteration: (bits >> 42) as u8 & SEARCH_ITERATION_NUM_MASK 
        }
    }
}

const SEARCH_ITERATION_NUM_MASK: u8 = 0b0011_1111; // Take 6 LSB

#[derive(Debug)]
pub struct TranspositionTable {
    // Use boxed slice rather than Vec to enforce it can't be resized
    entries: Box<[TTEntry]>,
    search_iteration: AtomicU8,
}

impl TranspositionTable {

    pub fn new(size_in_megabytes: usize) -> Self {
        if size_in_megabytes == 0 {
            panic!("TT size can't be 0")
        }
        let tt_entries = (size_in_megabytes * 1024 * 1024) / std::mem::size_of::<TTEntry>();

        let mut entry_vec = Vec::new();
        entry_vec.resize_with(tt_entries, TTEntry::default);
        Self { entries: entry_vec.into_boxed_slice(), search_iteration: AtomicU8::new(0)}
    }

    pub fn new_search(&self) {
        // Increment by 1, wrapping at 64
        let new_val = self.search_iteration.load(Ordering::Relaxed).wrapping_add(1) & SEARCH_ITERATION_NUM_MASK;
        self.search_iteration.store(new_val, Ordering::Relaxed);
    }

    fn index(&self, zobrist: u64) -> usize {
        // Given zobrist should be randomly distributed across u64, multiplying by num slots should randomly distribute 64 MSB
        // of a u128 across 0..(num slots).
        // alternative think of as 1/z * num_slots = randomly distributed int in 0..num_slots.
        (((zobrist as u128) * (self.entries.len() as u128)) >> 64) as usize
    }

    // Attempt to find an entry in the TT
    // 
    pub fn fetch(&self, zobrist: u64, ply: usize) -> Option<TTHit> {
        None
    }

    pub fn store(&self, zobrist: u64, mv: Move, score: Score, bound: ScoreBound, depth: u8, ply: i32) {

    }

}