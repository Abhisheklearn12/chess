//! The board: position state, FEN (de)serialization, and fully reversible
//! `make`/`unmake` with an incrementally-maintained Zobrist key.
//!
//! # Correctness model
//!
//! Special-move semantics (captures, en passant, castling, double pawn pushes)
//! are derived from the board state at apply time rather than carried on the
//! [`Move`]. Every applied move pushes a compact [`Undo`] record capturing
//! exactly the information needed to restore the previous position bit-for-bit,
//! including the Zobrist key. This makes the search's hot make/unmake loop both
//! fast (no board cloning) and provably reversible, verified by the perft
//! suite and by [`Board::debug_key_ok`].

use crate::moves::Move;
use crate::types::{
    square, Color, Piece, PieceKind, Sq, BOARD_ARRAY_SIZE, CASTLE_BK, CASTLE_BQ, CASTLE_WK,
    CASTLE_WQ,
};
use crate::zobrist::keys;
use std::fmt;

/// The canonical starting position in FEN.
pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Information required to reverse a single [`Board::make_move`].
#[derive(Clone, Copy)]
struct Undo {
    mv: Move,
    moved: Piece,
    captured: Piece,
    captured_sq: Sq,
    prev_castling: u8,
    prev_ep: Option<Sq>,
    prev_halfmove: u32,
    prev_fullmove: u32,
    prev_side_white: bool,
    prev_key: u64,
}

/// A full chess position plus its move history.
#[derive(Clone)]
pub struct Board {
    /// The `0x88` piece array (128 entries; half are off-board sentinels).
    pub cells: [Piece; BOARD_ARRAY_SIZE],
    /// `true` if it is White's turn.
    pub side_white: bool,
    /// Castling-rights bitmask: [`CASTLE_WK`] | [`CASTLE_WQ`] | [`CASTLE_BK`] |
    /// [`CASTLE_BQ`].
    pub castling: u8,
    /// The square a pawn may capture en passant onto, if any.
    pub ep: Option<Sq>,
    /// Halfmove clock for the fifty-move rule (plies since pawn move/capture).
    pub halfmove_clock: u32,
    /// Fullmove number (starts at 1, increments after Black moves).
    pub fullmove: u32,
    /// Incrementally maintained Zobrist hash of the current position.
    pub key: u64,

    history: Vec<Undo>,
    redo: Vec<Move>,
    /// Zobrist keys of every position reached on the current line, used for
    /// repetition detection. `key_history[i]` is the key *before* the i-th
    /// move in `history`.
    key_history: Vec<u64>,
}

impl Board {
    /// An empty board with White to move and no rights.
    pub fn empty() -> Board {
        Board {
            cells: [Piece::Empty; BOARD_ARRAY_SIZE],
            side_white: true,
            castling: 0,
            ep: None,
            halfmove_clock: 0,
            fullmove: 1,
            key: 0,
            history: Vec::new(),
            redo: Vec::new(),
            key_history: Vec::new(),
        }
    }

    /// The standard chess starting position.
    pub fn startpos() -> Board {
        Board::from_fen(START_FEN).expect("START_FEN is valid")
    }

    /// The color whose turn it is.
    #[inline]
    pub fn side_to_move(&self) -> Color {
        if self.side_white {
            Color::White
        } else {
            Color::Black
        }
    }

    // -----------------------------------------------------------------------
    // FEN
    // -----------------------------------------------------------------------

    /// Parse a position from FEN. Returns `Err` with a human-readable message
    /// on malformed input instead of panicking or silently producing garbage.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_chess_engine::board::Board;
    ///
    /// let board = Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    /// assert_eq!(board.to_fen(), "4k3/8/8/8/8/8/8/4K3 w - - 0 1");
    /// assert!(Board::from_fen("nonsense").is_err());
    /// ```
    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let mut b = Board::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.is_empty() {
            return Err("empty FEN".to_string());
        }

