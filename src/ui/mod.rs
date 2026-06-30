// Advanced Interactive Terminal UI for Chess Engine
// Features: Colored board, move highlighting, game stats, interactive menus

pub mod integration;
use crate::engine::{Board, Sq};
pub use integration::GameController;
use std::io::{self, Write};

// ============================================================================
// COLOR CODES & STYLING
// ============================================================================

pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const ITALIC: &str = "\x1b[3m";
    pub const UNDERLINE: &str = "\x1b[4m";

    // Foreground colors
    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    // Bright foreground
    pub const BRIGHT_BLACK: &str = "\x1b[90m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";

    // Background colors
    pub const BG_BLACK: &str = "\x1b[40m";
    pub const BG_RED: &str = "\x1b[41m";
    pub const BG_GREEN: &str = "\x1b[42m";
    pub const BG_YELLOW: &str = "\x1b[43m";
    pub const BG_BLUE: &str = "\x1b[44m";
    pub const BG_MAGENTA: &str = "\x1b[45m";
    pub const BG_CYAN: &str = "\x1b[46m";
    pub const BG_WHITE: &str = "\x1b[47m";

    // Custom backgrounds
    pub const BG_LIGHT: &str = "\x1b[48;5;252m"; // Light square
    pub const BG_DARK: &str = "\x1b[48;5;240m"; // Dark square
    pub const BG_HIGHLIGHT: &str = "\x1b[48;5;226m"; // Highlight move
    pub const BG_SELECTED: &str = "\x1b[48;5;117m"; // Selected square
    pub const BG_CHECK: &str = "\x1b[48;5;196m"; // King in check
}

// ============================================================================
// UNICODE CHESS PIECES
// ============================================================================

pub mod symbols {
    use crate::engine::Piece;

    pub const WHITE_KING: &str = "♔";
    pub const WHITE_QUEEN: &str = "♕";
    pub const WHITE_ROOK: &str = "♖";
    pub const WHITE_BISHOP: &str = "♗";
    pub const WHITE_KNIGHT: &str = "♘";
    pub const WHITE_PAWN: &str = "♙";

    pub const BLACK_KING: &str = "♚";
    pub const BLACK_QUEEN: &str = "♛";
    pub const BLACK_ROOK: &str = "♜";
    pub const BLACK_BISHOP: &str = "♝";
    pub const BLACK_KNIGHT: &str = "♞";
    pub const BLACK_PAWN: &str = "♟";

    pub fn piece_symbol(piece: Piece) -> &'static str {
        match piece {
            Piece::WK => WHITE_KING,
            Piece::WQ => WHITE_QUEEN,
            Piece::WR => WHITE_ROOK,
            Piece::WB => WHITE_BISHOP,
            Piece::WN => WHITE_KNIGHT,
            Piece::WP => WHITE_PAWN,
            Piece::BK => BLACK_KING,
            Piece::BQ => BLACK_QUEEN,
            Piece::BR => BLACK_ROOK,
            Piece::BB => BLACK_BISHOP,
            Piece::BN => BLACK_KNIGHT,
            Piece::BP => BLACK_PAWN,
            _ => " ",
        }
    }
}

// ============================================================================
// BOARD DISPLAY
// ============================================================================

pub struct BoardDisplay {
    pub show_coords: bool,
    pub use_unicode: bool,
    pub highlight_last_move: Option<(Sq, Sq)>,
    pub highlight_squares: Vec<Sq>,
    pub flip_board: bool,
}

impl Default for BoardDisplay {
    fn default() -> Self {
        Self {
            show_coords: true,
            use_unicode: true,
            highlight_last_move: None,
            highlight_squares: Vec::new(),
            flip_board: false,
        }
    }
}

