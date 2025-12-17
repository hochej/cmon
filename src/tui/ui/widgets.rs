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

/// Create a section header line for detail popups
///
/// Returns a Line with indentation and styled header text (highlighted, bold, underlined).
/// Used in job detail popups and similar views.
pub fn section_header(title: &str, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            title.to_string(),
            Style::default()
                .fg(theme.account_highlight)
                .bold()
                .underlined(),
        ),
    ])
}

/// Create a simple key-value detail row
///
/// Returns a Line with a bold label and plain value. For more complex rows with
/// multiple styled values, construct the Line directly.
///
/// # Arguments
/// * `label` - The label text (will be bolded), should include trailing padding
/// * `value` - The value to display
pub fn detail_row(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {}", label), Style::default().bold()),
        Span::raw(value.to_string()),
    ])
}

