//! A tiny opening book.
//!
//! The book maps a position (identified by the first four FEN fields: piece
//! placement, side, castling rights, and en-passant square, i.e. everything
//! that defines the position regardless of move counters) to a short list of
//! reasonable replies in UCI notation. When a position is found we pick one of
//! its moves pseudo-randomly (seeded by the Zobrist key so play varies between
//! games) and verify it is legal before returning it.
//!
//! This is intentionally small, enough to vary the engine's openings and add a
//! genuinely useful feature, not a grandmaster repertoire.

use crate::board::Board;
use crate::movegen::legal_moves;
use crate::moves::Move;

/// `(position_key, candidate_moves)` opening-book entries.
///
/// The `position_key` is the **first three** FEN fields: piece placement, side
/// to move, and castling rights. The en-passant field is deliberately excluded:
/// after, say, `1.e4` the real position carries `ep = e3`, so a key that pinned
/// the ep square would never match. Excluding it keeps the book robust while
/// still distinguishing genuinely different opening positions.
const BOOK: &[(&str, &[&str])] = &[
    // Starting position: offer the four classical first moves.
    (
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq",
        &["e2e4", "d2d4", "c2c4", "g1f3"],
    ),
    // 1.e4: Black's main replies.
    (
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq",
        &["c7c5", "e7e5", "e7e6", "c7c6"],
    ),
    // 1.e4 c5 (Sicilian).
    (
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/8/PPPP1PPP/RNBQKBNR w KQkq",
        &["g1f3", "c2c3", "b1c3"],
    ),
    // 1.e4 e5: King's Knight / Italian / Spanish branch.
    (
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq",
        &["g1f3", "f1c4", "b1c3"],
    ),
    // 1.e4 e5 2.Nf3: defend the pawn.
    (
        "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq",
        &["b8c6", "g8f6"],
    ),
    // 1.e4 e5 2.Nf3 Nc6: Ruy Lopez / Italian / Scotch.
    (
        "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq",
        &["f1b5", "f1c4", "d2d4"],
    ),
    // 1.e4 c6 (Caro-Kann).
    (
        "rnbqkbnr/pp1ppppp/2p5/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq",
        &["d2d4", "b1c3"],
    ),
    // 1.e4 e6 (French).
    (
        "rnbqkbnr/pppp1ppp/4p3/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq",
        &["d2d4", "b1c3"],
    ),
    // 1.d4: Black's main replies.
    (
        "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq",
        &["d7d5", "g8f6", "e7e6"],
    ),
    // 1.d4 d5: Queen's Gambit lines.
    (
        "rnbqkbnr/ppp1pppp/8/3p4/3P4/8/PPP1PPPP/RNBQKBNR w KQkq",
        &["c2c4", "g1f3"],
    ),
    // 1.d4 d5 2.c4 (Queen's Gambit): Black accepts or declines.
    (
        "rnbqkbnr/ppp1pppp/8/3p4/2PP4/8/PP2PPPP/RNBQKBNR b KQkq",
        &["e7e6", "c7c6", "d5c4"],
    ),
    // 1.d4 Nf6: Indian defenses.
    (
        "rnbqkb1r/pppppppp/5n2/8/3P4/8/PPP1PPPP/RNBQKBNR w KQkq",
        &["c2c4", "g1f3"],
    ),
    // 1.d4 Nf6 2.c4: King's Indian / Nimzo / Benoni branch.
    (
        "rnbqkb1r/pppppppp/5n2/8/2PP4/8/PP2PPPP/RNBQKBNR b KQkq",
        &["e7e6", "g7g6", "c7c5"],
    ),
    // 1.c4 (English).
    (
        "rnbqkbnr/pppppppp/8/8/2P5/8/PP1PPPPP/RNBQKBNR b KQkq",
        &["e7e5", "g8f6", "c7c5"],
    ),
    // 1.c4 e5: Reversed Sicilian.
    (
        "rnbqkbnr/pppp1ppp/8/4p3/2P5/8/PP1PPPPP/RNBQKBNR w KQkq",
        &["b1c3", "g2g3"],
    ),
    // 1.Nf3 (Réti).
    (
        "rnbqkbnr/pppppppp/8/8/8/5N2/PPPPPPPP/RNBQKB1R b KQkq",
        &["d7d5", "g8f6", "c7c5"],
    ),
];

/// The position key used for book lookup: piece placement, side, and castling.
fn position_key(board: &Board) -> String {
    board
        .to_fen()
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Look up a book move for the current position, if any. Returns a legal move
/// or `None`.
pub fn book_move(board: &mut Board) -> Option<Move> {
    let key = position_key(board);
    let candidates = BOOK.iter().find(|(k, _)| *k == key).map(|(_, m)| *m)?;

    // Verify legality and collect playable candidates.
    let mut legal = Vec::new();
    legal_moves(board, &mut legal);
    let playable: Vec<Move> = candidates
        .iter()
        .filter_map(|s| Move::from_uci(s))
        .filter(|bm| {
            legal
                .iter()
                .any(|m| m.from == bm.from && m.to == bm.to && m.promotion == bm.promotion)
        })
        .collect();

    if playable.is_empty() {
        return None;
    }
    // Deterministic-but-varied pick seeded by the position key.
    let idx = (board.key as usize) % playable.len();
    Some(playable[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_has_move_for_startpos() {
        let mut b = Board::startpos();
        let mv = book_move(&mut b).expect("book should know the start position");
        // Whatever it picks must be legal.
        let mut legal = Vec::new();
        legal_moves(&mut b, &mut legal);
        assert!(legal.contains(&mv));
    }

    #[test]
    fn book_responds_after_first_move_despite_ep_square() {
        // Regression: after 1.e4 the position has ep=e3. The book key must not
        // depend on the ep field, or this lookup would miss.
        let mut b = Board::startpos();
        b.make_move_struct(Move::from_uci("e2e4").unwrap());
        assert!(b.ep.is_some(), "1.e4 should set an ep square");
        let reply = book_move(&mut b).expect("book must reply to 1.e4");
        let mut legal = Vec::new();
        legal_moves(&mut b, &mut legal);
        assert!(legal.contains(&reply));
    }

    #[test]
    fn book_misses_unknown_position() {
        let mut b = Board::from_fen("8/8/8/4k3/8/8/4K3/8 w - - 0 1").unwrap();
        assert!(book_move(&mut b).is_none());
    }
}