impl BoardDisplay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(&self, board: &Board) {
        self.clear_screen();
        self.print_header();
        self.print_board(board);
        self.print_game_info(board);
    }

    fn clear_screen(&self) {
        print!("\x1b[2J\x1b[H"); // Clear screen and move to top
    }

    fn print_header(&self) {
        use colors::*;
        println!();
        println!(
            "{}{}╔════════════════════════════════════════════════════════════╗{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}║          🏆  RUST CHESS ENGINE - INTERACTIVE UI  🏆        ║{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}╚════════════════════════════════════════════════════════════╝{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!();
    }

    pub fn print_board(&self, board: &Board) {
        use colors::*;

        println!("    ╔════════════════════════════════════════╗");

        let ranks: Vec<i32> = if self.flip_board {
            (0..8).collect()
        } else {
            (0..8).rev().collect()
        };

        for rank in ranks {
            // Rank number
            print!("  {} ║", rank + 1);

            let files: Vec<i32> = if self.flip_board {
                (0..8).rev().collect()
            } else {
                (0..8).collect()
            };

            for file in files {
                let sq = Self::sq_from_coords(rank, file);
                let piece = board.cells[sq];

                // Determine background color
                let bg = self.get_square_color(sq, rank, file);

                // Get piece symbol
                let symbol = if self.use_unicode {
                    symbols::piece_symbol(piece)
                } else {
                    &piece.to_char().to_string()
                };

                // Color the piece
                let piece_color = if piece.is_white() {
                    BRIGHT_WHITE
                } else if piece.is_black() {
                    BRIGHT_BLACK
                } else {
                    ""
                };

                print!("{}{}{}  {}  {}", bg, BOLD, piece_color, symbol, RESET);
            }

            println!("║");
        }

        println!("    ╚════════════════════════════════════════╝");

        // File labels
        if self.flip_board {
            println!("       h    g    f    e    d    c    b    a   ");
        } else {
            println!("       a    b    c    d    e    f    g    h   ");
        }
        println!();
    }

    fn get_square_color(&self, sq: Sq, rank: i32, file: i32) -> &'static str {
        use colors::*;

        // Check if square is highlighted
        if self.highlight_squares.contains(&sq) {
            return BG_SELECTED;
        }

        // Check if it's part of last move
        if let Some((from, to)) = self.highlight_last_move
            && (sq == from || sq == to) {
                return BG_HIGHLIGHT;
            }

        // Normal checkerboard pattern
        if (rank + file) % 2 == 0 {
            BG_DARK
        } else {
            BG_LIGHT
        }
    }

    fn print_game_info(&self, board: &Board) {
        use colors::*;

        println!(
            "{}┌─────────────────────────────────────────────────────────┐{}",
            BRIGHT_BLUE, RESET
        );

        let side = if board.side_white {
            format!("{}White{}", BRIGHT_WHITE, RESET)
        } else {
            format!("{}Black{}", BRIGHT_BLACK, RESET)
        };

        println!(
            "│ {}Turn:{} {}  │  {}Move:{} {}  │  {}Halfmove:{} {}  │",
            BOLD, RESET, side, BOLD, RESET, board.fullmove, BOLD, RESET, board.halfmove_clock
        );

        let castling = format!(
            "{}{}{}{}",
            if board.castling & 1 != 0 { "K" } else { "-" },
            if board.castling & 2 != 0 { "Q" } else { "-" },
            if board.castling & 4 != 0 { "k" } else { "-" },
            if board.castling & 8 != 0 { "q" } else { "-" }
        );

        let ep = match board.ep {
            Some(sq) => Self::sq_to_alg(sq),
            None => "-".to_string(),
        };

        println!(
            "│ {}Castling:{} {}  │  {}En Passant:{} {}  │",
            BOLD, RESET, castling, BOLD, RESET, ep
        );

        println!(
            "{}└─────────────────────────────────────────────────────────┘{}",
            BRIGHT_BLUE, RESET
        );
        println!();
    }

    pub fn print_move_list(&self, moves: &[String]) {
        use colors::*;

        if moves.is_empty() {
            return;
        }

        println!("{}{}═══ Move History ═══{}", BOLD, BRIGHT_MAGENTA, RESET);

        for (i, pair) in moves.chunks(2).enumerate() {
            let move_num = i + 1;
            print!(
                "{}{}{}. {}{}",
                BOLD, BRIGHT_YELLOW, move_num, RESET, pair[0]
            );

            if pair.len() > 1 {
                print!("  {}", pair[1]);
            }
            println!();
        }
        println!();
    }

    pub fn print_analysis(&self, depth: i32, score: i32, nodes: u64, time_ms: u128, pv: &str) {
        use colors::*;

        println!("{}{}─── Engine Analysis ───{}", BOLD, BRIGHT_GREEN, RESET);

        let score_display = if score > 20000 {
            format!("{}Mate in {}{}", BRIGHT_GREEN, (20000 - score) / 100, RESET)
        } else if score < -20000 {
            format!("{}Mate in {}{}", BRIGHT_RED, (20000 + score) / 100, RESET)
        } else {
            format!(
                "{}{:.2}{}",
                if score >= 0 { BRIGHT_GREEN } else { BRIGHT_RED },
                score as f32 / 100.0,
                RESET
            )
        };

        let nps = if time_ms > 0 {
            (nodes as f64 / time_ms as f64 * 1000.0) as u64
        } else {
            0
        };

        println!(
            "│ Depth: {}{}{}  │  Score: {}  │",
            BRIGHT_CYAN, depth, RESET, score_display
        );
        println!(
            "│ Nodes: {}{}{}  │  Time: {}{}ms{}  │  NPS: {}{}{}  │",
            BRIGHT_YELLOW, nodes, RESET, BRIGHT_YELLOW, time_ms, RESET, BRIGHT_YELLOW, nps, RESET
        );

        if !pv.is_empty() {
            println!("│ PV: {}{}{}", BRIGHT_WHITE, pv, RESET);
        }

        println!();
    }

    // Helper functions
    fn sq_from_coords(rank: i32, file: i32) -> Sq {
        ((rank << 4) | file) as usize
    }

    fn sq_to_alg(s: Sq) -> String {
        let r = (s >> 4) as i32;
        let f = (s & 15) as i32;
        if !(0..=7).contains(&r) || !(0..=7).contains(&f) {
            return String::from("??");
        }
        let file = (b'a' + f as u8) as char;
        let rank = (1 + r).to_string();
        format!("{}{}", file, rank)
    }

    fn render_centered(&self, text: &str, width: usize) -> String {
        let text_len = text.len();
        if text_len >= width {
            return text.to_string();
        }

        let padding = (width - text_len) / 2;
        let mut result = " ".repeat(padding);
        result.push_str(text);
        result.push_str(&" ".repeat(width - text_len - padding));
        result
    }
}

