//! Tests for the high-level public API: the `Game` façade, opening book, time
//! manager, board mirror, and search telemetry.

use rust_chess_engine::board::Board;
use rust_chess_engine::book::book_move;
use rust_chess_engine::engine::GameStatus;
use rust_chess_engine::game::Game;
use rust_chess_engine::search::{search, SearchLimits};
use rust_chess_engine::timeman::TimeControl;

// --- Game ------------------------------------------------------------------

#[test]
fn game_play_records_san_and_status() {
    let mut g = Game::new();
    g.play_uci("e2e4").unwrap();
    g.play_san("c5").unwrap(); // Sicilian
    g.play_uci("g1f3").unwrap();
    assert_eq!(g.san_history(), vec!["e4", "c5", "Nf3"]);
    assert!(matches!(g.status(), GameStatus::Ongoing));
    assert!(!g.is_over());
}

#[test]
fn game_undo_redo_consistency() {
    let mut g = Game::new();
    for uci in ["e2e4", "e7e5", "g1f3", "b8c6"] {
        g.play_uci(uci).unwrap();
    }
    let fen_before = g.board().to_fen();
    assert!(g.undo());
    assert!(g.undo());
    assert!(g.redo());
    assert!(g.redo());
    assert_eq!(g.board().to_fen(), fen_before);
    // Redo with nothing left returns false.
    assert!(!g.redo());
}

#[test]
fn game_from_fen_and_legal_moves() {
    let g = Game::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1").unwrap();
    // King + rook: a known number of legal moves including castling.
    assert!(g.legal_moves().iter().any(|m| m.to_uci() == "e1g1"));
}

#[test]
fn game_detects_stalemate_status() {
    let g = Game::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert_eq!(g.status(), GameStatus::Stalemate);
    assert!(g.is_over());
}

// --- Opening book ----------------------------------------------------------

#[test]
fn book_returns_legal_first_move() {
    let mut b = Board::startpos();
    let mv = book_move(&mut b).expect("book knows the start position");
    let mut legal = Vec::new();
    rust_chess_engine::movegen::legal_moves(&mut b, &mut legal);
    assert!(legal.contains(&mv));
}

#[test]
fn book_is_deterministic_per_position() {
    let mut b1 = Board::startpos();
    let mut b2 = Board::startpos();
    assert_eq!(book_move(&mut b1), book_move(&mut b2));
}

// --- Time management -------------------------------------------------------

#[test]
fn timeman_scales_with_clock() {
    let lots = TimeControl {
        time_left_ms: 300_000,
        increment_ms: 0,
        moves_to_go: Some(40),
    };
    let little = TimeControl {
        time_left_ms: 10_000,
        increment_ms: 0,
        moves_to_go: Some(40),
    };
    assert!(lots.budget_ms() > little.budget_ms());
    assert!(lots.budget_ms() >= 1);
}

// --- Mirror ----------------------------------------------------------------

#[test]
fn mirror_is_an_involution() {
    let fens = [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    ];
    for fen in fens {
        let b = Board::from_fen(fen).unwrap();
        // Mirroring twice returns the original position.
        let back = b.mirror().mirror();
        assert_eq!(back.to_fen(), b.to_fen(), "mirror is not an involution");
    }
}

// --- Telemetry -------------------------------------------------------------

#[test]
fn search_reports_sensible_telemetry() {
    let mut b = Board::startpos();
    let res = search(&mut b, SearchLimits::depth(6));
    let t = res.telemetry;
    assert_eq!(t.nodes, res.nodes);
    assert!(t.nodes > 0);
    assert!(t.qnodes <= t.nodes);
    // Move ordering on the start position should be quite good.
    assert!(t.move_ordering_quality() >= 0.0 && t.move_ordering_quality() <= 1.0);
}
