//! Transposition Table (TT) for the chess engine.
//!
//! Features:
//! - Table sized in MB (approximate), rounded to nearest power-of-two bucket count
//! - Each bucket stores a single entry (simple direct-mapped). Replacement
//!   policy: prefer deeper entries, break ties by age (older replaced first).
//! - Node types: Exact, LowerBound (Beta), UpperBound (Alpha).
//! - Stores best move (for PV), depth, value, key, and an 8-bit age stamp.
//! - Probe returns:
//!   - Option<i32> when entry provides a usable score right away (cutoff/exact)
//!   - Otherwise returns Option<&TTEntry> for caller to inspect.
//! - Stats: probes, hits, stores.
//! - Save / load to compact binary file.
//!
//! Usage:
//! - Create via `TranspositionTable::new_strict_size_mb(mb)` or `::new_buckets(count)`.
//! - On new search call `tt.new_search()` to increment age.
//! - On each node: `tt.probe(key, depth, alpha, beta)` (use returned `ProbeResult`).
//! - After evaluating: `tt.store(key, depth, value, node_type, best_move)`.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// The kind of node stored in the TT entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeType {
    /// Exact score (best move found for this node)
    Exact = 0,
    /// Lower bound (was a beta cutoff; true score >= value)
    LowerBound = 1,
    /// Upper bound (was not enough to improve alpha; true score <= value)
    UpperBound = 2,
}

impl From<u8> for NodeType {
    fn from(v: u8) -> Self {
        match v {
            0 => NodeType::Exact,
            1 => NodeType::LowerBound,
            2 => NodeType::UpperBound,
            _ => NodeType::Exact,
        }
    }
}

impl From<NodeType> for u8 {
    fn from(n: NodeType) -> u8 {
        match n {
            NodeType::Exact => 0,
            NodeType::LowerBound => 1,
            NodeType::UpperBound => 2,
        }
    }
}

/// Packed representation of a Move for table storage.
/// We only need a small, stable packed encoding here to avoid depending on engine internals.
///
/// Layout (u32):
/// bits 0..6   : from square (0..127) -> 7 bits
/// bits 7..13  : to square (0..127)   -> 7 bits
/// bits 14..17 : promotion piece id   -> 4 bits (0=no-promo; 1..12 maps to Piece variants)
/// remaining bits unused.
///
/// NOTE: This encoding assumes squares are 0..127 (0x88). Promotion ids must be mapped by engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedMove(u32);

impl PackedMove {
    pub fn none() -> Self {
        PackedMove(0)
    }

    pub fn pack(from: usize, to: usize, promo_id: u8) -> Self {
        // Clips just in case
        let f = (from as u32) & 0x7F;
        let t = (to as u32) & 0x7F;
        let p = (promo_id as u32) & 0x0F;
        PackedMove((f) | (t << 7) | (p << 14))
    }

    pub fn unpack(&self) -> (usize, usize, u8) {
        let raw = self.0;
        let f = (raw & 0x7F) as usize;
        let t = ((raw >> 7) & 0x7F) as usize;
        let p = ((raw >> 14) & 0x0F) as u8;
        (f, t, p)
    }
}

/// TT Entry stored in the table.
///
/// Stored compactly. Uses u64 key (Zobrist), i32 value, i32 depth, one-byte age and node type,
/// and a PackedMove for best move.
#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub key: u64,
    pub value: i32,
    pub depth: i32,
    pub node: NodeType,
    pub age: u8,
    pub best: PackedMove,
}