// ============================================================================
// INTERACTIVE MENU SYSTEM
// ============================================================================

pub struct Menu {
    title: String,
    options: Vec<MenuOption>,
    selected: usize,
}

pub struct MenuOption {
    pub label: String,
    pub description: String,
    pub action: String,
}

impl Menu {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            options: Vec::new(),
            selected: 0,
        }
    }

    pub fn add_option(&mut self, label: &str, description: &str, action: &str) {
        self.options.push(MenuOption {
            label: label.to_string(),
            description: description.to_string(),
            action: action.to_string(),
        });
    }

    pub fn display(&self) {
        use colors::*;

        println!();
        println!(
            "{}{}╔════════════════════════════════════════════════════════╗{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}║  {}  ║{}",
            BOLD,
            BRIGHT_CYAN,
            self.center_text(&self.title, 52),
            RESET
        );
        println!(
            "{}{}╚════════════════════════════════════════════════════════╝{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!();

        for (i, option) in self.options.iter().enumerate() {
            let prefix = if i == self.selected {
                format!("{}{} ▶ {}", BOLD, BRIGHT_GREEN, RESET)
            } else {
                "   ".to_string()
            };

            let num_color = if i == self.selected {
                BRIGHT_YELLOW
            } else {
                WHITE
            };

            println!(
                "{} {}{}{}. {}{} - {}{}{}",
                prefix,
                BOLD,
                num_color,
                i + 1,
                RESET,
                option.label,
                DIM,
                option.description,
                RESET
            );
        }

        println!();
        print!(
            "{}Select option (1-{}): {}",
            BRIGHT_CYAN,
            self.options.len(),
            RESET
        );
        io::stdout().flush().unwrap();
    }

    pub fn get_selection(&self) -> io::Result<String> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if let Ok(num) = input.trim().parse::<usize>()
            && num > 0 && num <= self.options.len() {
                return Ok(self.options[num - 1].action.clone());
            }

        Ok(String::new())
    }

    fn center_text(&self, text: &str, width: usize) -> String {
        let text_len = text.len();
        if text_len >= width {
            return text.to_string();
        }

        let padding = (width - text_len) / 2;
        let mut result = " ".repeat(padding);
        result.push_str(text);
        result.push_str(&" ".repeat(width - text_len - padding));
        result
    }
}

// ============================================================================
// GAME INTERFACE
// ============================================================================

pub struct GameInterface {
    pub display: BoardDisplay,
    move_history: Vec<String>,
    game_mode: GameMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameMode {
    HumanVsHuman,
    HumanVsEngine,
    EngineVsEngine,
    Analysis,
}

impl Default for GameInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl GameInterface {
    pub fn new() -> Self {
        Self {
            display: BoardDisplay::new(),
            move_history: Vec::new(),
            game_mode: GameMode::HumanVsEngine,
        }
    }

    pub fn set_game_mode(&mut self, mode: GameMode) {
        self.game_mode = mode;
    }

    pub fn show_game_screen(&self, board: &Board) {
        self.display.render(board);

        if !self.move_history.is_empty() {
            self.display.print_move_list(&self.move_history);
        }

        self.print_command_bar();
    }

    pub fn add_move_to_history(&mut self, move_str: String) {
        self.move_history.push(move_str);
    }

    pub fn clear_history(&mut self) {
        self.move_history.clear();
    }

    pub fn highlight_move(&mut self, from: Sq, to: Sq) {
        self.display.highlight_last_move = Some((from, to));
    }

    pub fn clear_highlights(&mut self) {
        self.display.highlight_last_move = None;
        self.display.highlight_squares.clear();
    }

    fn print_command_bar(&self) {
        use colors::*;

        println!(
            "{}{}┌────────────────────────────────────────────────────────┐{}",
            BOLD, BRIGHT_BLUE, RESET
        );
        println!(
            "{}{}│                      COMMANDS                          │{}",
            BOLD, BRIGHT_BLUE, RESET
        );
        println!(
            "{}{}└────────────────────────────────────────────────────────┘{}",
            BOLD, BRIGHT_BLUE, RESET
        );

        let commands = vec![
            ("move <e2e4>", "Make a move"),
            ("undo", "Undo last move"),
            ("redo", "Redo move"),
            ("flip", "Flip board"),
            ("hint", "Get engine hint"),
            ("analyze", "Analyze position"),
            ("save", "Save game"),
            ("load", "Load game"),
            ("resign", "Resign game"),
            ("help", "Show all commands"),
            ("menu", "Main menu"),
        ];

        for chunk in commands.chunks(2) {
            if let Some(&(cmd, desc)) = chunk.first() {
                print!("│ {}{:<15}{} {:<20}", BRIGHT_GREEN, cmd, RESET, desc);
                if let Some(&(c2, d2)) = chunk.get(1) {
                    print!("│ {}{:<15}{} {:<20}", BRIGHT_GREEN, c2, RESET, d2);
                } else {
                    print!("│ {:<36}", "");
                }
                println!("│");
            }
        }

        println!();
    }

