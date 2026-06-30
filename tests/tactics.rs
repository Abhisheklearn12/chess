//! Tactical regression tests: positions with a single best move that the
//! search must find. These guard against regressions in evaluation, ordering,
//! quiescence, and mate detection.
//!
//! Each case gives a FEN, a search depth, and the expected best move in UCI.

use rust_chess_engine::board::Board;
use rust_chess_engine::search::{search, SearchLimits};

fn best_move(fen: &str, depth: i32) -> String {
    let mut b = Board::from_fen(fen).expect("valid FEN");
    search(&mut b, SearchLimits::depth(depth))
        .best_move
        .map(|m| m.to_uci())
        .unwrap_or_default()
}

#[test]
fn mate_in_one_back_rank() {
    // Rook delivers back-rank mate.
    assert_eq!(best_move("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", 3), "a1a8");
}

#[test]
fn mate_in_two() {
    // Sam Loyd's "Excelsior" mate-in-two: 1.Ra6! bxa6 2.b7#.
    let fen = "kbK5/pp6/1P6/8/8/8/8/R7 w - - 0 1";
    let mut b = Board::from_fen(fen).unwrap();
    let res = search(&mut b, SearchLimits::depth(5));
    assert_eq!(res.best_move.map(|m| m.to_uci()), Some("a1a6".to_string()));
    assert!(
        res.score >= rust_chess_engine::eval::MATE_THRESHOLD,
        "expected a forced mate, got score {}",
        res.score
    );
}

#[test]
fn wins_hanging_piece() {
    // Black queen sits undefended on d5; white rook on d2 should grab it.
    assert_eq!(best_move("4k3/8/8/3q4/8/8/3R4/4K3 w - - 0 1", 4), "d2d5");
}

#[test]
fn avoids_losing_capture() {
    // White rook can "win" a pawn on d5 but it is defended by a pawn; the
    // engine should NOT play the losing Rxd5 (SEE < 0). Any other sensible
    // move is fine; we just assert it is not the bad capture.
    let mv = best_move("4k3/8/2p5/3p4/8/8/3R3P/4K3 w - - 0 1", 5);
    assert_ne!(mv, "d2d5", "engine walked into a losing capture");
}

#[test]
fn promotes_to_queen() {
    // White pawn on a7 with a clear path should promote.
    let mv = best_move("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", 6);
    assert_eq!(mv, "a7a8q");
}