impl TTEntry {
    /// Empty (invalid) entry marker
    pub fn empty() -> Self {
        TTEntry {
            key: 0,
            value: 0,
            depth: -1,
            node: NodeType::Exact,
            age: 0,
            best: PackedMove::none(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.depth < 0
    }
}

/// Result of probing the table.
#[derive(Clone, Copy, Debug)]
pub enum ProbeResult {
    /// No entry found.
    Miss,
    /// Found entry but it does not provide immediate alpha/beta utility; returns entry.
    Found(TTEntry),
    /// Entry contains an exact or cutoff value that can be used immediately (score).
    /// The score is already adjusted for side-to-move semantics by caller convention (we store engine score).
    Usable(i32, Option<PackedMove>),
}

/// Transposition Table structure
pub struct TranspositionTable {
    buckets: Vec<TTEntry>,
    mask: usize, // index mask (buckets.len() - 1)
    pub age: u8,

    // Stats
    pub probes: u64,
    pub hits: u64,
    pub stores: u64,
}

impl TranspositionTable {
    /// Create a new table with approximately `size_mb` megabytes of storage.
    ///
    /// We approximate size per entry and choose nearest power-of-two bucket count.
    pub fn new_strict_size_mb(size_mb: usize) -> Self {
        // Estimate bytes per entry:
        // u64 key(8) + i32 value(4) + i32 depth(4) + u8 age(1) + u8 node(1) + PackedMove(4) + padding => ~24 bytes
        let bytes_per_entry = 24usize.max(std::mem::size_of::<TTEntry>());
        let total_bytes = size_mb * 1024 * 1024;
        let buckets = (total_bytes / bytes_per_entry).max(1);
        // Round down to the largest power of two that fits. NOTE: we must take
        // `leading_zeros` of the same width we shift in (usize), otherwise a
        // 32-bit count mixed with `usize::BITS` overshoots wildly.
        let pow = (usize::BITS - 1 - buckets.leading_zeros()) as usize;
        let count = 1usize << pow;
        TranspositionTable::new_buckets(count)
    }

    /// Create a new table with exactly `buckets` entries (must be power-of-two for mask).
    pub fn new_buckets(buckets: usize) -> Self {
        assert!(buckets >= 1, "buckets must be >= 1");
        // ensure power of two; if not, round up
        let mut count = 1usize;
        while count < buckets {
            count <<= 1;
        }
        let vec = vec![TTEntry::empty(); count];
        TranspositionTable {
            buckets: vec,
            mask: count - 1,
            age: 1,
            probes: 0,
            hits: 0,
            stores: 0,
        }
    }

    /// Simple index function: use lower bits of key xor-shifted.
    #[inline]
    fn index_of(&self, key: u64) -> usize {
        // xor-fold to reduce clustering
        let folded = key ^ (key >> 32) ^ (key >> 16);
        (folded as usize) & self.mask
    }

    /// Called at the start of a fresh search iteration to age entries.
    ///
    /// The TT stores an 8-bit age stamp. This is incremented per search so we can prefer
    /// newer entries (same depth) or invalidate old ones cheaply.
    pub fn new_search(&mut self) {
        // increment age (wrap-around allowed)
        self.age = self.age.wrapping_add(1);
    }

    /// Probe table for `key` with `depth` and alpha/beta window.
    ///
    /// If an entry is usable to return a value immediately according to alpha-beta rules,
    /// returns `ProbeResult::Usable(value, best_move_opt)`.
    /// If entry present but not immediately usable, returns `ProbeResult::Found(entry)`.
    /// If no entry, returns `ProbeResult::Miss`.
    ///
    /// Note: `alpha` and `beta` follow standard negamax bounds semantics.
    pub fn probe(&mut self, key: u64, depth: i32, alpha: i32, beta: i32) -> ProbeResult {
        self.probes = self.probes.wrapping_add(1);
        let idx = self.index_of(key);
        let entry = self.buckets[idx];
        if entry.is_empty() || entry.key != key {
            return ProbeResult::Miss;
        }
        // key matches
        self.hits = self.hits.wrapping_add(1);

        // If stored entry depth is >= required depth, we may use the entry
        if entry.depth >= depth {
            match entry.node {
                NodeType::Exact => {
                    ProbeResult::Usable(entry.value, Some(entry.best))
                }
                NodeType::LowerBound => {
                    // stored value is a lower bound: usable if value >= beta
                    if entry.value >= beta {
                        ProbeResult::Usable(entry.value, Some(entry.best))
                    } else {
                        ProbeResult::Found(entry)
                    }
                }
                NodeType::UpperBound => {
                    // stored value is an upper bound: usable if value <= alpha
                    if entry.value <= alpha {
                        ProbeResult::Usable(entry.value, Some(entry.best))
                    } else {
                        ProbeResult::Found(entry)
                    }
                }
            }
        } else {
            // depth insufficient: return entry for ordering info (e.g., PV move)
            ProbeResult::Found(entry)
        }
    }

    /// Store an entry into the table with replacement policy.
    ///
    /// Replacement heuristic:
    /// - If the slot is empty -> place entry
    /// - Else if new.depth > old.depth -> replace
    /// - Else if ages differ -> replace older entry (so newer searches prefer newer data)
    /// - Else replace (tie-break)
    pub fn store(
        &mut self,
        key: u64,
        depth: i32,
        value: i32,
        node: NodeType,
        best_move: Option<(usize, usize, u8)>, // (from, to, promo_id) packed here
    ) {
        self.stores = self.stores.wrapping_add(1);
        let idx = self.index_of(key);
        let old = self.buckets[idx];
        let mut packed = PackedMove::none();
        if let Some((from, to, promo_id)) = best_move {
            packed = PackedMove::pack(from, to, promo_id);
        }

        let new_entry = TTEntry {
            key,
            value,
            depth,
            node,
            age: self.age,
            best: packed,
        };

        // Decide replacement
        let replace = if old.is_empty() || new_entry.depth > old.depth {
            true
        } else if new_entry.depth == old.depth {
            // prefer newer age
            if new_entry.age != old.age {
                true // replace older with newer
            } else {
                // equal depth and age -> prefer Exact > LowerBound > UpperBound
                match (new_entry.node, old.node) {
                    (NodeType::Exact, NodeType::Exact) => true, // update best move / value
                    (NodeType::Exact, _) => true,
                    (NodeType::LowerBound, NodeType::UpperBound) => true,
                    (_, _) => false,
                }
            }
        } else {
            // new.depth < old.depth -> only replace if old is very old (age) or same key collision
            if old.age != self.age {
                // old is from older search -> replace
                true
            } else {
                // do not replace
                false
            }
        };

        if replace {
            self.buckets[idx] = new_entry;
        } else {
            // not replacing; however, if same key we might update the best move/value if deeper/equal
            if old.key == key && new_entry.depth >= old.depth {
                self.buckets[idx] = new_entry;
            }
        }
    }

    /// Force clear the TT (set all entries empty).
    pub fn clear(&mut self) {
        for e in self.buckets.iter_mut() {
            *e = TTEntry::empty();
        }
        self.age = 1;
        self.probes = 0;
        self.hits = 0;
        self.stores = 0;
    }

    /// Dump the table to a binary file. Format:
    /// [u64: magic][u32:buckets][entries...]
    /// Each entry serialized as:
    /// [u64 key][i32 value][i32 depth][u8 node][u8 age][u32 packed_move]
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut f = File::create(path)?;
        f.write_all(&0x54544142u64.to_le_bytes())?; // 'TTAB' magic
        let cnt = self.buckets.len() as u32;
        f.write_all(&cnt.to_le_bytes())?;
        for e in &self.buckets {
            f.write_all(&e.key.to_le_bytes())?;
            f.write_all(&e.value.to_le_bytes())?;
            f.write_all(&e.depth.to_le_bytes())?;
            f.write_all(&u8::from(e.node).to_le_bytes())?;
            f.write_all(&e.age.to_le_bytes())?;
            f.write_all(&e.best.0.to_le_bytes())?;
        }
        Ok(())
    }

    /// Load the table from file if compatible size; if incompatible, returns Err.
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let mut f = File::open(path)?;
        let mut buf8 = [0u8; 8];
        f.read_exact(&mut buf8)?;
        let magic = u64::from_le_bytes(buf8);
        if magic != 0x54544142u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bad magic",
            ));
        }
        let mut buf4 = [0u8; 4];
        f.read_exact(&mut buf4)?;
        let cnt = u32::from_le_bytes(buf4) as usize;
        if cnt != self.buckets.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bucket count mismatch",
            ));
        }
        for e in self.buckets.iter_mut() {
            let mut b8 = [0u8; 8];
            f.read_exact(&mut b8)?;
            let key = u64::from_le_bytes(b8);
            let mut i4 = [0u8; 4];
            f.read_exact(&mut i4)?;
            let value = i32::from_le_bytes(i4);
            f.read_exact(&mut i4)?;
            let depth = i32::from_le_bytes(i4);
            let mut b1 = [0u8; 1];
            f.read_exact(&mut b1)?;
            let node = NodeType::from(b1[0]);
            f.read_exact(&mut b1)?;
            let age = b1[0];
            let mut b4 = [0u8; 4];
            f.read_exact(&mut b4)?;
            let packed = u32::from_le_bytes(b4);
            *e = TTEntry {
                key,
                value,
                depth,
                node,
                age,
                best: PackedMove(packed),
            };
        }
        Ok(())
    }

    /// Return stats snapshot as a human-readable string.
    pub fn stats(&self) -> String {
        format!(
            "TT: buckets={} probes={} hits={} stores={} hit_rate={:.2}%",
            self.buckets.len(),
            self.probes,
            self.hits,
            self.stores,
            if self.probes == 0 {
                0.0
            } else {
                (self.hits as f64 / self.probes as f64) * 100.0
            }
        )
    }

    /// Return the best move stored for a given key, if present.
    pub fn best_move_for(&self, key: u64) -> Option<(usize, usize, u8)> {
        let idx = self.index_of(key);
        let e = self.buckets[idx];
        if e.key != key || e.is_empty() {
            None
        } else {
            let (f, t, p) = e.best.unpack();
            if e.best == PackedMove::none() {
                None
            } else {
                Some((f, t, p))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_store_and_probe() {
        let mut tt = TranspositionTable::new_buckets(1024);
        tt.clear();
        let key: u64 = 0x12345678abcdef;
        let depth = 5;
        let value = 42;
        let node = NodeType::Exact;
        tt.new_search();
        tt.store(key, depth, value, node, Some((10usize, 20usize, 0u8)));

        match tt.probe(key, depth, -1000000, 1000000) {
            ProbeResult::Usable(v, Some(p)) => {
                assert_eq!(v, 42);
                let (f, t, p_id) = p.unpack();
                assert_eq!(f, 10);
                assert_eq!(t, 20);
                assert_eq!(p_id, 0u8);
            }
            _ => panic!("expected usable exact"),
        }
    }

    #[test]
    fn replacement_policy_prefers_deeper() {
        let mut tt = TranspositionTable::new_buckets(256);
        tt.clear();
        let key = 0x1111u64;
        tt.new_search();
        tt.store(key, 3, 10, NodeType::UpperBound, None);
        let _first = tt.buckets[tt.index_of(key)];
        tt.store(key, 6, 20, NodeType::Exact, None);
        let second = tt.buckets[tt.index_of(key)];
        assert_eq!(second.depth, 6);
        assert_eq!(second.value, 20);
    }
}