    pub fn show_help(&self) {
        use colors::*;

        println!();
        println!(
            "{}{}═══════════════════════════════════════════════════════════{}",
            BOLD, BRIGHT_YELLOW, RESET
        );
        println!(
            "{}{}                    HELP & COMMANDS{}",
            BOLD, BRIGHT_YELLOW, RESET
        );
        println!(
            "{}{}═══════════════════════════════════════════════════════════{}",
            BOLD, BRIGHT_YELLOW, RESET
        );
        println!();

        let help_text = vec![
            (
                "BASIC COMMANDS",
                vec![
                    ("move <from><to>[promo]", "Make a move (e.g., e2e4, e7e8q)"),
                    ("undo / u", "Undo the last move"),
                    ("redo / r", "Redo a previously undone move"),
                    ("resign", "Resign the current game"),
                    ("new", "Start a new game"),
                ],
            ),
            (
                "BOARD CONTROLS",
                vec![
                    ("flip / f", "Flip the board orientation"),
                    ("coords on/off", "Toggle coordinate display"),
                    ("unicode on/off", "Toggle Unicode pieces"),
                ],
            ),
            (
                "ENGINE COMMANDS",
                vec![
                    ("hint", "Get a move suggestion from engine"),
                    ("analyze [depth]", "Deep position analysis"),
                    ("go depth <n>", "Engine thinks to depth n"),
                    ("go time <ms>", "Engine thinks for ms milliseconds"),
                ],
            ),
            (
                "GAME MANAGEMENT",
                vec![
                    ("save [filename]", "Save game to PGN file"),
                    ("load [filename]", "Load game from PGN file"),
                    ("pgn", "Display current game in PGN format"),
                    ("fen", "Display current position as FEN"),
                    ("position fen <fen>", "Set position from FEN string"),
                ],
            ),
            (
                "STATISTICS",
                vec![
                    ("stats", "Show engine statistics"),
                    ("hash", "Show position hash value"),
                    ("eval", "Show static evaluation"),
                    ("perft <depth>", "Run perft test"),
                ],
            ),
        ];

        for (section, commands) in help_text {
            println!("{}{}▶ {}{}", BOLD, BRIGHT_CYAN, section, RESET);
            println!();
            for (cmd, desc) in commands {
                println!("  {}{:<25}{} {}", BRIGHT_GREEN, cmd, RESET, desc);
            }
            println!();
        }

        println!("{}Press Enter to continue...{}", DIM, RESET);
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy).ok();
    }

    pub fn show_thinking_animation(&self, depth: i32, nodes: u64) {
        use colors::*;

        let spinners = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = (nodes / 1000) as usize % spinners.len();

        print!(
            "\r{}{}{}  Thinking... Depth: {} | Nodes: {} {}",
            BRIGHT_YELLOW,
            spinners[idx],
            RESET,
            depth,
            nodes,
            " ".repeat(20)
        );
        io::stdout().flush().unwrap();
    }

    pub fn show_game_result(&self, result: GameResult) {
        use colors::*;

        self.display.clear_screen();

        println!();
        println!(
            "{}{}╔════════════════════════════════════════════════════════╗{}",
            BOLD, BRIGHT_MAGENTA, RESET
        );

        let (_msg, _color) = match result {
            GameResult::WhiteWins => ("🏆  WHITE WINS!  🏆", BRIGHT_GREEN),
            GameResult::BlackWins => ("🏆  BLACK WINS!  🏆", BRIGHT_GREEN),
            GameResult::Draw => ("🤝  DRAW  🤝", BRIGHT_YELLOW),
            GameResult::Stalemate => ("😐  STALEMATE  😐", BRIGHT_BLUE),
            GameResult::Resignation => ("🏳  RESIGNATION  🏳", BRIGHT_RED),
        };

        let msg_text = match result {
            GameResult::WhiteWins => "🏆  WHITE WINS!  🏆",
            GameResult::BlackWins => "🏆  BLACK WINS!  🏆",
            GameResult::Draw => "🤝  DRAW  🤝",
            GameResult::Stalemate => "😐  STALEMATE  😐",
            GameResult::Resignation => "🏳  RESIGNATION  🏳",
        };

        println!(
            "{}{}║  {}  ║{}",
            BOLD,
            BRIGHT_MAGENTA,
            self.display.render_centered(msg_text, 52),
            RESET
        );
        println!(
            "{}{}╚════════════════════════════════════════════════════════╝{}",
            BOLD, BRIGHT_MAGENTA, RESET
        );
        println!();

        println!("{}Press Enter to continue...{}", DIM, RESET);
        let mut dummy = String::new();
        io::stdin().read_line(&mut dummy).ok();
    }

