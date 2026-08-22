use std::{collections::{HashMap, hash_map::Entry::{Occupied, Vacant}}, hash::Hash};

#[derive(Debug)]
pub enum MultiSetErr {
    KeyDoesNotExist
}

#[derive(Debug, Clone)]
pub struct MultiSet<K: Eq + Hash> {
    counts: HashMap<K, u64>
}

impl<K: Eq + Hash> MultiSet<K> {

    pub fn new() -> Self {
        Self{counts: HashMap::new()}
    }

    pub fn add(&mut self, key: K) {
        *self.counts.entry(key).or_insert(0) += 1;
    }

    pub fn count(&self, key: K) -> u64 {
        *self.counts.get(&key).unwrap_or(&0u64)
    }

    pub fn remove(&mut self, key: K) -> Result<(), MultiSetErr>{
        match self.counts.entry(key) {
            Occupied(x) => {
                if *x.get() == 0 {
                    x.remove();
                }
                return Ok(())
            },
            Vacant(_) => {
                return Err(MultiSetErr::KeyDoesNotExist)
            }
        }
    }

}