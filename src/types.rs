//! Core value types shared across the whole engine.
//!
//! This module defines the fundamental, dependency-free building blocks:
//! colors, pieces, piece kinds, and the `0x88` square representation helpers.
//!
//! # The `0x88` board
//!
//! Squares are stored in a 128-entry array indexed `0..=127`. A square index
//! is split into a rank (`index >> 4`, `0..=7`) and a file (`index & 7`,
//! `0..=7`). The clever part is the test [`square::on_board`]: a valid square
//! has *no* bits set in the mask `0x88` (`0b1000_1000`). Any horizontal or
//! vertical "wrap" off the edge of the board flips one of those bits, so a
//! single bitwise-and rejects illegal destinations cheaply, including the
//! diagonal pawn-capture wraps that plagued the previous implementation.
//!
//! All helpers here are `const fn` where possible and never panic.

use std::fmt;

/// A board square in the `0x88` representation (`0..=127`).
///
/// Only 64 of the 128 indices correspond to real board squares; use
/// [`square::on_board`] to test validity.
pub type Sq = usize;

/// The number of indices in the `0x88` board array.
pub const BOARD_ARRAY_SIZE: usize = 128;

/// Sentinel used where "no square" must be representable in a `Sq` slot.
pub const NO_SQ: Sq = 127;

// ===========================================================================
// Color
// ===========================================================================

/// The side to move / piece owner.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Color {
    White,
    Black,
}

impl Color {
    /// The opposing color.
    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// `true` for [`Color::White`].
    #[inline]
    pub const fn is_white(self) -> bool {
        matches!(self, Color::White)
    }

    /// A stable index: white = 0, black = 1.
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Color::White => 0,
            Color::Black => 1,
        }
    }

    /// The sign used to fold a white-relative score into the color's
    /// perspective: `+1` for white, `-1` for black.
    #[inline]
    pub const fn sign(self) -> i32 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Color::White => "white",
            Color::Black => "black",
        })
    }
}

// ===========================================================================
// PieceKind
// ===========================================================================

/// A piece type, independent of color.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    /// A stable `0..=5` index for table lookups.
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            PieceKind::Pawn => 0,
            PieceKind::Knight => 1,
            PieceKind::Bishop => 2,
            PieceKind::Rook => 3,
            PieceKind::Queen => 4,
            PieceKind::King => 5,
        }
    }

    /// Lowercase SAN/FEN letter for the piece kind (pawn has none in SAN,
    /// returns `'p'` here for completeness).
    #[inline]
    pub const fn to_char(self) -> char {
        match self {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n',
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        }
    }
}

// ===========================================================================
// Piece
// ===========================================================================

/// A colored piece occupying a square, or [`Piece::Empty`].
///
/// The discriminant layout is kept identical to the original engine so the
/// terminal UI and Zobrist tables continue to work unchanged.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Piece {
    Empty,
    WP,
    WN,
    WB,
    WR,
    WQ,
    WK,
    BP,
    BN,
    BB,
    BR,
    BQ,
    BK,
}

impl Piece {
    /// Parse a FEN/SAN piece letter. Unknown characters map to
    /// [`Piece::Empty`] (callers that need strictness should check first).
    pub const fn from_char(c: char) -> Piece {
        match c {
            'P' => Piece::WP,
            'N' => Piece::WN,
            'B' => Piece::WB,
            'R' => Piece::WR,
            'Q' => Piece::WQ,
            'K' => Piece::WK,
            'p' => Piece::BP,
            'n' => Piece::BN,
            'b' => Piece::BB,
            'r' => Piece::BR,
            'q' => Piece::BQ,
            'k' => Piece::BK,
            _ => Piece::Empty,
        }
    }