    pub fn prompt_input(&self, prompt: &str) -> String {
        use colors::*;

        print!("{}{} > {}", BRIGHT_CYAN, prompt, RESET);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_string()
    }

    pub fn show_error(&self, message: &str) {
        use colors::*;
        println!("{}❌ Error: {}{}", BRIGHT_RED, message, RESET);
    }

    pub fn show_success(&self, message: &str) {
        use colors::*;
        println!("{}✅ {}{}", BRIGHT_GREEN, message, RESET);
    }

    pub fn show_info(&self, message: &str) {
        use colors::*;
        println!("{}ℹ️  {}{}", BRIGHT_BLUE, message, RESET);
    }

    pub fn show_warning(&self, message: &str) {
        use colors::*;
        println!("{}⚠️  {}{}", BRIGHT_YELLOW, message, RESET);
    }
}

// ============================================================================
// GAME RESULT
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameResult {
    WhiteWins,
    BlackWins,
    Draw,
    Stalemate,
    Resignation,
}

// ============================================================================
// PROGRESS BAR
// ============================================================================

pub struct ProgressBar {
    width: usize,
    current: usize,
    total: usize,
}

impl ProgressBar {
    pub fn new(total: usize) -> Self {
        Self {
            width: 50,
            current: 0,
            total,
        }
    }

    pub fn update(&mut self, current: usize) {
        self.current = current;
        self.render();
    }

    fn render(&self) {
        use colors::*;

        let progress = if self.total > 0 {
            (self.current as f64 / self.total as f64 * self.width as f64) as usize
        } else {
            0
        };

        let percentage = if self.total > 0 {
            (self.current as f64 / self.total as f64 * 100.0) as usize
        } else {
            0
        };

        print!("\r{}[", BRIGHT_BLUE);
        print!("{}{}", BRIGHT_GREEN, "█".repeat(progress));
        print!("{}{}", DIM, "░".repeat(self.width - progress));
        print!(
            "{}] {}{}%{} {}/{}",
            BRIGHT_BLUE, BRIGHT_YELLOW, percentage, RESET, self.current, self.total
        );
        io::stdout().flush().unwrap();
    }

    pub fn finish(&self) {
        println!();
    }
}

// ============================================================================
// TABLE RENDERER
// ============================================================================

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: Vec<String>) -> Self {
        Self {
            headers,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn render(&self) {
        use colors::*;

        // Calculate column widths
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();

        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        // Print top border
        print!("┌");
        for (i, &width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┬");
            }
        }
        println!("┐");

        // Print headers
        print!("│");
        for (i, header) in self.headers.iter().enumerate() {
            print!(" {}{:<width$}{} │", BOLD, header, RESET, width = widths[i]);
        }
        println!();

        // Print separator
        print!("├");
        for (i, &width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┼");
            }
        }
        println!("┤");

        // Print rows
        for row in &self.rows {
            print!("│");
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    print!(" {:<width$} │", cell, width = widths[i]);
                }
            }
            println!();
        }

        // Print bottom border
        print!("└");
        for (i, &width) in widths.iter().enumerate() {
            print!("{}", "─".repeat(width + 2));
            if i < widths.len() - 1 {
                print!("┴");
            }
        }
        println!("┘");
    }
}

// ============================================================================
// STATISTICS DISPLAY
// ============================================================================

pub struct StatsDisplay;

impl StatsDisplay {
    pub fn show_engine_stats(nodes: u64, time_ms: u128, tt_hits: u64, tt_probes: u64) {
        use colors::*;

        println!();
        println!(
            "{}{}╔════════════════════════════════════════════════════════╗{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}║              ENGINE STATISTICS                         ║{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!(
            "{}{}╚════════════════════════════════════════════════════════╝{}",
            BOLD, BRIGHT_CYAN, RESET
        );
        println!();

        let mut table = Table::new(vec!["Metric".to_string(), "Value".to_string()]);

        table.add_row(vec!["Nodes Searched".to_string(), format!("{}", nodes)]);
        table.add_row(vec!["Time Elapsed".to_string(), format!("{}ms", time_ms)]);

        let nps = if time_ms > 0 {
            (nodes as f64 / time_ms as f64 * 1000.0) as u64
        } else {
            0
        };
        table.add_row(vec!["Nodes/Second".to_string(), format!("{}", nps)]);

        table.add_row(vec!["TT Probes".to_string(), format!("{}", tt_probes)]);
        table.add_row(vec!["TT Hits".to_string(), format!("{}", tt_hits)]);

        let hit_rate = if tt_probes > 0 {
            tt_hits as f64 / tt_probes as f64 * 100.0
        } else {
            0.0
        };
        table.add_row(vec!["TT Hit Rate".to_string(), format!("{:.2}%", hit_rate)]);

        table.render();
        println!();
    }

