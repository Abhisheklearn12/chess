//! Notation round-trip and edge-case tests for SAN, PGN, and UCI.

use rust_chess_engine::board::Board;
use rust_chess_engine::moves::Move;
use rust_chess_engine::pgn::PgnGame;
use rust_chess_engine::san::{move_to_san, san_to_move};
use rust_chess_engine::uci::UciEngine;

// --- SAN -------------------------------------------------------------------

fn san(fen: &str, uci: &str) -> String {
    let b = Board::from_fen(fen).unwrap();
    move_to_san(&b, Move::from_uci(uci).unwrap())
}

#[test]
fn san_pawn_push_and_capture() {
    assert_eq!(san("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1", "e2e4"), "e4");
    // Pawn capture uses the origin file.
    assert_eq!(
        san("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "e4d5"),
        "exd5"
    );
}

#[test]
fn san_promotion_with_check() {
    // Pawn promotes to a queen giving check.
    let s = san("4k3/1P6/8/8/8/8/8/4K3 w - - 0 1", "b7b8q");
    assert!(s.starts_with("b8=Q"), "got {}", s);
}

#[test]
fn san_en_passant() {
    let s = san(
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        "e5d6",
    );
    assert_eq!(s, "exd6");
}

#[test]
fn san_rank_disambiguation() {
    // Two rooks on the same file (a1, a3) both reach a2: need the rank.
    let s = san("4k3/8/8/8/8/R7/8/R3K3 w - - 0 1", "a1a2");
    assert_eq!(s, "R1a2");
}

#[test]
fn san_full_square_disambiguation() {
    // Three queens can reach e4 from e2, c4, and g2 ... use a known triple.
    // Queens on a1, h1, and e4 target; here two queens share file and rank with
    // the mover, forcing a full-square disambiguator.
    let b = Board::from_fen("4k3/8/8/8/Q6Q/8/8/Q3K3 w - - 0 1").unwrap();
    // Qa1, Qa4, Qh4: moving Qa4 to d4, a-file shares with Qa1, rank-4 shares
    // with Qh4, so disambiguation must be the full square "a4".
    let s = move_to_san(&b, Move::from_uci("a4d4").unwrap());
    assert_eq!(s, "Qa4d4");
}

#[test]
fn san_parse_roundtrip_for_all_legal_moves() {
    // Every legal move must render to SAN and parse back to itself.
    let b = Board::from_fen(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    )
    .unwrap();
    let mut moves = Vec::new();
    rust_chess_engine::movegen::gen_moves(&b, &mut moves);
    for mv in moves {
        let s = move_to_san(&b, mv);
        assert_eq!(san_to_move(&b, &s), Some(mv), "SAN '{}' did not round-trip", s);
    }
}

// --- PGN -------------------------------------------------------------------

#[test]
fn pgn_black_to_move_start() {
    // A game that starts with Black to move should render "1... ".
    let start = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    let moves = vec![Move::from_uci("d5e4").unwrap()];
    let game = PgnGame::from_moves(start, moves.clone());
    let pgn = game.to_pgn();
    assert!(pgn.contains("1..."), "expected black-first numbering:\n{}", pgn);
    let parsed = PgnGame::parse(&pgn).unwrap();
    assert_eq!(parsed.moves, moves);
}

#[test]
fn pgn_setup_tag_for_custom_start() {
    let start = "4k3/8/8/8/8/8/8/4K2R w K - 0 1";
    let game = PgnGame::from_moves(start, vec![Move::from_uci("e1g1").unwrap()]);
    let pgn = game.to_pgn();
    assert!(pgn.contains("[FEN \""), "custom start needs a FEN tag:\n{}", pgn);
    assert!(pgn.contains("O-O"));
}

// --- UCI -------------------------------------------------------------------

fn uci_run(cmds: &[&str]) -> String {
    let mut e = UciEngine::new();
    let mut out = Vec::new();
    for c in cmds {
        e.handle_line(c, &mut out);
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn uci_position_fen_then_print() {
    let out = uci_run(&[
        "position fen 4k3/8/8/8/8/8/8/4K2R w K - 0 1",
        "d",
    ]);
    assert!(out.contains("4k3/8/8/8/8/8/8/4K2R w K - 0 1"));
}

#[test]
fn uci_unknown_command_is_ignored() {
    // Unknown commands must not crash and must keep the loop alive.
    let mut e = UciEngine::new();
    let mut out = Vec::new();
    assert!(e.handle_line("frobnicate the widget", &mut out));
}

#[test]
fn uci_go_movetime_returns_bestmove() {
    let out = uci_run(&["position startpos", "go movetime 100"]);
    assert!(out.lines().any(|l| l.starts_with("bestmove ")));
}