    /// The FEN/UI letter for this piece (`'.'` for empty).
    pub const fn to_char(self) -> char {
        match self {
            Piece::WP => 'P',
            Piece::WN => 'N',
            Piece::WB => 'B',
            Piece::WR => 'R',
            Piece::WQ => 'Q',
            Piece::WK => 'K',
            Piece::BP => 'p',
            Piece::BN => 'n',
            Piece::BB => 'b',
            Piece::BR => 'r',
            Piece::BQ => 'q',
            Piece::BK => 'k',
            Piece::Empty => '.',
        }
    }

    /// Build a colored piece from a color and kind.
    #[inline]
    pub const fn make(color: Color, kind: PieceKind) -> Piece {
        match (color, kind) {
            (Color::White, PieceKind::Pawn) => Piece::WP,
            (Color::White, PieceKind::Knight) => Piece::WN,
            (Color::White, PieceKind::Bishop) => Piece::WB,
            (Color::White, PieceKind::Rook) => Piece::WR,
            (Color::White, PieceKind::Queen) => Piece::WQ,
            (Color::White, PieceKind::King) => Piece::WK,
            (Color::Black, PieceKind::Pawn) => Piece::BP,
            (Color::Black, PieceKind::Knight) => Piece::BN,
            (Color::Black, PieceKind::Bishop) => Piece::BB,
            (Color::Black, PieceKind::Rook) => Piece::BR,
            (Color::Black, PieceKind::Queen) => Piece::BQ,
            (Color::Black, PieceKind::King) => Piece::BK,
        }
    }

    #[inline]
    pub const fn is_white(self) -> bool {
        matches!(
            self,
            Piece::WP | Piece::WN | Piece::WB | Piece::WR | Piece::WQ | Piece::WK
        )
    }

    #[inline]
    pub const fn is_black(self) -> bool {
        matches!(
            self,
            Piece::BP | Piece::BN | Piece::BB | Piece::BR | Piece::BQ | Piece::BK
        )
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        matches!(self, Piece::Empty)
    }

    /// The color of the piece, or `None` if empty.
    #[inline]
    pub const fn color(self) -> Option<Color> {
        if self.is_white() {
            Some(Color::White)
        } else if self.is_black() {
            Some(Color::Black)
        } else {
            None
        }
    }

    /// The kind of the piece, or `None` if empty.
    #[inline]
    pub const fn kind(self) -> Option<PieceKind> {
        match self {
            Piece::WP | Piece::BP => Some(PieceKind::Pawn),
            Piece::WN | Piece::BN => Some(PieceKind::Knight),
            Piece::WB | Piece::BB => Some(PieceKind::Bishop),
            Piece::WR | Piece::BR => Some(PieceKind::Rook),
            Piece::WQ | Piece::BQ => Some(PieceKind::Queen),
            Piece::WK | Piece::BK => Some(PieceKind::King),
            Piece::Empty => None,
        }
    }