    pub fn show_position_eval(material: i32, positional: i32, total: i32) {
        use colors::*;

        println!();
        println!(
            "{}{}─── Position Evaluation ───{}",
            BOLD, BRIGHT_YELLOW, RESET
        );
        println!();

        let mut table = Table::new(vec!["Component".to_string(), "Score".to_string()]);

        table.add_row(vec![
            "Material".to_string(),
            format!("{:+.2}", material as f32 / 100.0),
        ]);
        table.add_row(vec![
            "Positional".to_string(),
            format!("{:+.2}", positional as f32 / 100.0),
        ]);
        table.add_row(vec!["───────────".to_string(), "───────".to_string()]);
        table.add_row(vec![
            "Total".to_string(),
            format!("{:+.2}", total as f32 / 100.0),
        ]);

        table.render();
        println!();
    }
}

// ============================================================================
// ASCII ART & BANNERS
// ============================================================================

pub struct AsciiArt;

impl AsciiArt {
    pub fn show_welcome_banner() {
        use colors::*;

        println!();
        println!("{}{}", BOLD, BRIGHT_CYAN);
        println!(
            r"
    ╔═══════════════════════════════════════════════════════════════════╗
    ║                                                                   ║
    ║   ██████╗ ██╗   ██╗███████╗████████╗     ██████╗██╗  ██╗███████╗  ║
    ║   ██╔══██╗██║   ██║██╔════╝╚══██╔══╝    ██╔════╝██║  ██║██╔════╝  ║
    ║   ██████╔╝██║   ██║███████╗   ██║       ██║     ███████║█████╗    ║
    ║   ██╔══██╗██║   ██║╚════██║   ██║       ██║     ██╔══██║██╔══╝    ║
    ║   ██║  ██║╚██████╔╝███████║   ██║       ╚██████╗██║  ██║███████╗  ║
    ║   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝        ╚═════╝╚═╝  ╚═╝╚══════╝  ║
    ║                                                                   ║
    ║              ███████╗███╗   ██╗ ██████╗ ██╗███╗   ██╗███████╗     ║
    ║              ██╔════╝████╗  ██║██╔════╝ ██║████╗  ██║██╔════╝     ║
    ║              █████╗  ██╔██╗ ██║██║  ███╗██║██╔██╗ ██║█████╗       ║
    ║              ██╔══╝  ██║╚██╗██║██║   ██║██║██║╚██╗██║██╔══╝       ║
    ║              ███████╗██║ ╚████║╚██████╔╝██║██║ ╚████║███████╗     ║
    ║              ╚══════╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝╚══════╝     ║
    ║                                                                   ║
    ║                    Advanced Chess Engine in Rust                  ║
    ║                           Version v0.2.0                          ║
    ║                                                                   ║
    ╚═══════════════════════════════════════════════════════════════════╝
        "
        );
        println!("{}", RESET);

        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    pub fn show_thinking() {
        use colors::*;
        println!(
            "{}
    🤔 Thinking...
        ",
            BRIGHT_YELLOW
        );
        println!("{}", RESET);
    }

    pub fn show_checkmate() {
        use colors::*;
        println!(
            "{}
    ╔════════════════════════════════╗
    ║        CHECKMATE! ♚♔          ║
    ╚════════════════════════════════╝
        ",
            BRIGHT_RED
        );
        println!("{}", RESET);
    }

    pub fn show_check() {
        use colors::*;
        println!("{}⚠️  CHECK! ⚠️{}", BRIGHT_YELLOW, RESET);
    }
}

// ============================================================================
// NOTIFICATION SYSTEM
// ============================================================================

pub struct Notification {
    message: String,
    kind: NotificationKind,
}

#[derive(Debug, Clone, Copy)]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

impl Notification {
    pub fn new(message: String, kind: NotificationKind) -> Self {
        Self { message, kind }
    }

    pub fn show(&self) {
        use colors::*;

        let (icon, color) = match self.kind {
            NotificationKind::Info => ("ℹ️", BRIGHT_BLUE),
            NotificationKind::Success => ("✅", BRIGHT_GREEN),
            NotificationKind::Warning => ("⚠️", BRIGHT_YELLOW),
            NotificationKind::Error => ("❌", BRIGHT_RED),
        };

        println!();
        println!(
            "{}┌─────────────────────────────────────────────────────────┐{}",
            color, RESET
        );
        println!(
            "{}│ {} {}{}  {:<50} {}│{}",
            color, icon, BOLD, color, self.message, RESET, color
        );
        println!(
            "{}└─────────────────────────────────────────────────────────┘{}",
            color, RESET
        );
        println!();
    }

