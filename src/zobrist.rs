// Features:
// - Incremental hash updates (make/unmake moves without full board scan)
// - Hash verification and collision detection
// - Polyglot opening book compatibility option
// - Pawn hash keys for pawn structure evaluation
// - Material hash keys for endgame tablebase lookups
// - Statistical tracking (collisions, updates, verifications)
// - Save/load hash keys from file for reproducibility
// - Debug utilities and hash key inspection
// - Thread-safe singleton pattern for global Zobrist instance

use crate::board::Board;
use crate::types::{Piece, Sq};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::sync::{Arc, Mutex, OnceLock};

/// The process-wide, deterministically-seeded key table used for all board
/// hashing. Seeded once with a fixed value so hashes are stable across runs.
static SHARED_KEYS: OnceLock<Zobrist> = OnceLock::new();

/// Borrow the shared, immutable Zobrist key table used by [`crate::board`] for
/// incremental hashing. Cheap to call (one `OnceLock` read after init).
#[inline]
pub fn keys() -> &'static Zobrist {
    SHARED_KEYS.get_or_init(|| Zobrist::with_seed(0x00C0_FFEE_5EED_1234))
}

impl Default for Zobrist {
    fn default() -> Self {
        Self::new()
    }
}

// =====================
// Core Zobrist Structure
// =====================

#[derive(Clone)]
pub struct Zobrist {
    // Piece placement keys: [square][piece_type]
    pub pieces: [[u64; 12]; 128],

    // Side to move key (XOR if white to move)
    pub side: u64,

    // Castling rights keys (16 possible combinations)
    pub castling: [u64; 16],

    // En passant file keys (8 files, indexed by file 0-7)
    pub ep_file: [u64; 8],

    // Full en passant square keys (for maximum precision)
    pub ep_square: [u64; 128],

    // Seed used for generation (for reproducibility)
    seed: u64,

    // Statistics tracking
    stats: ZobristStats,
}

#[derive(Clone, Debug, Default)]
pub struct ZobristStats {
    pub hash_calls: u64,
    pub incremental_updates: u64,
    pub full_rehashes: u64,
    pub collisions_detected: u64,
    pub verifications: u64,
}

// =====================
// Zobrist Implementation
// =====================

impl Zobrist {
    /// Create a new Zobrist instance with the default seed
    pub fn new() -> Self {
        Self::with_seed(2024)
    }

    /// Create a new Zobrist instance with a custom seed for reproducibility
    pub fn with_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        // Generate piece placement keys (only valid 0x88 squares).
        let mut pieces = [[0u64; 12]; 128];
        for (sq, row) in pieces.iter_mut().enumerate() {
            if (sq & 0x88) == 0 {
                for key in row.iter_mut() {
                    *key = rng.r#gen();
                }
            }
        }

        // Generate castling rights keys.
        let mut castling = [0u64; 16];
        for key in castling.iter_mut() {
            *key = rng.r#gen();
        }

        // Generate en passant file keys (standard approach).
        let mut ep_file = [0u64; 8];
        for key in ep_file.iter_mut() {
            *key = rng.r#gen();
        }

        // Generate en passant square keys (for precision).
        let mut ep_square = [0u64; 128];
        for (sq, key) in ep_square.iter_mut().enumerate() {
            if (sq & 0x88) == 0 {
                *key = rng.r#gen();
            }
        }

        // Side to move key
        let side = rng.r#gen();

