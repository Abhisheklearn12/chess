//! Standard Algebraic Notation (SAN).
//!
//! [`move_to_san`] renders a move in the human-readable form used in books and
//! PGN (`Nf3`, `exd5`, `O-O`, `e8=Q+`, `Qh4#`), including the minimal
//! disambiguation required by the SAN rules and a check/checkmate suffix.
//!
//! [`san_to_move`] parses SAN back into a [`Move`] by the robust strategy of
//! rendering every legal move and matching, which guarantees the parser and
//! printer always agree and sidesteps the notoriously fiddly SAN grammar.

use crate::attacks::in_check;
use crate::board::Board;
use crate::movegen::{gen_moves, has_legal_move};
use crate::moves::Move;
use crate::types::{square, PieceKind};

/// Render `mv` (which must be legal in `board`) as SAN.
pub fn move_to_san(board: &Board, mv: Move) -> String {
    let piece = board.cells[mv.from];
    let kind = match piece.kind() {
        Some(k) => k,
        None => return mv.to_uci(), // defensive: empty origin
    };

    // Castling.
    if kind == PieceKind::King && (square::file(mv.from) - square::file(mv.to)).abs() == 2 {
        let mut s = if square::file(mv.to) == 6 {
            "O-O".to_string()
        } else {
            "O-O-O".to_string()
        };
        s.push_str(&check_suffix(board, mv));
        return s;
    }

    let is_ep = kind == PieceKind::Pawn
        && square::file(mv.from) != square::file(mv.to)
        && board.cells[mv.to].is_empty();
    let is_capture = !board.cells[mv.to].is_empty() || is_ep;

    let mut s = String::new();
    if kind == PieceKind::Pawn {
        if is_capture {
            s.push((b'a' + square::file(mv.from) as u8) as char);
            s.push('x');
        }
        s.push_str(&square::to_alg(mv.to));
        if let Some(promo) = mv.promotion {
            s.push('=');
            s.push(promo.to_char().to_ascii_uppercase());
        }
    } else {
        s.push(kind.to_char().to_ascii_uppercase());
        s.push_str(&disambiguation(board, mv, kind));
        if is_capture {
            s.push('x');
        }
        s.push_str(&square::to_alg(mv.to));
    }

    s.push_str(&check_suffix(board, mv));
    s
}

/// The disambiguation string (`""`, a file, a rank, or a full square) needed to
/// distinguish `mv` from other same-kind moves to the same destination.
fn disambiguation(board: &Board, mv: Move, kind: PieceKind) -> String {
    let mut legal = Vec::new();
    gen_moves(board, &mut legal);

    let others: Vec<Move> = legal
        .into_iter()
        .filter(|m| {
            m.to == mv.to
                && m.from != mv.from
                && board.cells[m.from].kind() == Some(kind)
        })
        .collect();

    if others.is_empty() {
        return String::new();
    }

    let same_file = others.iter().any(|m| square::file(m.from) == square::file(mv.from));
    let same_rank = others.iter().any(|m| square::rank(m.from) == square::rank(mv.from));

    if !same_file {
        // File letter is enough.
        ((b'a' + square::file(mv.from) as u8) as char).to_string()
    } else if !same_rank {
        // Rank digit is enough.
        ((b'1' + square::rank(mv.from) as u8) as char).to_string()
    } else {
        // Need the whole origin square.
        square::to_alg(mv.from)
    }
}

/// `"+"`, `"#"`, or `""` depending on whether `mv` gives check or mate.
fn check_suffix(board: &Board, mv: Move) -> String {
    let mut b = board.clone();
    b.make_move_struct(mv);
    let opponent = b.side_to_move();
    if in_check(&b, opponent) {
        if has_legal_move(&mut b) {
            "+".to_string()
        } else {
            "#".to_string()
        }
    } else {
        String::new()
    }
}

/// Parse SAN in the context of `board`, returning the matching legal move.
///
/// Annotation glyphs (`!`, `?`), check/mate markers, and `e.p.` suffixes are
/// ignored, and `0-0` is accepted as a synonym for `O-O`.
pub fn san_to_move(board: &Board, san: &str) -> Option<Move> {
    let want = normalize_san(san);
    if want.is_empty() {
        return None;
    }
    let mut legal = Vec::new();
    gen_moves(board, &mut legal);
    legal
        .into_iter()
        .find(|&m| normalize_san(&move_to_san(board, m)) == want)
}

/// Strip everything that does not affect move identity so user input and our
/// rendering compare equal.
fn normalize_san(san: &str) -> String {
    san.trim()
        .replace('0', "O") // accept 0-0 for O-O
        .chars()
        .filter(|c| !matches!(c, '+' | '#' | '!' | '?' | ' '))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_moves() {
        let b = Board::startpos();
        assert_eq!(move_to_san(&b, Move::from_uci("e2e4").unwrap()), "e4");
        assert_eq!(move_to_san(&b, Move::from_uci("g1f3").unwrap()), "Nf3");
    }

    #[test]
    fn renders_capture_and_check() {
        // Scholar's mate: with the bishop on c4 defending f7, Qxf7 is mate.
        let b = Board::from_fen("rnbqkbnr/pppp1ppp/8/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 0 1")
            .unwrap();
        let mate = Move::from_uci("h5f7").unwrap();
        assert_eq!(move_to_san(&b, mate), "Qxf7#");
    }

    #[test]
    fn renders_castling() {
        let b = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        assert_eq!(move_to_san(&b, Move::from_uci("e1g1").unwrap()), "O-O");
        assert_eq!(move_to_san(&b, Move::from_uci("e1c1").unwrap()), "O-O-O");
    }

    #[test]
    fn disambiguates_knights() {
        // Two white knights (b1, f3) can both reach d2.
        let b = Board::from_fen("4k3/8/8/8/8/5N2/8/1N2K3 w - - 0 1").unwrap();
        let san = move_to_san(&b, Move::from_uci("b1d2").unwrap());
        assert_eq!(san, "Nbd2");
    }

    #[test]
    fn parses_back() {
        let b = Board::startpos();
        assert_eq!(san_to_move(&b, "e4"), Move::from_uci("e2e4"));
        assert_eq!(san_to_move(&b, "Nf3"), Move::from_uci("g1f3"));
        assert_eq!(san_to_move(&b, "Ke2"), None); // illegal
    }
}
