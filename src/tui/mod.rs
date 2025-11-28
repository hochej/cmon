//! Terminal User Interface for cmon
//!
//! This module provides an interactive TUI for monitoring Slurm clusters.
//! It features:
//! - Real-time job and node status with automatic refresh
//! - Dual-channel event architecture (priority input, backpressure-aware data)
//! - Keyboard-driven navigation
//! - Multi-account support
//! - Graceful degradation when data is unavailable

pub mod app;
pub mod event;
pub mod runtime;
pub mod theme;
pub mod ui;

use std::io::{self, stdout, IsTerminal};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{bail, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::tui::app::App;
use crate::tui::runtime::{
    create_channels, run_event_loop, spawn_animation_tick, spawn_fairshare_fetcher,
    spawn_input_task, spawn_job_fetcher, spawn_node_fetcher, spawn_scheduler_stats_fetcher,
    spawn_throttle_decay, FetcherThrottle, TuiRuntime,
};

/// Terminal capability requirements for TUI mode
#[derive(Debug)]
pub struct TerminalCapabilities {
    pub is_tty: bool,
    pub term_type: String,
    pub supports_alternate_screen: bool,
    pub supports_colors: bool,
}

impl TerminalCapabilities {
    /// Detect terminal capabilities
    pub fn detect() -> Self {
        let is_tty = stdout().is_terminal();
        let term_type = std::env::var("TERM").unwrap_or_default();

        // Check for known problematic terminals
        let supports_alternate_screen = !matches!(
            term_type.as_str(),
            "dumb" | "" | "unknown"
        );

        // Most modern terminals support colors
        let supports_colors = supports_alternate_screen
            && !term_type.starts_with("vt")
            || term_type.contains("color")
            || term_type.contains("256")
            || term_type.contains("xterm")
            || term_type.contains("screen")
            || term_type.contains("tmux")
            || term_type.contains("kitty")
            || term_type.contains("alacritty");

        Self {
            is_tty,
            term_type,
            supports_alternate_screen,
            supports_colors,
        }
    }

    /// Check if terminal is suitable for TUI mode
    pub fn is_suitable(&self) -> bool {
        self.is_tty && self.supports_alternate_screen
    }

    /// Get error message for unsuitable terminal
    pub fn error_message(&self) -> String {
        if !self.is_tty {
            "TUI mode requires an interactive terminal (stdout is not a TTY).\n\
             Hint: Use non-TUI commands like 'cmon jobs' or 'cmon status' instead.".to_string()
        } else if !self.supports_alternate_screen {
            format!(
                "Terminal type '{}' may not support TUI mode.\n\
                 Hint: Set TERM to a supported value (e.g., xterm-256color) or use CLI mode.",
                if self.term_type.is_empty() { "(unset)" } else { &self.term_type }
            )
        } else {
            "Unknown terminal capability issue.".to_string()
        }
    }
}

/// Run the TUI application
pub async fn run_tui() -> Result<()> {
    // Check terminal capabilities before attempting TUI mode
    let capabilities = TerminalCapabilities::detect();
    if !capabilities.is_suitable() {
        bail!("{}", capabilities.error_message());
    }

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Create the application state
    let mut app = App::new();

    // Create dual channels
    let (input_tx, input_rx, data_tx, data_rx) = create_channels();

    // Create runtime and shared state
    let mut runtime = TuiRuntime::new();
    let throttle = Arc::new(FetcherThrottle::default());
    let show_all = Arc::new(AtomicBool::new(app.show_all_jobs));
    let animation_visible = Arc::new(AtomicBool::new(true));

    // Spawn background tasks
    runtime.track(spawn_input_task(input_tx, runtime.cancel_token()));

    runtime.track(spawn_job_fetcher(
        data_tx.clone(),
        runtime.cancel_token(),
        throttle.clone(),
        app.username.clone(),
        show_all.clone(),
    ));

    runtime.track(spawn_node_fetcher(
        data_tx.clone(),
        runtime.cancel_token(),
        throttle.clone(),
    ));

    runtime.track(spawn_fairshare_fetcher(
        data_tx.clone(),
        runtime.cancel_token(),
        throttle.clone(),
        app.username.clone(),
    ));

    runtime.track(spawn_scheduler_stats_fetcher(
        data_tx.clone(),
        runtime.cancel_token(),
        throttle.clone(),
    ));

    runtime.track(spawn_animation_tick(
        data_tx.clone(),
        runtime.cancel_token(),
        animation_visible.clone(),
    ));

    runtime.track(spawn_throttle_decay(
        runtime.cancel_token(),
        throttle.clone(),
    ));

    // Run the main event loop
    let result = run_event_loop(app, input_rx, data_rx, throttle.clone(), |app| {
        terminal.draw(|frame| ui::render(app, frame))?;
        Ok(())
    })
    .await;

    // Shutdown background tasks
    runtime.shutdown().await;

    // Restore terminal
    restore_terminal(&mut terminal)?;

    result
}

/// Setup the terminal for TUI mode
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to normal mode
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI with the tokio runtime (entry point from main)
pub fn run() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_tui())
}