        Self {
            pieces,
            side,
            castling,
            ep_file,
            ep_square,
            seed,
            stats: ZobristStats::default(),
        }
    }

    /// Create Zobrist with Polyglot-compatible keys (for opening book compatibility)
    pub fn polyglot() -> Self {
        // Polyglot uses a specific seed and generation pattern
        // This is a simplified version; full Polyglot compatibility requires exact values
        Self::with_seed(0x0123456789ABCDEF)
    }

    // =====================
    // Piece Indexing
    // =====================

    /// Convert a Piece enum to an index (0-11)
    pub fn piece_index(piece: Piece) -> Option<usize> {
        match piece {
            Piece::WP => Some(0),
            Piece::WN => Some(1),
            Piece::WB => Some(2),
            Piece::WR => Some(3),
            Piece::WQ => Some(4),
            Piece::WK => Some(5),
            Piece::BP => Some(6),
            Piece::BN => Some(7),
            Piece::BB => Some(8),
            Piece::BR => Some(9),
            Piece::BQ => Some(10),
            Piece::BK => Some(11),
            Piece::Empty => None,
        }
    }

    /// Get piece type name for debugging
    pub fn piece_name(index: usize) -> &'static str {
        match index {
            0 => "WP",
            1 => "WN",
            2 => "WB",
            3 => "WR",
            4 => "WQ",
            5 => "WK",
            6 => "BP",
            7 => "BN",
            8 => "BB",
            9 => "BR",
            10 => "BQ",
            11 => "BK",
            _ => "??",
        }
    }

    /// The Zobrist key for `piece` standing on square `sq`. Returns `0`
    /// (the XOR identity) for an empty piece, so callers can XOR
    /// unconditionally.
    #[inline]
    pub fn piece_key(&self, piece: Piece, sq: Sq) -> u64 {
        match Self::piece_index(piece) {
            Some(idx) => self.pieces[sq][idx],
            None => 0,
        }
    }

    // =====================
    // Full Board Hashing
    // =====================

    /// Compute the full Zobrist hash for a board position
    pub fn hash_board(&mut self, board: &Board) -> u64 {
        self.stats.hash_calls += 1;
        self.stats.full_rehashes += 1;

        let mut h = 0u64;

        // Hash all pieces on valid squares
        for sq in 0..128 {
            if (sq & 0x88) != 0 {
                continue; // Skip invalid 0x88 squares
            }

            let piece = board.cells[sq];
            if let Some(idx) = Self::piece_index(piece) {
                h ^= self.pieces[sq][idx];
            }
        }

        // Hash castling rights
        h ^= self.castling[board.castling as usize];

        // Hash en passant (using file-based approach for Polyglot compatibility)
        if let Some(ep_sq) = board.ep {
            let file = ep_sq & 15;
            if file < 8 {
                h ^= self.ep_file[file];
            }
        }

        // Hash side to move (XOR if white to move)
        if board.side_white {
            h ^= self.side;
        }

        h
    }

    /// Compute hash without updating statistics (for verification)
    pub fn hash_board_quiet(&self, board: &Board) -> u64 {
        let mut h = 0u64;

        for sq in 0..128 {
            if (sq & 0x88) != 0 {
                continue;
            }
            let piece = board.cells[sq];
            if let Some(idx) = Self::piece_index(piece) {
                h ^= self.pieces[sq][idx];
            }
        }

        h ^= self.castling[board.castling as usize];

        if let Some(ep_sq) = board.ep {
            let file = ep_sq & 15;
            if file < 8 {
                h ^= self.ep_file[file];
            }
        }

        if board.side_white {
            h ^= self.side;
        }

        h
    }

    // =====================
    // Incremental Hashing
    // =====================

    /// Incrementally update hash after moving a piece
    /// Returns the new hash value
    pub fn update_move(
        &mut self,
        current_hash: u64,
        from: Sq,
        to: Sq,
        piece: Piece,
        captured: Option<Piece>,
    ) -> u64 {
        self.stats.incremental_updates += 1;

        let mut h = current_hash;

        // Remove piece from source square
        if let Some(idx) = Self::piece_index(piece) {
            h ^= self.pieces[from][idx];
        }

        // Remove captured piece from destination (if any)
        if let Some(cap) = captured
            && let Some(idx) = Self::piece_index(cap) {
                h ^= self.pieces[to][idx];
            }

        // Add piece to destination square
        if let Some(idx) = Self::piece_index(piece) {
            h ^= self.pieces[to][idx];
        }

        // Toggle side to move
        h ^= self.side;

        h
    }

    /// Update hash for castling rights change
    pub fn update_castling(&self, current_hash: u64, old_rights: u8, new_rights: u8) -> u64 {
        let mut h = current_hash;
        h ^= self.castling[old_rights as usize];
        h ^= self.castling[new_rights as usize];
        h
    }

    /// Update hash for en passant change
    pub fn update_ep(&self, current_hash: u64, old_ep: Option<Sq>, new_ep: Option<Sq>) -> u64 {
        let mut h = current_hash;

        // Remove old EP
        if let Some(ep) = old_ep {
            let file = ep & 15;
            if file < 8 {
                h ^= self.ep_file[file];
            }
        }

        // Add new EP
        if let Some(ep) = new_ep {
            let file = ep & 15;
            if file < 8 {
                h ^= self.ep_file[file];
            }
        }

        h
    }

    /// Toggle side to move in hash
    pub fn toggle_side(&self, current_hash: u64) -> u64 {
        current_hash ^ self.side
    }

    // =====================
    // Specialized Hashing
    // =====================

    /// Compute pawn structure hash (for pawn evaluation caching)
    pub fn pawn_hash(&self, board: &Board) -> u64 {
        let mut h = 0u64;

        for sq in 0..128 {
            if (sq & 0x88) != 0 {
                continue;
            }

            let piece = board.cells[sq];
            match piece {
                Piece::WP => h ^= self.pieces[sq][0],
                Piece::BP => h ^= self.pieces[sq][6],
                _ => {}
            }
        }

        h
    }

    /// Compute material hash (for endgame tablebase lookups)
    pub fn material_hash(&self, board: &Board) -> u64 {
        let mut h = 0u64;
        let mut piece_counts = [0u8; 12];

        // Count pieces
        for sq in 0..128 {
            if (sq & 0x88) != 0 {
                continue;
            }
            if let Some(idx) = Self::piece_index(board.cells[sq]) {
                piece_counts[idx] += 1;
            }
        }

        // Hash based on piece counts (order-independent)
        for (idx, &count) in piece_counts.iter().enumerate() {
            for _ in 0..count {
                // Use a simple hash combining piece type and count
                h ^= self.pieces[idx][idx].wrapping_mul(count as u64);
            }
        }

        h
    }

    /// Compute hash for only a specific piece type (for specialized evaluation)
    pub fn piece_type_hash(&self, board: &Board, piece: Piece) -> u64 {
        let mut h = 0u64;

        if let Some(idx) = Self::piece_index(piece) {
            for sq in 0..128 {
                if (sq & 0x88) != 0 {
                    continue;
                }
                if board.cells[sq] == piece {
                    h ^= self.pieces[sq][idx];
                }
            }
        }

        h
    }

    // =====================
    // Verification and Debugging
    // =====================

    /// Verify that an incremental hash matches a full board hash
    pub fn verify_hash(&mut self, board: &Board, incremental_hash: u64) -> bool {
        self.stats.verifications += 1;
        let full_hash = self.hash_board_quiet(board);
        let matches = full_hash == incremental_hash;

        if !matches {
            self.stats.collisions_detected += 1;
            eprintln!(
                "HASH MISMATCH! Incremental: {}, Full: {}",
                incremental_hash, full_hash
            );
        }

        matches
    }

    /// Get a detailed breakdown of the hash components
    pub fn hash_breakdown(&self, board: &Board) -> String {
        let mut parts = Vec::new();

        parts.push("=== Zobrist Hash Breakdown ===".to_string());

        let mut piece_hash = 0u64;
        for sq in 0..128 {
            if (sq & 0x88) != 0 {
                continue;
            }
            let piece = board.cells[sq];
            if let Some(idx) = Self::piece_index(piece) {
                piece_hash ^= self.pieces[sq][idx];
                let rank = sq >> 4;
                let file = sq & 15;
                parts.push(format!(
                    "  {:?} at {}{}: 0x{:016X}",
                    piece,
                    (b'a' + file as u8) as char,
                    rank + 1,
                    self.pieces[sq][idx]
                ));
            }
        }
        parts.push(format!("Piece hash: 0x{:016X}", piece_hash));

        let cast_hash = self.castling[board.castling as usize];
        parts.push(format!(
            "Castling (0b{:04b}): 0x{:016X}",
            board.castling, cast_hash
        ));

        if let Some(ep) = board.ep {
            let file = ep & 15;
            if file < 8 {
                parts.push(format!(
                    "EP file {}: 0x{:016X}",
                    (b'a' + file as u8) as char,
                    self.ep_file[file]
                ));
            }
        }

        if board.side_white {
            parts.push(format!("Side (white): 0x{:016X}", self.side));
        }

        let total = self.hash_board_quiet(board);
        parts.push(format!("=== Total: 0x{:016X} ===", total));

        parts.join("\n")
    }

    /// Check if a hash looks suspicious (all zeros, all ones, low entropy)
    pub fn is_suspicious_hash(&self, hash: u64) -> bool {
        if hash == 0 || hash == u64::MAX {
            return true;
        }

        // Check for low bit diversity
        let bits_set = hash.count_ones();
        if !(16..=48).contains(&bits_set) {
            return true;
        }

        false
    }

    // =====================
    // Statistics
    // =====================

    /// Get current statistics
    pub fn stats(&self) -> &ZobristStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = ZobristStats::default();
    }

    /// Print statistics report
    pub fn print_stats(&self) {
        println!("=== Zobrist Statistics ===");
        println!("Hash calls:          {}", self.stats.hash_calls);
        println!("Full rehashes:       {}", self.stats.full_rehashes);
        println!("Incremental updates: {}", self.stats.incremental_updates);
        println!("Verifications:       {}", self.stats.verifications);
        println!("Collisions detected: {}", self.stats.collisions_detected);

        if self.stats.hash_calls > 0 {
            let incremental_pct =
                (self.stats.incremental_updates as f64 / self.stats.hash_calls as f64) * 100.0;
            println!("Incremental rate:    {:.2}%", incremental_pct);
        }
    }

    // =====================
    // Persistence
    // =====================

    /// Save Zobrist keys to a file (for reproducibility across runs)
    pub fn save_to_file(&self, path: &str) -> io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write seed
        writer.write_all(&self.seed.to_le_bytes())?;

        // Write piece keys
        for sq in 0..128 {
            for pc in 0..12 {
                writer.write_all(&self.pieces[sq][pc].to_le_bytes())?;
            }
        }

        // Write other keys
        writer.write_all(&self.side.to_le_bytes())?;
        for i in 0..16 {
            writer.write_all(&self.castling[i].to_le_bytes())?;
        }
        for i in 0..8 {
            writer.write_all(&self.ep_file[i].to_le_bytes())?;
        }
        for sq in 0..128 {
            writer.write_all(&self.ep_square[sq].to_le_bytes())?;
        }

        Ok(())
    }

    /// Load Zobrist keys from a file
    pub fn load_from_file(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut buf = [0u8; 8];

        // Read seed
        reader.read_exact(&mut buf)?;
        let seed = u64::from_le_bytes(buf);

        // Read piece keys
        let mut pieces = [[0u64; 12]; 128];
        for row in pieces.iter_mut() {
            for key in row.iter_mut() {
                reader.read_exact(&mut buf)?;
                *key = u64::from_le_bytes(buf);
            }
        }

        // Read other keys
        reader.read_exact(&mut buf)?;
        let side = u64::from_le_bytes(buf);

        let mut castling = [0u64; 16];
        for key in castling.iter_mut() {
            reader.read_exact(&mut buf)?;
            *key = u64::from_le_bytes(buf);
        }

        let mut ep_file = [0u64; 8];
        for key in ep_file.iter_mut() {
            reader.read_exact(&mut buf)?;
            *key = u64::from_le_bytes(buf);
        }

        let mut ep_square = [0u64; 128];
        for key in ep_square.iter_mut() {
            reader.read_exact(&mut buf)?;
            *key = u64::from_le_bytes(buf);
        }

        Ok(Self {
            pieces,
            side,
            castling,
            ep_file,
            ep_square,
            seed,
            stats: ZobristStats::default(),
        })
    }
}