        // 1. Piece placement.
        let ranks: Vec<&str> = parts[0].split('/').collect();
        if ranks.len() != 8 {
            return Err(format!("FEN must have 8 ranks, found {}", ranks.len()));
        }
        for (i, rank_str) in ranks.iter().enumerate() {
            let rank = 7 - i as i32; // FEN lists rank 8 first
            let mut file = 0i32;
            for ch in rank_str.chars() {
                if let Some(skip) = ch.to_digit(10) {
                    file += skip as i32;
                } else {
                    if file > 7 {
                        return Err(format!("rank '{}' overflows the board", rank_str));
                    }
                    let p = Piece::from_char(ch);
                    if p.is_empty() {
                        return Err(format!("invalid piece character '{}'", ch));
                    }
                    b.cells[square::make(rank, file)] = p;
                    file += 1;
                }
            }
            if file != 8 {
                return Err(format!("rank '{}' does not fill 8 files", rank_str));
            }
        }

        // 2. Side to move.
        b.side_white = match parts.get(1) {
            Some(&"w") | None => true,
            Some(&"b") => false,
            Some(other) => return Err(format!("invalid side to move '{}'", other)),
        };

        // 3. Castling rights.
        b.castling = 0;
        if let Some(&c) = parts.get(2)
            && c != "-" {
                for ch in c.chars() {
                    match ch {
                        'K' => b.castling |= CASTLE_WK,
                        'Q' => b.castling |= CASTLE_WQ,
                        'k' => b.castling |= CASTLE_BK,
                        'q' => b.castling |= CASTLE_BQ,
                        _ => return Err(format!("invalid castling character '{}'", ch)),
                    }
                }
            }

        // 4. En passant target.
        b.ep = match parts.get(3) {
            Some(&"-") | None => None,
            Some(sq) => Some(square::from_alg(sq).ok_or_else(|| format!("bad ep square '{}'", sq))?),
        };

        // 5. Halfmove clock and 6. fullmove number.
        if let Some(hc) = parts.get(4) {
            b.halfmove_clock = hc.parse().map_err(|_| format!("bad halfmove clock '{}'", hc))?;
        }
        if let Some(fm) = parts.get(5) {
            b.fullmove = fm.parse().map_err(|_| format!("bad fullmove number '{}'", fm))?;
            if b.fullmove == 0 {
                b.fullmove = 1;
            }
        }