    pub fn show_timed(&self, duration_ms: u64) {
        self.show();
        std::thread::sleep(std::time::Duration::from_millis(duration_ms));
    }
}

// ============================================================================
// INPUT VALIDATOR
// ============================================================================

pub struct InputValidator;

impl InputValidator {
    pub fn validate_move(input: &str) -> Result<(String, String, Option<char>), String> {
        let input = input.trim().to_lowercase();

        if input.len() < 4 {
            return Err("Move too short. Format: e2e4 or e7e8q".to_string());
        }

        let from = &input[0..2];
        let to = &input[2..4];

        // Validate square format
        if !Self::is_valid_square(from) {
            return Err(format!("Invalid source square: {}", from));
        }

        if !Self::is_valid_square(to) {
            return Err(format!("Invalid destination square: {}", to));
        }

        // Check for promotion
        let promotion = if input.len() >= 5 {
            let promo_char = input.chars().nth(4).unwrap();
            if !['q', 'r', 'b', 'n'].contains(&promo_char) {
                return Err(format!(
                    "Invalid promotion piece: {}. Use q, r, b, or n",
                    promo_char
                ));
            }
            Some(promo_char)
        } else {
            None
        };

        Ok((from.to_string(), to.to_string(), promotion))
    }

    fn is_valid_square(sq: &str) -> bool {
        if sq.len() != 2 {
            return false;
        }

        let bytes = sq.as_bytes();
        let file = bytes[0] as char;
        let rank = bytes[1] as char;

        ('a'..='h').contains(&file) && ('1'..='8').contains(&rank)
    }

    pub fn validate_fen(fen: &str) -> Result<(), String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();

        if parts.is_empty() {
            return Err("Empty FEN string".to_string());
        }

        // Basic FEN validation (can be expanded)
        let ranks: Vec<&str> = parts[0].split('/').collect();
        if ranks.len() != 8 {
            return Err("FEN must have 8 ranks separated by /".to_string());
        }

        Ok(())
    }
}

// ============================================================================
// GAME SETTINGS
// ============================================================================

pub struct GameSettings {
    pub time_control: TimeControl,
    pub engine_strength: EngineStrength,
    pub show_hints: bool,
    pub auto_save: bool,
    pub sound_enabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum TimeControl {
    Unlimited,
    FixedTime(u64),
    TimeAndIncrement { time: u64, increment: u64 },
    MovesInTime { moves: u32, time: u64 },
}

#[derive(Debug, Clone, Copy)]
pub enum EngineStrength {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
    Master,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            time_control: TimeControl::Unlimited,
            engine_strength: EngineStrength::Intermediate,
            show_hints: true,
            auto_save: false,
            sound_enabled: false,
        }
    }
}

impl GameSettings {
    pub fn configure_interactive() -> Self {
        use colors::*;

        let mut settings = Self::default();

        println!();
        println!("{}{}═══ Game Settings ═══{}", BOLD, BRIGHT_MAGENTA, RESET);
        println!();

        // Time control
        println!("{}Time Control:{}", BOLD, RESET);
        println!("  1. Unlimited");
        println!("  2. Fixed time per move (5 seconds)");
        println!("  3. Time + Increment (5 min + 3 sec)");
        println!("  4. Classical (40 moves in 90 minutes)");

        print!("\nSelect (1-4): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        settings.time_control = match input.trim() {
            "2" => TimeControl::FixedTime(5000),
            "3" => TimeControl::TimeAndIncrement {
                time: 300000,
                increment: 3000,
            },
            "4" => TimeControl::MovesInTime {
                moves: 40,
                time: 5400000,
            },
            _ => TimeControl::Unlimited,
        };

        // Engine strength
        println!();
        println!("{}Engine Strength:{}", BOLD, RESET);
        println!("  1. Beginner (ELO ~1200)");
        println!("  2. Intermediate (ELO ~1600)");
        println!("  3. Advanced (ELO ~2000)");
        println!("  4. Expert (ELO ~2200)");
        println!("  5. Master (ELO ~2400)");

        print!("\nSelect (1-5): ");
        io::stdout().flush().unwrap();

        input.clear();
        io::stdin().read_line(&mut input).unwrap();

        settings.engine_strength = match input.trim() {
            "1" => EngineStrength::Beginner,
            "3" => EngineStrength::Advanced,
            "4" => EngineStrength::Expert,
            "5" => EngineStrength::Master,
            _ => EngineStrength::Intermediate,
        };

        settings
    }

    pub fn get_search_depth(&self) -> i32 {
        match self.engine_strength {
            EngineStrength::Beginner => 3,
            EngineStrength::Intermediate => 5,
            EngineStrength::Advanced => 7,
            EngineStrength::Expert => 9,
            EngineStrength::Master => 12,
        }
    }
}

// ============================================================================
// ANIMATION UTILITIES
// ============================================================================

pub struct Animation;

impl Animation {
    pub fn loading(duration_ms: u64) {
        use colors::*;

        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let start = std::time::Instant::now();

        while start.elapsed().as_millis() < duration_ms as u128 {
            for frame in &frames {
                print!("\r{}{} Loading...{}", BRIGHT_CYAN, frame, RESET);
                io::stdout().flush().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(80));

                if start.elapsed().as_millis() >= duration_ms as u128 {
                    break;
                }
            }
        }
        println!("\r                    \r");
    }

    pub fn countdown(seconds: u32) {
        use colors::*;

        for i in (1..=seconds).rev() {
            print!("\r{}Starting in {} seconds...{}", BRIGHT_YELLOW, i, RESET);
            io::stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        println!("\r                              \r");
    }
}

