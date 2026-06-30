//! Binary entry point.
//!
//! Dispatches on the first CLI argument so the one binary serves several roles:
//!
//! * (no args):        the interactive, authenticated terminal application.
//! * `uci`:            speak the UCI protocol (for chess GUIs / tournaments).
//! * `perft <d> [fen]`: print a perft divide for a position.
//! * `bench [depth]`:  fixed-depth search benchmark (nodes/sec).
//! * `help`:           usage.

use rust_chess_engine::auth::{show_welcome_menu, AuthSystem};
use rust_chess_engine::board::{Board, START_FEN};
use rust_chess_engine::perft::divide;
use rust_chess_engine::search::{SearchLimits, Searcher};
use rust_chess_engine::ui::GameController;
use rust_chess_engine::uci::UciEngine;
use std::io;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("uci") => UciEngine::new().run(),
        Some("perft") => run_perft(&args),
        Some("bench") => run_bench(&args),
        Some("help") | Some("-h") | Some("--help") => print_usage(),
        Some(other) => {
            eprintln!("unknown command '{}'\n", other);
            print_usage();
        }
        None => run_interactive(),
    }
}

fn print_usage() {
    println!("Rust Chess Engine\n");
    println!("USAGE:");
    println!("  rust_chess_engine             Launch the interactive terminal app");
    println!("  rust_chess_engine uci         Run as a UCI engine (for GUIs)");
    println!("  rust_chess_engine perft <d> [fen]   Perft divide to depth d");
    println!("  rust_chess_engine bench [depth]     Search benchmark");
    println!("  rust_chess_engine help        Show this message");
}

/// `perft <depth> [fen...]`: print each root move's subtree size and the total.
fn run_perft(args: &[String]) {
    let depth: u32 = match args.get(2).and_then(|s| s.parse().ok()) {
        Some(d) => d,
        None => {
            eprintln!("usage: perft <depth> [fen]");
            return;
        }
    };
    let fen = if args.len() > 3 {
        args[3..].join(" ")
    } else {
        START_FEN.to_string()
    };
    let mut board = match Board::from_fen(&fen) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("bad FEN: {}", e);
            return;
        }
    };

    let start = Instant::now();
    let (entries, total) = divide(&mut board, depth);
    let elapsed = start.elapsed();
    for e in &entries {
        println!("{}: {}", e.mv, e.nodes);
    }
    let nps = if elapsed.as_secs_f64() > 0.0 {
        (total as f64 / elapsed.as_secs_f64()) as u64
    } else {
        0
    };
    println!("\nNodes: {}  Time: {:?}  NPS: {}", total, elapsed, nps);
}

/// `bench [depth]`: fixed-depth search of the start position.
fn run_bench(args: &[String]) {
    let depth: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
    let mut board = Board::startpos();
    let mut searcher = Searcher::new(64);
    let limits = SearchLimits {
        max_depth: depth,
        movetime_ms: None,
        verbose: true,
    };
    let res = searcher.think(&mut board, limits);
    let nps = (res.nodes as u128 * 1000)
        .checked_div(res.time_ms)
        .unwrap_or(0) as u64;
    println!(
        "\nbestmove {}  score {}  depth {}  nodes {}  time {}ms  nps {}",
        res.best_move.map(|m| m.to_uci()).unwrap_or_default(),
        res.score_string(),
        res.depth,
        res.nodes,
        res.time_ms,
        nps
    );
    // Observability: search-quality telemetry.
    println!("telemetry: {}", res.telemetry.summary());
}

/// The authenticated interactive application (the default mode).
fn run_interactive() {
    let mut auth = AuthSystem::new();

    loop {
        match show_welcome_menu() {
            Ok(1) => {
                if let Err(e) = auth.register() {
                    eprintln!("Registration error: {}", e);
                }
            }
            Ok(2) => match auth.login() {
                Ok(true) => {
                    println!("\n🎮 Starting Chess Engine...\n");
                    let mut controller = GameController::new();
                    controller.run();
                    auth.logout();
                }
                Ok(false) => {}
                Err(e) => eprintln!("Login error: {}", e),
            },
            Ok(3) => {
                println!("\n👋 Thank you for playing! Goodbye!\n");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }

        if !auth.is_logged_in() {
            println!("\nPress Enter to continue...");
            let mut dummy = String::new();
            io::stdin().read_line(&mut dummy).ok();
        }
    }
}