    /// Dense `0..=11` index (white pieces `0..=5`, black `6..=11`) used by the
    /// Zobrist piece-key table. Returns `None` for [`Piece::Empty`].
    #[inline]
    pub const fn zobrist_index(self) -> Option<usize> {
        match self {
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

    /// `true` if `self` belongs to `color`.
    #[inline]
    pub const fn is_color(self, color: Color) -> bool {
        match color {
            Color::White => self.is_white(),
            Color::Black => self.is_black(),
        }
    }
}

// ===========================================================================
// Castling rights
// ===========================================================================

/// White kingside castling right (`O-O`).
pub const CASTLE_WK: u8 = 1;
/// White queenside castling right (`O-O-O`).
pub const CASTLE_WQ: u8 = 2;
/// Black kingside castling right (`O-O`).
pub const CASTLE_BK: u8 = 4;
/// Black queenside castling right (`O-O-O`).
pub const CASTLE_BQ: u8 = 8;

// ===========================================================================
// Square helpers (0x88)
// ===========================================================================

/// `0x88` square-arithmetic helpers. None of these panic.
pub mod square {
    use super::Sq;

    /// Off-board mask: a square is valid iff `index & 0x88 == 0`.
    pub const OFF_BOARD_MASK: usize = 0x88;

    /// Build a square index from a rank and file (both `0..=7`).
    #[inline]
    pub const fn make(rank: i32, file: i32) -> Sq {
        ((rank << 4) | file) as usize
    }

    /// `true` if the index lands on a real board square.
    #[inline]
    pub const fn on_board(s: Sq) -> bool {
        (s & OFF_BOARD_MASK) == 0
    }

    /// `true` if a *signed* index lands on a real board square. Useful when
    /// adding deltas that may go negative.
    #[inline]
    pub const fn on_board_i32(s: i32) -> bool {
        s >= 0 && (s & (OFF_BOARD_MASK as i32)) == 0
    }

    /// The rank (`0..=7`) of a valid square.
    #[inline]
    pub const fn rank(s: Sq) -> i32 {
        (s >> 4) as i32
    }

    /// The file (`0..=7`) of a valid square.
    #[inline]
    pub const fn file(s: Sq) -> i32 {
        (s & 7) as i32
    }

    /// Convert to algebraic coordinates such as `"e4"`. Returns `"??"` for an
    /// off-board index rather than panicking.
    pub fn to_alg(s: Sq) -> String {
        if !on_board(s) {
            return String::from("??");
        }
        let f = file(s) as u8;
        let r = rank(s) as u8;
        format!("{}{}", (b'a' + f) as char, (b'1' + r) as char)
    }

    /// Parse algebraic coordinates such as `"e4"`. Returns `None` on any
    /// malformed input.
    pub fn from_alg(s: &str) -> Option<Sq> {
        let bytes = s.trim().as_bytes();
        if bytes.len() < 2 {
            return None;
        }
        let file = bytes[0].to_ascii_lowercase();
        let rank = bytes[1];
        if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
            return None;
        }
        Some(make((rank - b'1') as i32, (file - b'a') as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_roundtrip() {
        assert_eq!(Color::White.flip(), Color::Black);
        assert_eq!(Color::Black.flip(), Color::White);
        assert_eq!(Color::White.sign(), 1);
        assert_eq!(Color::Black.sign(), -1);
    }

    #[test]
    fn piece_make_and_decompose() {
        for &c in &[Color::White, Color::Black] {
            for &k in &[
                PieceKind::Pawn,
                PieceKind::Knight,
                PieceKind::Bishop,
                PieceKind::Rook,
                PieceKind::Queen,
                PieceKind::King,
            ] {
                let p = Piece::make(c, k);
                assert_eq!(p.color(), Some(c));
                assert_eq!(p.kind(), Some(k));
                assert!(p.is_color(c));
                assert!(!p.is_color(c.flip()));
            }
        }
        assert_eq!(Piece::Empty.color(), None);
        assert_eq!(Piece::Empty.kind(), None);
    }

    #[test]
    fn piece_char_roundtrip() {
        for c in "PNBRQKpnbrqk".chars() {
            assert_eq!(Piece::from_char(c).to_char(), c);
        }
        assert_eq!(Piece::from_char('x'), Piece::Empty);
    }

    #[test]
    fn square_on_board() {
        assert!(square::on_board(square::make(0, 0))); // a1
        assert!(square::on_board(square::make(7, 7))); // h8
        // a2 capturing "left" off the board wraps into the 0x88 region.
        let a2 = square::make(1, 0);
        assert!(!square::on_board_i32(a2 as i32 + 15));
    }

    #[test]
    fn algebraic_roundtrip() {
        for r in 0..8 {
            for f in 0..8 {
                let s = square::make(r, f);
                let alg = square::to_alg(s);
                assert_eq!(square::from_alg(&alg), Some(s));
            }
        }
        assert_eq!(square::from_alg("z9"), None);
        assert_eq!(square::from_alg(""), None);
        assert_eq!(square::to_alg(NO_SQ), "??");
    }
}
