//! Backwards-compatible facade.
//!
//! The engine was originally a single `engine.rs` module. It is now split into
//! focused modules ([`crate::board`], [`crate::movegen`], [`crate::search`],
//! ...), but the terminal UI and external callers still refer to
//! `crate::engine::{Board, Move, Piece, ...}`. This module re-exports the
//! stable surface so that code keeps working unchanged while the internals stay
//! cleanly separated.

pub use crate::board::{Board, START_FEN};
pub use crate::eval::{evaluate, eval_terms, EvalTerms};
pub use crate::moves::{Move, MoveList};
pub use crate::movegen::{gen_captures, gen_moves, has_legal_move, is_king_attacked, legal_moves};
pub use crate::perft::{divide, perft};
pub use crate::search::{ai_move, search, SearchLimits, SearchResult, Searcher};
pub use crate::types::{square, Color, Piece, PieceKind, Sq};

/// Status of a game, derived from the position.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    Ongoing,
    Checkmate { winner: Color },
    Stalemate,
    DrawFiftyMove,
    DrawRepetition,
    DrawInsufficientMaterial,
}

/// Determine the game status of `board` (whether the side to move is mated,
/// stalemated, or the position is drawn by rule).
pub fn game_status(board: &mut Board) -> GameStatus {
    if board.is_fifty_move_draw() {
        return GameStatus::DrawFiftyMove;
    }
    if board.repetition_count() >= 3 {
        return GameStatus::DrawRepetition;
    }
    if board.is_insufficient_material() {
        return GameStatus::DrawInsufficientMaterial;
    }
    if has_legal_move(board) {
        return GameStatus::Ongoing;
    }
    if is_king_attacked(board, board.side_white) {
        GameStatus::Checkmate {
            winner: board.side_to_move().flip(),
        }
    } else {
        GameStatus::Stalemate
    }
}