// =====================
// Global Zobrist Instance (Singleton Pattern)
// =====================

static GLOBAL_ZOBRIST: OnceLock<Arc<Mutex<Zobrist>>> = OnceLock::new();

impl Zobrist {
    /// Get or initialize the global Zobrist instance (thread-safe singleton)
    pub fn global() -> Arc<Mutex<Zobrist>> {
        GLOBAL_ZOBRIST
            .get_or_init(|| Arc::new(Mutex::new(Zobrist::new())))
            .clone()
    }

    /// Initialize global instance with custom seed
    pub fn init_global(seed: u64) {
        let _ = GLOBAL_ZOBRIST.set(Arc::new(Mutex::new(Zobrist::with_seed(seed))));
    }
}

// =====================
// Tests
// =====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_hashing() {
        let zob1 = Zobrist::with_seed(12345);
        let zob2 = Zobrist::with_seed(12345);

        // Just test that keys are identical
        assert_eq!(
            zob1.side, zob2.side,
            "Same seed should produce same side key"
        );
        assert_eq!(
            zob1.pieces[0][0], zob2.pieces[0][0],
            "Same seed should produce same piece keys"
        );
    }

    #[test]
    fn test_different_seeds() {
        let zob1 = Zobrist::with_seed(111);
        let zob2 = Zobrist::with_seed(222);

        assert_ne!(
            zob1.side, zob2.side,
            "Different seeds should produce different keys"
        );
    }

    #[test]
    fn test_piece_index() {
        assert_eq!(Zobrist::piece_index(Piece::WP), Some(0));
        assert_eq!(Zobrist::piece_index(Piece::WK), Some(5));
        assert_eq!(Zobrist::piece_index(Piece::BP), Some(6));
        assert_eq!(Zobrist::piece_index(Piece::BK), Some(11));
        assert_eq!(Zobrist::piece_index(Piece::Empty), None);
    }

    #[test]
    fn test_save_load() {
        let zob1 = Zobrist::with_seed(9999);
        let path = "/tmp/zobrist_test.bin";

        zob1.save_to_file(path).expect("Save failed");
        let zob2 = Zobrist::load_from_file(path).expect("Load failed");

        assert_eq!(zob1.seed, zob2.seed);
        assert_eq!(zob1.side, zob2.side);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_zobrist_keys_nonzero() {
        let zob = Zobrist::new();

        assert_ne!(zob.side, 0, "Side key should be non-zero");
        assert_ne!(zob.castling[0], 0, "Castling keys should be non-zero");
        assert_ne!(zob.pieces[0][0], 0, "Piece keys should be non-zero");
    }
}
