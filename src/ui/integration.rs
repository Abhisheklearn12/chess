//! Integration layer between the terminal UI and the chess engine.
//!
//! [`GameController`] owns the game state (a [`Board`] plus the played move
//! list) and drives the various game modes. It records moves in SAN, runs real
//! searches for engine moves / hints / analysis, evaluates positions with the
//! real evaluator, detects game end through [`game_status`], and saves/loads
//! games as PGN.

use crate::board::Board;
use crate::engine::{game_status, GameStatus};
use crate::eval::eval_terms;
use crate::moves::Move;
use crate::movegen::legal_moves;
use crate::pgn::PgnGame;
use crate::san::move_to_san;
use crate::search::{search, SearchLimits};
use crate::types::{square, Color, Piece, Sq};
use crate::ui::{
    AsciiArt, ConfirmDialog, GameInterface, GameMode, GameResult, GameSettings, InputValidator,
    MoveHistoryDisplay, Notification, NotificationKind, StatsDisplay, create_game_mode_menu,
    create_main_menu,
};
use std::io;
use std::time::Instant;

/// Orchestrates a game: state, history, settings, and the main input loops.
pub struct GameController {
    board: Board,
    interface: GameInterface,
    settings: GameSettings,
    move_history: MoveHistoryDisplay,
    /// The mainline moves actually played (source of truth for SAN/PGN).
    played: Vec<Move>,
    /// Moves that were undone and can be redone.
    redo: Vec<Move>,
    /// FEN the current game started from (for PGN export).
    start_fen: String,
    game_active: bool,
}

impl GameController {
    pub fn new() -> Self {
        Self {
            board: Board::startpos(),
            interface: GameInterface::new(),
            settings: GameSettings::default(),
            move_history: MoveHistoryDisplay::new(),
            played: Vec::new(),
            redo: Vec::new(),
            start_fen: crate::board::START_FEN.to_string(),
            game_active: false,
        }
    }

