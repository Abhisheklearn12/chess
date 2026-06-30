//! A high-level [`Game`] abstraction tying the board, move history, undo/redo,
//! SAN, PGN, and status detection together behind one tidy API.
//!
//! This is the type an application (the TUI, a future GUI, a test harness)
//! should reach for: it keeps the board and the move list perfectly in sync and
//! exposes intent-level operations (`play_uci`, `play_san`, `undo`, `redo`,
//! `status`, `to_pgn`) rather than raw make/unmake.

use crate::board::{Board, START_FEN};
use crate::engine::{game_status, GameStatus};
use crate::movegen::legal_moves;
use crate::moves::Move;
use crate::pgn::PgnGame;
use crate::san::{move_to_san, san_to_move};

/// A chess game: a position plus its full, navigable move history.
///
/// # Examples
///
/// ```
/// use rust_chess_engine::game::Game;
///
/// let mut game = Game::new();
/// game.play_uci("e2e4").unwrap();
/// game.play_san("c5").unwrap();      // Sicilian
/// assert_eq!(game.san_history(), vec!["e4", "c5"]);
///
/// game.undo();
/// assert_eq!(game.moves().len(), 1);
/// assert!(game.play_uci("e2e5").is_err()); // illegal
/// ```
#[derive(Clone)]
pub struct Game {
    board: Board,
    start_fen: String,
    played: Vec<Move>,
    redo: Vec<Move>,
}

impl Game {
    /// A new game from the standard starting position.
    pub fn new() -> Self {
        Game {
            board: Board::startpos(),
            start_fen: START_FEN.to_string(),
            played: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A new game from an arbitrary FEN.
    pub fn from_fen(fen: &str) -> Result<Self, String> {
        let board = Board::from_fen(fen)?;
        Ok(Game {
            board,
            start_fen: fen.to_string(),
            played: Vec::new(),
            redo: Vec::new(),
        })
    }

    /// Read-only access to the current position.
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Mutable access to the current position (e.g. to run a search).
    pub fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// The moves played so far, in order.
    pub fn moves(&self) -> &[Move] {
        &self.played
    }

    /// All legal moves in the current position.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut b = self.board.clone();
        let mut moves = Vec::new();
        legal_moves(&mut b, &mut moves);
        moves
    }

    /// `true` if `mv` is legal right now.
    pub fn is_legal(&self, mv: Move) -> bool {
        self.legal_moves().contains(&mv)
    }

    /// Play a fully specified [`Move`]; returns `Err` if it is not legal.
    pub fn play(&mut self, mv: Move) -> Result<(), String> {
        if !self.is_legal(mv) {
            return Err(format!("illegal move {}", mv.to_uci()));
        }
        self.board.make_move_struct(mv);
        self.played.push(mv);
        self.redo.clear();
        Ok(())
    }

    /// Play a move given in UCI notation (`e2e4`, `e7e8q`).
    pub fn play_uci(&mut self, uci: &str) -> Result<(), String> {
        let parsed = Move::from_uci(uci).ok_or_else(|| format!("bad UCI '{}'", uci))?;
        // Resolve against legal moves so promotions get the right color, etc.
        let mv = self
            .legal_moves()
            .into_iter()
            .find(|m| {
                m.from == parsed.from && m.to == parsed.to && m.promotion == parsed.promotion
            })
            .ok_or_else(|| format!("illegal move '{}'", uci))?;
        self.play(mv)
    }

    /// Play a move given in SAN (`Nf3`, `exd5`, `O-O`).
    pub fn play_san(&mut self, san: &str) -> Result<(), String> {
        let mv = san_to_move(&self.board, san).ok_or_else(|| format!("illegal SAN '{}'", san))?;
        self.play(mv)
    }

    /// Undo the last move; returns `true` if there was one.
    pub fn undo(&mut self) -> bool {
        if let Some(mv) = self.played.pop() {
            self.board.unmake_move();
            self.redo.push(mv);
            true
        } else {
            false
        }
    }

    /// Redo the most recently undone move; returns `true` if there was one.
    pub fn redo(&mut self) -> bool {
        if let Some(mv) = self.redo.pop() {
            self.board.make_move_struct(mv);
            self.played.push(mv);
            true
        } else {
            false
        }
    }

    /// The game's status (ongoing, mate, stalemate, or a draw rule).
    pub fn status(&self) -> GameStatus {
        let mut b = self.board.clone();
        game_status(&mut b)
    }

    /// `true` if the game has ended (mate, stalemate, or draw).
    pub fn is_over(&self) -> bool {
        !matches!(self.status(), GameStatus::Ongoing)
    }

    /// The move history rendered as SAN strings.
    pub fn san_history(&self) -> Vec<String> {
        let mut b = Board::from_fen(&self.start_fen).unwrap_or_else(|_| Board::startpos());
        let mut out = Vec::with_capacity(self.played.len());
        for &mv in &self.played {
            out.push(move_to_san(&b, mv));
            b.make_move_struct(mv);
        }
        out
    }

    /// Export the game as PGN text.
    pub fn to_pgn(&self) -> String {
        PgnGame::from_moves(&self.start_fen, self.played.clone()).to_pgn()
    }

    /// Replace this game's contents by parsing PGN text.
    pub fn load_pgn(&mut self, text: &str) -> Result<(), String> {
        let parsed = PgnGame::parse(text)?;
        let mut board = Board::from_fen(&parsed.start_fen)?;
        for &mv in &parsed.moves {
            board.make_move_struct(mv);
        }
        self.start_fen = parsed.start_fen;
        self.board = board;
        self.played = parsed.moves;
        self.redo.clear();
        Ok(())
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_undo_redo() {
        let mut g = Game::new();
        g.play_uci("e2e4").unwrap();
        g.play_san("e5").unwrap();
        assert_eq!(g.moves().len(), 2);
        assert_eq!(g.san_history(), vec!["e4", "e5"]);

        assert!(g.undo());
        assert_eq!(g.moves().len(), 1);
        assert!(g.redo());
        assert_eq!(g.moves().len(), 2);
    }

    #[test]
    fn rejects_illegal() {
        let mut g = Game::new();
        assert!(g.play_uci("e2e5").is_err());
        assert!(g.play_san("Qd5").is_err());
    }

    #[test]
    fn detects_fools_mate() {
        let mut g = Game::new();
        for san in ["f3", "e5", "g4", "Qh4"] {
            g.play_san(san).unwrap();
        }
        assert!(matches!(g.status(), GameStatus::Checkmate { .. }));
        assert!(g.is_over());
    }

    #[test]
    fn pgn_roundtrip() {
        let mut g = Game::new();
        for uci in ["e2e4", "e7e5", "g1f3", "b8c6"] {
            g.play_uci(uci).unwrap();
        }
        let pgn = g.to_pgn();
        let mut g2 = Game::new();
        g2.load_pgn(&pgn).unwrap();
        assert_eq!(g2.moves(), g.moves());
    }
}