        b.key = keys().hash_board_quiet(&b);
        Ok(b)
    }

    /// Serialize the current position to FEN.
    pub fn to_fen(&self) -> String {
        let mut s = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.cells[square::make(r, f)];
                if p.is_empty() {
                    empty += 1;
                } else {
                    if empty > 0 {
                        s.push_str(&empty.to_string());
                        empty = 0;
                    }
                    s.push(p.to_char());
                }
            }
            if empty > 0 {
                s.push_str(&empty.to_string());
            }
            if r > 0 {
                s.push('/');
            }
        }
        s.push(' ');
        s.push(if self.side_white { 'w' } else { 'b' });
        s.push(' ');
        let mut cast = String::new();
        if self.castling & CASTLE_WK != 0 {
            cast.push('K');
        }
        if self.castling & CASTLE_WQ != 0 {
            cast.push('Q');
        }
        if self.castling & CASTLE_BK != 0 {
            cast.push('k');
        }
        if self.castling & CASTLE_BQ != 0 {
            cast.push('q');
        }
        if cast.is_empty() {
            cast.push('-');
        }
        s.push_str(&cast);
        s.push(' ');
        match self.ep {
            Some(e) => s.push_str(&square::to_alg(e)),
            None => s.push('-'),
        }
        s.push(' ');
        s.push_str(&self.halfmove_clock.to_string());
        s.push(' ');
        s.push_str(&self.fullmove.to_string());
        s
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// The piece on `s` (caller must pass an on-board square).
    #[inline]
    pub fn piece_at(&self, s: Sq) -> Piece {
        self.cells[s]
    }

    /// Locate the king of `color`, or `None` if it is missing (only possible
    /// in malformed/contrived positions).
    pub fn king_square(&self, color: Color) -> Option<Sq> {
        let king = Piece::make(color, PieceKind::King);
        for r in 0..8 {
            for f in 0..8 {
                let s = square::make(r, f);
                if self.cells[s] == king {
                    return Some(s);
                }
            }
        }
        None
    }

    /// Number of plies recorded in the move history.
    #[inline]
    pub fn ply(&self) -> usize {
        self.history.len()
    }

    // -----------------------------------------------------------------------
    // Make / Unmake (core fast path used by search and movegen)
    // -----------------------------------------------------------------------

    /// Apply `mv` to the board, updating all state and the Zobrist key, and
    /// push an undo record. Does not validate legality; callers must only
    /// pass moves produced by the generator. Does not touch the redo stack.
    pub fn make_move_struct(&mut self, mv: Move) {
        let k = keys();
        let from = mv.from;
        let to = mv.to;
        let moved = self.cells[from];
        let color = moved.color().expect("moving an empty square");
        let mut h = self.key;

        // --- Detect en passant capture (pawn moves diagonally to empty ep) ---
        let is_ep = matches!(moved.kind(), Some(PieceKind::Pawn))
            && Some(to) == self.ep
            && square::file(from) != square::file(to);

        let (captured, captured_sq) = if is_ep {
            // The captured pawn sits on the moving side's destination file but
            // on the origin rank.
            let cap_sq = square::make(square::rank(from), square::file(to));
            (self.cells[cap_sq], cap_sq)
        } else {
            (self.cells[to], to)
        };

        // Save undo information before mutating.
        let undo = Undo {
            mv,
            moved,
            captured,
            captured_sq,
            prev_castling: self.castling,
            prev_ep: self.ep,
            prev_halfmove: self.halfmove_clock,
            prev_fullmove: self.fullmove,
            prev_side_white: self.side_white,
            prev_key: self.key,
        };

        // --- Move the piece on the board + hash ---
        // Remove mover from origin.
        h ^= k.piece_key(moved, from);
        self.cells[from] = Piece::Empty;

        // Remove any captured piece.
        if !captured.is_empty() {
            h ^= k.piece_key(captured, captured_sq);
            self.cells[captured_sq] = Piece::Empty;
        }

        // Place mover (or promotion) on destination.
        let placed = match mv.promotion {
            Some(promo) => Piece::make(color, promo.kind().unwrap_or(PieceKind::Queen)),
            None => moved,
        };
        h ^= k.piece_key(placed, to);
        self.cells[to] = placed;

        // --- Castling: move the rook too ---
        if matches!(moved.kind(), Some(PieceKind::King))
            && (square::file(from) - square::file(to)).abs() == 2
        {
            let rank = square::rank(from);
            let (rook_from, rook_to) = if square::file(to) == 6 {
                (square::make(rank, 7), square::make(rank, 5)) // kingside
            } else {
                (square::make(rank, 0), square::make(rank, 3)) // queenside
            };
            let rook = self.cells[rook_from];
            h ^= k.piece_key(rook, rook_from);
            self.cells[rook_from] = Piece::Empty;
            h ^= k.piece_key(rook, rook_to);
            self.cells[rook_to] = rook;
        }

        // --- Update castling rights (king/rook moved or rook captured) ---
        let mut new_castling = self.castling;
        new_castling &= Self::castle_mask_for(from);
        new_castling &= Self::castle_mask_for(to);
        if new_castling != self.castling {
            h ^= k.castling[self.castling as usize];
            h ^= k.castling[new_castling as usize];
            self.castling = new_castling;
        }

        // --- En passant target square ---
        if let Some(old_ep) = self.ep {
            h ^= k.ep_file[square::file(old_ep) as usize];
        }
        let new_ep = if matches!(moved.kind(), Some(PieceKind::Pawn))
            && (square::rank(to) - square::rank(from)).abs() == 2
        {
            Some(square::make((square::rank(from) + square::rank(to)) / 2, square::file(from)))
        } else {
            None
        };
        if let Some(ep) = new_ep {
            h ^= k.ep_file[square::file(ep) as usize];
        }
        self.ep = new_ep;

        // --- Clocks ---
        if matches!(moved.kind(), Some(PieceKind::Pawn)) || !captured.is_empty() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock += 1;
        }
        if !self.side_white {
            self.fullmove += 1;
        }

        // --- Flip side ---
        self.side_white = !self.side_white;
        h ^= k.side;
        self.key = h;

        self.key_history.push(undo.prev_key);
        self.history.push(undo);
    }

    /// Convenience wrapper matching the original UI API.
    #[inline]
    pub fn make_move(&mut self, from: Sq, to: Sq, promotion: Option<Piece>) {
        self.make_move_struct(Move {
            from,
            to,
            promotion,
        });
    }

    /// Reverse the most recent [`Board::make_move`]. Returns the move that was
    /// undone, or `None` if there was nothing to undo. Does not touch the redo
    /// stack (the UI-facing [`Board::undo_move`] layers redo on top).
    pub fn unmake_move(&mut self) -> Option<Move> {
        let u = self.history.pop()?;
        self.key_history.pop();

        let from = u.mv.from;
        let to = u.mv.to;

        // Reverse rook movement for castling.
        if matches!(u.moved.kind(), Some(PieceKind::King))
            && (square::file(from) - square::file(to)).abs() == 2
        {
            let rank = square::rank(from);
            let (rook_from, rook_to) = if square::file(to) == 6 {
                (square::make(rank, 7), square::make(rank, 5))
            } else {
                (square::make(rank, 0), square::make(rank, 3))
            };
            let rook = self.cells[rook_to];
            self.cells[rook_to] = Piece::Empty;
            self.cells[rook_from] = rook;
        }

        // Put the mover back, clear destination, restore captured piece.
        self.cells[from] = u.moved;
        self.cells[to] = Piece::Empty;
        if !u.captured.is_empty() {
            self.cells[u.captured_sq] = u.captured;
        }

        // Restore scalar state directly from the undo record.
        self.castling = u.prev_castling;
        self.ep = u.prev_ep;
        self.halfmove_clock = u.prev_halfmove;
        self.fullmove = u.prev_fullmove;
        self.side_white = u.prev_side_white;
        self.key = u.prev_key;

        Some(u.mv)
    }

    /// The castling-rights mask that survives a piece leaving or arriving on
    /// `s`. ANDing rights with this clears the appropriate bits when a king or
    /// rook moves, or when a rook is captured on its home square.
    #[inline]
    fn castle_mask_for(s: Sq) -> u8 {
        // e1, e8 are the king squares; corners are the rook squares.
        const A1: usize = 0x00;
        const E1: usize = 0x04;
        const H1: usize = 0x07;
        const A8: usize = 0x70;
        const E8: usize = 0x74;
        const H8: usize = 0x77;
        match s {
            E1 => !(CASTLE_WK | CASTLE_WQ),
            E8 => !(CASTLE_BK | CASTLE_BQ),
            H1 => !CASTLE_WK,
            A1 => !CASTLE_WQ,
            H8 => !CASTLE_BK,
            A8 => !CASTLE_BQ,
            _ => 0xFF,
        }
    }

    // -----------------------------------------------------------------------
    // UI-facing move application with redo support
    // -----------------------------------------------------------------------

    /// Play a move as part of real gameplay: applies it and discards any
    /// pending redo history (a fresh move invalidates the redo branch).
    pub fn play_move(&mut self, mv: Move) {
        self.make_move_struct(mv);
        self.redo.clear();
    }

    /// Undo the last move and remember it so it can be redone. Returns `true`
    /// if a move was undone.
    pub fn undo_move(&mut self) -> bool {
        if let Some(mv) = self.unmake_move() {
            self.redo.push(mv);
            true
        } else {
            false
        }
    }

    /// Re-apply the most recently undone move. Returns `true` if one was
    /// redone.
    pub fn redo_move(&mut self) -> bool {
        if let Some(mv) = self.redo.pop() {
            self.make_move_struct(mv);
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Draw detection
    // -----------------------------------------------------------------------

    /// Number of times the current position's key has occurred on this line
    /// (including the current occurrence). A value `>= 3` is a threefold
    /// repetition; `>= 2` during search is the usual cheap draw heuristic.
    pub fn repetition_count(&self) -> usize {
        let mut count = 1;
        // Only positions reachable without an irreversible move can repeat, so
        // we never scan further back than the halfmove clock.
        let limit = self.halfmove_clock as usize;
        let start = self.key_history.len().saturating_sub(limit);
        for &k in self.key_history[start..].iter() {
            if k == self.key {
                count += 1;
            }
        }
        count
    }

    /// `true` if the position is drawn by the fifty-move rule (100 plies).
    #[inline]
    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove_clock >= 100
    }

    /// `true` if neither side has sufficient material to deliver checkmate
    /// (K vs K, K+minor vs K, and K+B vs K+B with same-colored bishops).
    pub fn is_insufficient_material(&self) -> bool {
        let mut knights = 0;
        let mut bishops_light = 0;
        let mut bishops_dark = 0;
        for r in 0..8 {
            for f in 0..8 {
                let s = square::make(r, f);
                match self.cells[s].kind() {
                    None | Some(PieceKind::King) => {}
                    Some(PieceKind::Knight) => knights += 1,
                    Some(PieceKind::Bishop) => {
                        if (r + f) % 2 == 0 {
                            bishops_dark += 1;
                        } else {
                            bishops_light += 1;
                        }
                    }
                    // Any pawn, rook, or queen is sufficient.
                    _ => return false,
                }
            }
        }
        let bishops = bishops_light + bishops_dark;
        match (knights, bishops) {
            (0, 0) => true,             // K vs K
            (1, 0) => true,             // K+N vs K
            (0, 1) => true,             // K+B vs K
            (0, _) => bishops_light == 0 || bishops_dark == 0, // all bishops one color
            _ => false,
        }
    }

    /// `true` for any drawn-by-rule position (fifty-move, threefold, or
    /// insufficient material).
    pub fn is_draw(&self) -> bool {
        self.is_fifty_move_draw()
            || self.repetition_count() >= 3
            || self.is_insufficient_material()
    }

    // -----------------------------------------------------------------------
    // Observability / debugging
    // -----------------------------------------------------------------------

    /// `true` if the incrementally maintained key matches a fresh full hash.
    /// Used by tests and by the search's optional self-check.
    pub fn debug_key_ok(&self) -> bool {
        self.key == keys().hash_board_quiet(self)
    }

    /// Produce the color-and-rank mirror of this position: every piece is
    /// reflected to the opposite rank with its color swapped, the side to move
    /// flips, and castling rights / en-passant are mirrored accordingly.
    ///
    /// The mirrored position is strategically identical from the other side's
    /// point of view, which makes it the perfect tool for testing that the
    /// evaluation is free of any color bias (see the eval-symmetry test).
    pub fn mirror(&self) -> Board {
        let mut b = Board::empty();
        for r in 0..8 {
            for f in 0..8 {
                let s = square::make(r, f);
                let p = self.cells[s];
                if let (Some(c), Some(k)) = (p.color(), p.kind()) {
                    let ms = square::make(7 - r, f);
                    b.cells[ms] = Piece::make(c.flip(), k);
                }
            }
        }
        b.side_white = !self.side_white;
        // Swap white/black castling bits.
        let c = self.castling;
        let mut nc = 0;
        if c & CASTLE_WK != 0 {
            nc |= CASTLE_BK;
        }
        if c & CASTLE_WQ != 0 {
            nc |= CASTLE_BQ;
        }
        if c & CASTLE_BK != 0 {
            nc |= CASTLE_WK;
        }
        if c & CASTLE_BQ != 0 {
            nc |= CASTLE_WQ;
        }
        b.castling = nc;
        b.ep = self
            .ep
            .map(|e| square::make(7 - square::rank(e), square::file(e)));
        b.halfmove_clock = self.halfmove_clock;
        b.fullmove = self.fullmove;
        b.key = keys().hash_board_quiet(&b);
        b
    }

    /// A plain ASCII rendering of the board (rank 8 at the top).
    pub fn ascii(&self) -> String {
        let mut s = String::new();
        s.push_str("  +------------------------+\n");
        for r in (0..8).rev() {
            s.push_str(&format!("{} |", r + 1));
            for f in 0..8 {
                s.push(' ');
                s.push(self.cells[square::make(r, f)].to_char());
            }
            s.push_str(" |\n");
        }
        s.push_str("  +------------------------+\n");
        s.push_str("    a b c d e f g h\n");
        s
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.ascii())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_fen_roundtrips() {
        let b = Board::startpos();
        assert_eq!(b.to_fen(), START_FEN);
    }

    #[test]
    fn fen_rejects_bad_input() {
        assert!(Board::from_fen("not a fen").is_err());
        assert!(Board::from_fen("8/8/8/8/8/8/8 w - - 0 1").is_err()); // 7 ranks
        assert!(Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1").is_err());
    }

    #[test]
    fn make_unmake_restores_everything() {
        let mut b = Board::startpos();
        let original = b.to_fen();
        let key = b.key;
        b.make_move_struct(Move::from_uci("e2e4").unwrap());
        assert!(b.debug_key_ok());
        b.make_move_struct(Move::from_uci("c7c5").unwrap());
        assert!(b.debug_key_ok());
        b.unmake_move();
        b.unmake_move();
        assert_eq!(b.to_fen(), original);
        assert_eq!(b.key, key);
    }

    #[test]
    fn ep_capture_make_unmake() {
        // White pawn on e5, black plays d7d5, white captures e5xd6 e.p.
        let mut b = Board::from_fen("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3")
            .unwrap();
        let fen = b.to_fen();
        b.make_move_struct(Move::from_uci("e5d6").unwrap());
        assert!(b.debug_key_ok());
        // The black d5 pawn must be gone.
        assert_eq!(b.cells[square::from_alg("d5").unwrap()], Piece::Empty);
        assert_eq!(b.cells[square::from_alg("d6").unwrap()], Piece::WP);
        b.unmake_move();
        assert_eq!(b.to_fen(), fen);
    }

    #[test]
    fn castling_make_unmake() {
        let mut b =
            Board::from_fen("rnbqk2r/pppp1ppp/5n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4")
                .unwrap();
        let fen = b.to_fen();
        b.make_move_struct(Move::from_uci("e1g1").unwrap()); // O-O
        assert!(b.debug_key_ok());
        assert_eq!(b.cells[square::from_alg("g1").unwrap()], Piece::WK);
        assert_eq!(b.cells[square::from_alg("f1").unwrap()], Piece::WR);
        assert_eq!(b.castling & (CASTLE_WK | CASTLE_WQ), 0);
        b.unmake_move();
        assert_eq!(b.to_fen(), fen);
    }

    #[test]
    fn promotion_make_unmake() {
        let mut b = Board::from_fen("8/P7/8/8/8/8/8/k6K w - - 0 1").unwrap();
        let fen = b.to_fen();
        b.make_move_struct(Move::from_uci("a7a8q").unwrap());
        assert!(b.debug_key_ok());
        assert_eq!(b.cells[square::from_alg("a8").unwrap()], Piece::WQ);
        b.unmake_move();
        assert_eq!(b.to_fen(), fen);
    }

    #[test]
    fn insufficient_material() {
        assert!(Board::from_fen("8/8/8/4k3/8/8/4K3/8 w - - 0 1")
            .unwrap()
            .is_insufficient_material());
        assert!(Board::from_fen("8/8/8/4k3/8/8/4KN2/8 w - - 0 1")
            .unwrap()
            .is_insufficient_material());
        assert!(!Board::from_fen("8/8/8/4k3/8/8/4KR2/8 w - - 0 1")
            .unwrap()
            .is_insufficient_material());
    }
}