    pub fn run(&mut self) {
        AsciiArt::show_welcome_banner();

        loop {
            let menu = create_main_menu();
            menu.display();

            match menu.get_selection() {
                Ok(action) => match action.as_str() {
                    "new_game" => self.start_new_game(),
                    "load_game" => self.load_game(),
                    "settings" => self.configure_settings(),
                    "tutorial" => self.show_tutorial(),
                    "stats" => self.show_statistics(),
                    "about" => self.show_about(),
                    "logout"
                        if ConfirmDialog::confirm("Are you sure you want to logout?") => {
                            break;
                        }
                    "exit"
                        if ConfirmDialog::confirm("Are you sure you want to exit?") => {
                            std::process::exit(0);
                        }
                    _ => {}
                },
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Move application (keeps board, history, and PGN list in lock-step)
    // -----------------------------------------------------------------------

    /// Commit a legal move: SAN is computed *before* the position changes, then
    /// the board, the played-move list, and the display history are updated.
    fn commit(&mut self, mv: Move) {
        let san = move_to_san(&self.board, mv);
        self.board.make_move_struct(mv);
        self.played.push(mv);
        self.redo.clear();
        self.interface.highlight_move(mv.from, mv.to);
        self.move_history.add_move(san.clone());
        self.interface.add_move_to_history(san);
    }

    fn undo(&mut self) {
        if let Some(mv) = self.played.pop() {
            self.board.unmake_move();
            self.redo.push(mv);
            self.rebuild_history();
        }
    }

    fn redo_last(&mut self) {
        if let Some(mv) = self.redo.pop() {
            let san = move_to_san(&self.board, mv);
            self.board.make_move_struct(mv);
            self.played.push(mv);
            self.move_history.add_move(san.clone());
        }
    }

    /// Recompute the SAN move-history display by replaying the played moves on
    /// a fresh board (used after an undo).
    fn rebuild_history(&mut self) {
        self.move_history.clear();
        self.interface.clear_history();
        let mut b = Board::from_fen(&self.start_fen).unwrap_or_else(|_| Board::startpos());
        for &mv in &self.played {
            let san = move_to_san(&b, mv);
            b.make_move_struct(mv);
            self.move_history.add_move(san.clone());
            self.interface.add_move_to_history(san);
        }
    }

    // -----------------------------------------------------------------------
    // Game setup & loops
    // -----------------------------------------------------------------------

    fn start_new_game(&mut self) {
        let mode_menu = create_game_mode_menu();
        mode_menu.display();

        let mode = match mode_menu.get_selection() {
            Ok(action) => match action.as_str() {
                "human_vs_engine" => GameMode::HumanVsEngine,
                "human_vs_human" => GameMode::HumanVsHuman,
                "engine_vs_engine" => GameMode::EngineVsEngine,
                "analysis" => GameMode::Analysis,
                _ => return,
            },
            Err(_) => return,
        };

        self.interface.set_game_mode(mode);
        self.reset_game();

        Notification::new(
            "Game started! Good luck!".to_string(),
            NotificationKind::Success,
        )
        .show_timed(1200);

        match mode {
            GameMode::HumanVsEngine => self.play_human_vs_engine(),
            GameMode::HumanVsHuman => self.play_human_vs_human(),
            GameMode::EngineVsEngine => self.play_engine_vs_engine(),
            GameMode::Analysis => self.analysis_mode(),
        }
    }

    fn reset_game(&mut self) {
        self.board = Board::startpos();
        self.start_fen = crate::board::START_FEN.to_string();
        self.played.clear();
        self.redo.clear();
        self.move_history.clear();
        self.interface.clear_history();
        self.interface.clear_highlights();
        self.game_active = true;
    }

    fn play_human_vs_engine(&mut self) {
        while self.game_active {
            self.interface.show_game_screen(&self.board);

            if let Some(result) = self.check_game_end() {
                self.interface.show_game_result(result);
                self.game_active = false;
                break;
            }

            if self.board.side_white {
                if !self.handle_human_move() {
                    break;
                }
            } else {
                self.engine_play_move();
            }
        }
        self.show_end_game_options();
    }

    fn play_human_vs_human(&mut self) {
        while self.game_active {
            self.interface.show_game_screen(&self.board);
            if let Some(result) = self.check_game_end() {
                self.interface.show_game_result(result);
                self.game_active = false;
                break;
            }
            if !self.handle_human_move() {
                break;
            }
        }
        self.show_end_game_options();
    }

    fn play_engine_vs_engine(&mut self) {
        Notification::new(
            "Watching engine battle...".to_string(),
            NotificationKind::Info,
        )
        .show();

        while self.game_active {
            self.interface.show_game_screen(&self.board);
            if let Some(result) = self.check_game_end() {
                self.interface.show_game_result(result);
                self.game_active = false;
                break;
            }
            if !self.engine_play_move() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        self.show_end_game_options();
    }

    /// Have the engine search and play a move. Returns `false` if it had none.
    fn engine_play_move(&mut self) -> bool {
        Notification::new("Engine is thinking...".to_string(), NotificationKind::Info).show();
        let limits = SearchLimits {
            max_depth: self.settings.get_search_depth(),
            movetime_ms: Some(4000),
            verbose: false,
        };
        let start = Instant::now();
        let res = search(&mut self.board, limits);
        let Some(mv) = res.best_move else {
            self.interface.show_error("Engine couldn't find a move!");
            self.game_active = false;
            return false;
        };
        let san = move_to_san(&self.board, mv);
        self.commit(mv);
        Notification::new(
            format!(
                "Engine played {} ({}, depth {}, {} nodes, {}ms)",
                san,
                res.score_string(),
                res.depth,
                res.nodes,
                start.elapsed().as_millis()
            ),
            NotificationKind::Success,
        )
        .show_timed(900);
        true
    }

    fn analysis_mode(&mut self) {
        self.reset_game();
        loop {
            self.interface.show_game_screen(&self.board);
            let input = self.interface.prompt_input("analysis");
            if input.is_empty() {
                continue;
            }
            let parts: Vec<&str> = input.split_whitespace().collect();
            match parts[0] {
                "move" | "m" => {
                    if let Some(s) = parts.get(1)
                        && let Err(e) = self.try_play(s) {
                            self.interface.show_error(&e);
                        }
                }
                "analyze" | "a" => {
                    let depth = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
                    self.run_analysis(depth);
                }
                "eval" | "e" => self.show_evaluation(),
                "perft" => {
                    let depth = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
                    self.run_perft(depth);
                }
                "fen" => self.interface.show_info(&self.board.to_fen()),
                "undo" | "u" => self.undo(),
                "redo" | "r" => self.redo_last(),
                "flip" | "f" => {
                    self.interface.display.flip_board = !self.interface.display.flip_board;
                }
                "back" | "exit" | "quit" => break,
                _ => {
                    if self.try_play(parts[0]).is_err() {
                        self.interface.show_error("Unknown command");
                    }
                }
            }
        }
    }

    fn handle_human_move(&mut self) -> bool {
        loop {
            let side = if self.board.side_white { "White" } else { "Black" };
            let input = self.interface.prompt_input(&format!("{} to move", side));
            if input.is_empty() {
                continue;
            }
            let parts: Vec<&str> = input.split_whitespace().collect();
            match parts[0] {
                "move" | "m" => {
                    if let Some(s) = parts.get(1) {
                        match self.try_play(s) {
                            Ok(()) => return true,
                            Err(e) => self.interface.show_error(&e),
                        }
                    } else {
                        self.interface.show_error("Usage: move e2e4");
                    }
                }
                "undo" | "u" => {
                    self.undo();
                    return true;
                }
                "redo" | "r" => {
                    self.redo_last();
                    return true;
                }
                "hint" | "h" => self.show_hint(),
                "analyze" => self.run_analysis(self.settings.get_search_depth()),
                "eval" => self.show_evaluation(),
                "fen" => self.interface.show_info(&self.board.to_fen()),
                "flip" | "f" => {
                    self.interface.display.flip_board = !self.interface.display.flip_board;
                    return true;
                }
                "save" => {
                    self.save_game();
                }
                "resign" => {
                    if ConfirmDialog::confirm("Are you sure you want to resign?") {
                        self.game_active = false;
                        return false;
                    }
                }
                "menu" | "quit" | "exit" => {
                    if ConfirmDialog::confirm("Quit current game?") {
                        self.game_active = false;
                        return false;
                    }
                }
                "help" => {
                    self.interface.show_help();
                    return true;
                }
                _ => match self.try_play(parts[0]) {
                    Ok(()) => return true,
                    Err(e) => self
                        .interface
                        .show_error(&format!("Invalid command or move: {}", e)),
                },
            }
        }
    }

    /// Parse a UCI or SAN move string, verify legality, and commit it.
    fn try_play(&mut self, move_str: &str) -> Result<(), String> {
        // Accept either UCI (e2e4) or SAN (Nf3) input.
        let mv = self
            .parse_uci_legal(move_str)
            .or_else(|| crate::san::san_to_move(&self.board, move_str))
            .ok_or_else(|| format!("illegal or unrecognized move '{}'", move_str))?;
        self.commit(mv);
        Ok(())
    }

    /// Validate a UCI move string against the legal move list.
    fn parse_uci_legal(&mut self, move_str: &str) -> Option<Move> {
        let (from_str, to_str, promo_char) = InputValidator::validate_move(move_str).ok()?;
        let from = square::from_alg(&from_str)?;
        let to = square::from_alg(&to_str)?;
        let promotion = promo_char.map(|c| Piece::from_char(c.to_ascii_uppercase()));
        let mut legal = Vec::new();
        legal_moves(&mut self.board, &mut legal);
        legal
            .into_iter()
            .find(|m| m.from == from && m.to == to && m.promotion == promotion)
    }

    // -----------------------------------------------------------------------
    // Engine features
    // -----------------------------------------------------------------------

    fn show_hint(&mut self) {
        Notification::new("Calculating best move...".to_string(), NotificationKind::Info).show();
        let limits = SearchLimits {
            max_depth: self.settings.get_search_depth(),
            movetime_ms: Some(2500),
            verbose: false,
        };
        let res = search(&mut self.board, limits);
        match res.best_move {
            Some(mv) => {
                let san = move_to_san(&self.board, mv);
                Notification::new(
                    format!("Suggested: {} ({})", san, res.score_string()),
                    NotificationKind::Success,
                )
                .show_timed(1500);
            }
            None => self.interface.show_error("Could not calculate a hint"),
        }
    }

    fn run_analysis(&mut self, depth: i32) {
        use crate::ui::colors::*;
        println!("{}{}Running analysis to depth {}...{}", BOLD, BRIGHT_CYAN, depth, RESET);
        let limits = SearchLimits {
            max_depth: depth,
            movetime_ms: Some(5000),
            verbose: false,
        };
        let res = search(&mut self.board, limits);
        let pv = self.pv_to_san(&res.pv);
        self.interface
            .display
            .print_analysis(res.depth, res.score, res.nodes, res.time_ms, &pv);
        let mut dummy = String::new();
        println!("{}Press Enter to continue...{}", DIM, RESET);
        io::stdin().read_line(&mut dummy).ok();
    }

    /// Render a principal variation (a list of moves) as space-separated SAN.
    fn pv_to_san(&self, pv: &[Move]) -> String {
        let mut b = self.board.clone();
        let mut out = Vec::new();
        for &mv in pv {
            // A PV move could in theory be stale; stop if it is not legal.
            let mut legal = Vec::new();
            legal_moves(&mut b, &mut legal);
            if !legal.contains(&mv) {
                break;
            }
            out.push(move_to_san(&b, mv));
            b.make_move_struct(mv);
        }
        out.join(" ")
    }

    fn show_evaluation(&self) {
        let terms = eval_terms(&self.board);
        StatsDisplay::show_position_eval(terms.material, terms.positional, terms.total);
    }

    fn run_perft(&mut self, depth: u32) {
        use crate::ui::colors::*;
        let start = Instant::now();
        let (entries, total) = crate::perft::divide(&mut self.board, depth);
        for e in &entries {
            println!("  {}{}{}: {}", BRIGHT_GREEN, e.mv, RESET, e.nodes);
        }
        println!(
            "{}perft({}) = {} in {:?}{}",
            BOLD,
            depth,
            total,
            start.elapsed(),
            RESET
        );
        let mut dummy = String::new();
        println!("Press Enter to continue...");
        io::stdin().read_line(&mut dummy).ok();
    }

    fn check_game_end(&mut self) -> Option<GameResult> {
        match game_status(&mut self.board) {
            GameStatus::Ongoing => None,
            GameStatus::Checkmate { winner } => Some(if winner == Color::White {
                GameResult::WhiteWins
            } else {
                GameResult::BlackWins
            }),
            GameStatus::Stalemate => Some(GameResult::Stalemate),
            GameStatus::DrawFiftyMove
            | GameStatus::DrawRepetition
            | GameStatus::DrawInsufficientMaterial => Some(GameResult::Draw),
        }
    }

    // -----------------------------------------------------------------------
    // Persistence (PGN)
    // -----------------------------------------------------------------------

    fn show_end_game_options(&mut self) {
        let options = vec!["Save Game (PGN)", "Show PGN", "Main Menu"];
        let choice = ConfirmDialog::choose("What would you like to do?", &options);
        match choice {
            0 => self.save_game(),
            1 => {
                let game = PgnGame::from_moves(&self.start_fen, self.played.clone());
                println!("\n{}", game.to_pgn());
                let mut dummy = String::new();
                println!("Press Enter to continue...");
                io::stdin().read_line(&mut dummy).ok();
            }
            _ => {}
        }
    }

    fn save_game(&self) {
        let filename = self.interface.prompt_input("Save as (filename)");
        if filename.is_empty() {
            self.interface.show_warning("Save cancelled");
            return;
        }
        let path = if filename.ends_with(".pgn") {
            filename
        } else {
            format!("{}.pgn", filename)
        };
        let game = PgnGame::from_moves(&self.start_fen, self.played.clone());
        match std::fs::write(&path, game.to_pgn()) {
            Ok(()) => self.interface.show_success(&format!("Saved to {}", path)),
            Err(e) => self.interface.show_error(&format!("Could not save: {}", e)),
        }
    }

    fn load_game(&mut self) {
        let filename = self.interface.prompt_input("Load PGN (filename)");
        if filename.is_empty() {
            self.interface.show_warning("Load cancelled");
            return;
        }
        let path = if filename.ends_with(".pgn") {
            filename
        } else {
            format!("{}.pgn", filename)
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.interface.show_error(&format!("Could not read {}: {}", path, e));
                return;
            }
        };
        match PgnGame::parse(&text) {
            Ok(game) => {
                self.start_fen = game.start_fen.clone();
                self.board = Board::from_fen(&self.start_fen).unwrap_or_else(|_| Board::startpos());
                self.played.clear();
                self.redo.clear();
                for mv in game.moves {
                    self.board.make_move_struct(mv);
                    self.played.push(mv);
                }
                self.rebuild_history();
                self.interface.set_game_mode(GameMode::Analysis);
                self.game_active = true;
                self.interface
                    .show_success(&format!("Loaded {} ({} moves)", path, self.played.len()));
                self.analysis_mode();
            }
            Err(e) => self.interface.show_error(&format!("Bad PGN: {}", e)),
        }
    }

    // -----------------------------------------------------------------------
    // Misc menu screens
    // -----------------------------------------------------------------------

    fn configure_settings(&mut self) {
        self.settings = GameSettings::configure_interactive();
        Notification::new("Settings updated!".to_string(), NotificationKind::Success).show();
    }

    fn show_tutorial(&self) {
        self.interface.show_help();
    }

    fn show_statistics(&self) {
        StatsDisplay::show_engine_stats(0, 0, 0, 0);
    }

    fn show_about(&self) {
        use crate::ui::colors::*;
        println!();
        println!(
            "{}{}╔════════════════════════════════════════════════════════╗{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}║              RUST CHESS ENGINE                          ║{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}╚════════════════════════════════════════════════════════╝{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!();
        println!("  {}Features:{}", BOLD, RESET);
        println!("    • 0x88 board, perft-verified legal move generation");
        println!("    • Iterative-deepening PVS + transposition table + Zobrist");
        println!("    • Tapered piece-square-table evaluation");
        println!("    • SAN/PGN, UCI protocol, opening book");
        println!("    • Interactive terminal UI with multiple game modes");
        println!();
        println!("{}Press Enter to continue...{}", DIM, RESET);
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy).ok();
    }

    /// 0x88 square to algebraic, retained for any external callers.
    #[allow(dead_code)]
    fn sq_to_alg(s: Sq) -> String {
        square::to_alg(s)
    }
}

impl Default for GameController {
    fn default() -> Self {
        Self::new()
    }
}
