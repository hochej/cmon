//! Reusable UI widgets and helper functions
//!
//! This module contains shared rendering utilities used across different views.

use ratatui::prelude::*;
use ratatui::widgets::{Cell, Row};

use crate::tui::theme::Theme;

/// Create a styled table header row from column names
pub fn create_table_header<'a>(columns: &[&'a str], theme: &Theme) -> Row<'a> {
    let header_cells = columns
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1)
}

/// Calculate scroll offset to keep selection visible
pub fn calculate_scroll_offset(selected: usize, visible_height: usize, total: usize) -> usize {
    if visible_height == 0 || total == 0 {
        return 0;
    }

    if selected < visible_height / 2 {
        0
    } else if selected > total.saturating_sub(visible_height / 2) {
        total.saturating_sub(visible_height)
    } else {
        selected.saturating_sub(visible_height / 2)
    }
}

/// Create a centered rectangle
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Create a progress bar as a Span
pub fn create_progress_bar(percent: f64, width: usize, theme: &Theme) -> Span<'static> {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);

    let color = theme.progress_color(percent);

    let bar = format!("[{}{}]", "=".repeat(filled), ".".repeat(empty));

    Span::styled(bar, Style::default().fg(color))
}
