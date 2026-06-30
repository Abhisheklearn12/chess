//! Perft: the move generator's correctness oracle.
//!
//! `perft(d)` counts the number of leaf nodes reachable in exactly `d` plies
//! using fully legal moves. Because the reference node counts for many
//! positions are published and independently verified, matching them is a
//! strong proof that move generation, make/unmake, and special-move handling
//! are all correct. The integration tests in `tests/perft.rs` pin these.

use crate::board::Board;
use crate::movegen::legal_moves;
use crate::moves::MoveList;

/// Count the legal-move leaf nodes at depth `depth` from the current position.
///
/// # Examples
///
/// ```
/// use rust_chess_engine::board::Board;
/// use rust_chess_engine::perft::perft;
///
/// let mut board = Board::startpos();
/// assert_eq!(perft(&mut board, 1), 20);
/// assert_eq!(perft(&mut board, 3), 8_902);
/// ```
pub fn perft(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut moves: MoveList = Vec::with_capacity(48);
    legal_moves(board, &mut moves);

    // At depth 1 the number of moves *is* the node count, a useful shortcut.
    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0;
    for mv in moves {
        board.make_move_struct(mv);
        nodes += perft(board, depth - 1);
        board.unmake_move();
    }
    nodes
}

/// One root move and the perft node count beneath it. Useful for debugging a
/// mismatch by comparing against another engine's `divide` output.
#[derive(Clone, Debug)]
pub struct DivideEntry {
    pub mv: String,
    pub nodes: u64,
}

/// "Divide": perft broken down by each legal root move.
pub fn divide(board: &mut Board, depth: u32) -> (Vec<DivideEntry>, u64) {
    let mut moves: MoveList = Vec::with_capacity(48);
    legal_moves(board, &mut moves);
    let mut entries = Vec::with_capacity(moves.len());
    let mut total = 0;
    for mv in moves {
        board.make_move_struct(mv);
        let nodes = if depth <= 1 { 1 } else { perft(board, depth - 1) };
        board.unmake_move();
        total += nodes;
        entries.push(DivideEntry {
            mv: mv.to_uci(),
            nodes,
        });
    }
    entries.sort_by(|a, b| a.mv.cmp(&b.mv));
    (entries, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perft_startpos_shallow() {
        let mut b = Board::startpos();
        assert_eq!(perft(&mut b, 1), 20);
        assert_eq!(perft(&mut b, 2), 400);
        assert_eq!(perft(&mut b, 3), 8902);
        assert_eq!(perft(&mut b, 4), 197281);
    }

    #[test]
    fn perft_kiwipete_shallow() {
        // The famous "Kiwipete" position exercises castling, en passant,
        // promotions, and pins all at once.
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let mut b = Board::from_fen(fen).unwrap();
        assert_eq!(perft(&mut b, 1), 48);
        assert_eq!(perft(&mut b, 2), 2039);
        assert_eq!(perft(&mut b, 3), 97862);
    }
}
