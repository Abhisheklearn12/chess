//! Property/invariant tests that exercise the engine end-to-end.
//!
//! Rather than hand-pick positions, these walk many pseudo-random legal games
//! and assert invariants that must hold after *every* move:
//!
//! * the incrementally maintained Zobrist key equals a fresh full hash;
//! * unmaking every move restores the exact starting FEN;
//! * the side to move is never left able to capture the opponent's king.

use rust_chess_engine::attacks::in_check;
use rust_chess_engine::board::Board;
use rust_chess_engine::engine::{game_status, GameStatus};
use rust_chess_engine::eval::eval_terms;
use rust_chess_engine::movegen::legal_moves;
use rust_chess_engine::pgn::PgnGame;
use rust_chess_engine::search::{search, SearchLimits};

/// A tiny deterministic xorshift RNG so the test is reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn random_games_preserve_invariants() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for _game in 0..40 {
        let mut board = Board::startpos();
        let start_fen = board.to_fen();
        let mut applied = Vec::new();

        for _ply in 0..60 {
            // The side to move must never be able to capture the enemy king.
            let opponent = board.side_to_move().flip();
            assert!(
                !in_check(&board, opponent),
                "side to move can capture enemy king: {}",
                board.to_fen()
            );

            let mut moves = Vec::new();
            legal_moves(&mut board, &mut moves);
            if moves.is_empty() {
                break;
            }
            let mv = moves[(rng.next() as usize) % moves.len()];
            board.make_move_struct(mv);
            applied.push(mv);

            // Incremental key must match a from-scratch hash.
            assert!(
                board.debug_key_ok(),
                "zobrist key drifted after {}: {}",
                mv,
                board.to_fen()
            );

            if board.is_draw() {
                break;
            }
        }

        // Unmaking everything must restore the exact starting position.
        while board.unmake_move().is_some() {}
        assert_eq!(board.to_fen(), start_fen, "unmake did not restore start");
    }
}

#[test]
fn evaluation_is_color_symmetric() {
    // For any position, the White-perspective evaluation must be the exact
    // negation of the same position with colors and ranks mirrored. This is a
    // strong guarantee that no evaluation term carries a hidden color bias.
    let mut rng = Rng(0xDEAD_BEEF_CAFE_1234);
    let mut board = Board::startpos();
    let mut checked = 0;

    for _ in 0..500 {
        let mut moves = Vec::new();
        legal_moves(&mut board, &mut moves);
        if moves.is_empty() || board.is_draw() {
            board = Board::startpos();
            continue;
        }

        let mirror = board.mirror();
        assert_eq!(
            eval_terms(&board).total,
            -eval_terms(&mirror).total,
            "evaluation asymmetry at {}",
            board.to_fen()
        );
        checked += 1;

        let mv = moves[(rng.next() as usize) % moves.len()];
        board.make_move_struct(mv);
    }
    assert!(checked > 100, "did not check enough positions");
}

#[test]
fn engine_self_play_terminates_and_pgn_roundtrips() {
    let mut board = Board::startpos();
    let mut moves = Vec::new();

    for _ in 0..80 {
        match game_status(&mut board) {
            GameStatus::Ongoing => {}
            _ => break,
        }
        let res = search(&mut board, SearchLimits::depth(3));
        let Some(mv) = res.best_move else { break };
        board.make_move_struct(mv);
        moves.push(mv);
        assert!(board.debug_key_ok());
    }

    assert!(!moves.is_empty(), "engine made no moves");

    // The whole game must serialize to PGN and parse back to the same moves.
    let game = PgnGame::from_moves(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        moves.clone(),
    );
    let pgn = game.to_pgn();
    let parsed = PgnGame::parse(&pgn).expect("self-play PGN must parse");
    assert_eq!(parsed.moves, moves, "PGN round-trip changed the moves");
}