// ============================================================================
// MOVE HISTORY DISPLAY
// ============================================================================

pub struct MoveHistoryDisplay {
    moves: Vec<String>,
    current_move: usize,
}

impl Default for MoveHistoryDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveHistoryDisplay {
    pub fn new() -> Self {
        Self {
            moves: Vec::new(),
            current_move: 0,
        }
    }

    pub fn add_move(&mut self, move_str: String) {
        self.moves.push(move_str);
        self.current_move = self.moves.len();
    }

    pub fn display_full(&self) {
        use colors::*;

        if self.moves.is_empty() {
            println!("{}No moves yet{}", DIM, RESET);
            return;
        }

        println!();
        println!("{}{}═══ Move History ═══{}", BOLD, BRIGHT_MAGENTA, RESET);
        println!();

        for (i, pair) in self.moves.chunks(2).enumerate() {
            let move_num = i + 1;

            let white_move = &pair[0];
            let black_move = if pair.len() > 1 { &pair[1] } else { "" };

            let highlight_white = (i * 2 + 1) == self.current_move;
            let highlight_black = (i * 2 + 2) == self.current_move;

            print!("{}{:3}.{} ", BRIGHT_YELLOW, move_num, RESET);

            if highlight_white {
                print!("{}{}{:<8}{}", BOLD, BRIGHT_GREEN, white_move, RESET);
            } else {
                print!("{:<8}", white_move);
            }

            if !black_move.is_empty() {
                if highlight_black {
                    print!(" {}{}{:<8}{}", BOLD, BRIGHT_GREEN, black_move, RESET);
                } else {
                    print!(" {:<8}", black_move);
                }
            }

            println!();
        }
        println!();
    }

    pub fn clear(&mut self) {
        self.moves.clear();
        self.current_move = 0;
    }
}

// ============================================================================
// CONFIRMATION DIALOG
// ============================================================================

pub struct ConfirmDialog;

impl ConfirmDialog {
    pub fn confirm(message: &str) -> bool {
        use colors::*;

        print!(
            "{}{} {} (y/n): {}",
            BRIGHT_YELLOW, message, RESET, BRIGHT_CYAN
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
    }

    pub fn choose(message: &str, options: &[&str]) -> usize {
        use colors::*;

        println!();
        println!("{}{}{}", BOLD, BRIGHT_CYAN, message);
        println!();

        for (i, option) in options.iter().enumerate() {
            println!("  {}{}. {}{}", BRIGHT_GREEN, i + 1, option, RESET);
        }

        loop {
            print!(
                "\n{}Select option (1-{}): {}",
                BRIGHT_CYAN,
                options.len(),
                RESET
            );
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            if let Ok(choice) = input.trim().parse::<usize>()
                && choice > 0 && choice <= options.len() {
                    return choice - 1;
                }

            println!("{}Invalid choice!{}", BRIGHT_RED, RESET);
        }
    }
}

// ============================================================================
// EXPORT FUNCTIONS
// ============================================================================

pub fn create_main_menu() -> Menu {
    let mut menu = Menu::new("🏠 MAIN MENU");

    menu.add_option("New Game", "Start a new chess game", "new_game");
    menu.add_option("Load Game", "Load a saved game", "load_game");
    menu.add_option("Settings", "Configure game settings", "settings");
    menu.add_option("Tutorial", "Learn how to play", "tutorial");
    menu.add_option("Statistics", "View game statistics", "stats");
    menu.add_option("About", "About this engine", "about");
    menu.add_option("Logout", "Return to login screen", "logout");
    menu.add_option("Exit", "Quit application", "exit");

    menu
}

pub fn create_game_mode_menu() -> Menu {
    let mut menu = Menu::new("🎮 SELECT GAME MODE");

    menu.add_option(
        "Human vs Engine",
        "Play against the computer",
        "human_vs_engine",
    );
    menu.add_option(
        "Human vs Human",
        "Play against another person",
        "human_vs_human",
    );
    menu.add_option(
        "Engine vs Engine",
        "Watch engines battle",
        "engine_vs_engine",
    );
    menu.add_option("Analysis Mode", "Analyze positions freely", "analysis");
    menu.add_option("Back", "Return to main menu", "back");

    menu
}

// ============================================================================
// MODULE TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_validation() {
        assert!(InputValidator::validate_move("e2e4").is_ok());
        assert!(InputValidator::validate_move("e7e8q").is_ok());
        assert!(InputValidator::validate_move("a1h8").is_ok());
        assert!(InputValidator::validate_move("e2").is_err());
        assert!(InputValidator::validate_move("e2e9").is_err());
        assert!(InputValidator::validate_move("z1a1").is_err());
    }

    #[test]
    fn test_square_validation() {
        assert!(InputValidator::is_valid_square("e2"));
        assert!(InputValidator::is_valid_square("a1"));
        assert!(InputValidator::is_valid_square("h8"));
        assert!(!InputValidator::is_valid_square("e9"));
        assert!(!InputValidator::is_valid_square("z1"));
        assert!(!InputValidator::is_valid_square("e"));
    }
}
