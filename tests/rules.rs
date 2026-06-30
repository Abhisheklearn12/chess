//! Chess-rule edge-case tests.
//!
//! Perft already proves bulk move-generation correctness; these target specific
//! rules that are easy to get subtly wrong: en-passant timing, castling-right
//! loss, under-promotion, stalemate vs. checkmate, and the draw rules.

use rust_chess_engine::board::Board;
use rust_chess_engine::engine::{game_status, GameStatus};
use rust_chess_engine::moves::Move;
use rust_chess_engine::movegen::legal_moves;
use rust_chess_engine::types::{square, CASTLE_BK, CASTLE_BQ, CASTLE_WK, CASTLE_WQ};

fn legal(fen: &str) -> Vec<Move> {
    let mut b = Board::from_fen(fen).unwrap();
    let mut m = Vec::new();
    legal_moves(&mut b, &mut m);
    m
}

fn has_move(fen: &str, uci: &str) -> bool {
    let want = Move::from_uci(uci).unwrap();
    legal(fen)
        .iter()
        .any(|m| m.from == want.from && m.to == want.to && m.promotion == want.promotion)
}

// --- En passant -----------------------------------------------------------

#[test]
fn en_passant_available_immediately() {
    // After ...d7d5, the white e5 pawn may capture exd6 e.p.
    assert!(has_move(
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3",
        "e5d6"
    ));
}

#[test]
fn en_passant_expires_after_one_move() {
    // Same structure but no ep square set: the capture is illegal.
    assert!(!has_move(
        "rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3",
        "e5d6"
    ));
}

#[test]
fn en_passant_that_exposes_king_is_illegal() {
    // Classic pin: capturing en passant would expose the white king to a rook
    // on the fifth rank, so it must not be generated.
    let fen = "8/8/8/K2pP2r/8/8/8/7k w - d6 0 1";
    assert!(!has_move(fen, "e5d6"));
}

// --- Castling rights -------------------------------------------------------

#[test]
fn moving_king_forfeits_both_castles() {
    let mut b = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    b.make_move_struct(Move::from_uci("e1e2").unwrap());
    assert_eq!(b.castling & (CASTLE_WK | CASTLE_WQ), 0);
    // Black still has both.
    assert_eq!(b.castling & (CASTLE_BK | CASTLE_BQ), CASTLE_BK | CASTLE_BQ);
}

#[test]
fn moving_rook_forfeits_that_side() {
    let mut b = Board::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    b.make_move_struct(Move::from_uci("h1h2").unwrap());
    assert_eq!(b.castling & CASTLE_WK, 0, "kingside right should be gone");
    assert_eq!(b.castling & CASTLE_WQ, CASTLE_WQ, "queenside survives");
}

#[test]
fn capturing_rook_forfeits_enemy_right() {
    // White rook on a1 captures the black rook on a8, removing Black's
    // queenside castling right.
    let mut b = Board::from_fen("r3k3/8/8/8/8/8/8/R3K2R w KQq - 0 1").unwrap();
    b.make_move_struct(Move::from_uci("a1a8").unwrap());
    assert_eq!(b.castling & CASTLE_BQ, 0);
}

// --- Promotion -------------------------------------------------------------

#[test]
fn all_four_promotions_generated() {
    let fen = "8/P7/8/8/8/8/8/k6K w - - 0 1";
    let moves = legal(fen);
    let promos: Vec<_> = moves
        .iter()
        .filter(|m| m.from == square::from_alg("a7").unwrap())
        .filter_map(|m| m.promotion)
        .collect();
    assert_eq!(promos.len(), 4, "queen, rook, bishop, knight");
}

#[test]
fn underpromotion_to_knight_can_be_played() {
    let mut b = Board::from_fen("8/P7/8/8/8/8/8/k6K w - - 0 1").unwrap();
    b.make_move_struct(Move::from_uci("a7a8n").unwrap());
    assert_eq!(
        b.cells[square::from_alg("a8").unwrap()],
        rust_chess_engine::types::Piece::WN
    );
}

// --- Terminal states -------------------------------------------------------

#[test]
fn detects_checkmate() {
    // Fool's mate.
    let mut b =
        Board::from_fen("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3").unwrap();
    assert!(matches!(game_status(&mut b), GameStatus::Checkmate { .. }));
}

#[test]
fn detects_stalemate() {
    // Black to move, not in check, but has no legal move.
    let mut b = Board::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap();
    assert_eq!(game_status(&mut b), GameStatus::Stalemate);
}

#[test]
fn detects_insufficient_material_draw() {
    let mut b = Board::from_fen("8/8/4k3/8/8/4K3/8/8 w - - 0 1").unwrap();
    assert_eq!(game_status(&mut b), GameStatus::DrawInsufficientMaterial);
}

#[test]
fn detects_fifty_move_draw() {
    let mut b = Board::from_fen("4k3/8/8/8/8/8/8/4K2R w - - 100 80").unwrap();
    assert_eq!(game_status(&mut b), GameStatus::DrawFiftyMove);
}

#[test]
fn detects_threefold_repetition() {
    let mut b = Board::startpos();
    // Shuffle both knights out and back twice to repeat the start position.
    for uci in [
        "g1f3", "g8f6", "f3g1", "f6g8", // back to start (2nd occurrence)
        "g1f3", "g8f6", "f3g1", "f6g8", // back to start (3rd occurrence)
    ] {
        b.make_move_struct(Move::from_uci(uci).unwrap());
    }
    assert!(b.repetition_count() >= 3);
    assert_eq!(game_status(&mut b), GameStatus::DrawRepetition);
}

// --- FEN robustness --------------------------------------------------------

#[test]
fn fen_roundtrip_many_positions() {
    let fens = [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
    ];
    for fen in fens {
        let b = Board::from_fen(fen).unwrap();
        assert_eq!(b.to_fen(), fen, "FEN did not round-trip");
        // And the key must be self-consistent.
        assert!(b.debug_key_ok());
    }
}

#[test]
fn fen_rejects_malformed() {
    assert!(Board::from_fen("").is_err());
    assert!(Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1").is_err());
    assert!(Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR z - - 0 1").is_err());
    assert!(Board::from_fen("rnbqkbnr/pppXpppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").is_err());
}
