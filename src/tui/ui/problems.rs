//! Problems view rendering
//!
//! Handles rendering of the Problems view showing down and draining nodes.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::formatting::truncate_string;
use crate::models::{NodeInfo, ReasonInfo};
use crate::slurm::shorten_node_name;
use crate::tui::app::{App, ProblemsPanel};
use crate::tui::theme::Theme;

use super::widgets::calculate_scroll_offset;

pub fn render_problems_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let down_nodes = app.down_nodes();
    let draining_nodes = app.draining_nodes();
    let total_problems = down_nodes.len() + draining_nodes.len();

    let title = format!(" Problems ({} nodes) ", total_problems);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if total_problems > 0 {
            theme.failed
        } else {
            theme.border_focused
        }))
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.data.nodes.is_empty() {
        let msg = if app.data.nodes.last_updated.is_none() {
            "Loading node data..."
        } else {
            "No node data available"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    if total_problems == 0 {
        let para = Paragraph::new("No problematic nodes found - all systems operational")
            .style(Style::default().fg(theme.running))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Split into down and draining sections
    let chunks =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(inner);

    // Down nodes section
    let down_focused = app.problems_view.selected_panel == ProblemsPanel::Down;
    render_down_nodes_section(app, &down_nodes, frame, chunks[0], theme, down_focused);

    // Draining nodes section
    let draining_focused = app.problems_view.selected_panel == ProblemsPanel::Draining;
    render_draining_nodes_section(
        app,
        &draining_nodes,
        frame,
        chunks[1],
        theme,
        draining_focused,
    );
}

fn render_down_nodes_section(
    app: &App,
    nodes: &[&NodeInfo],
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
) {
    // Highlight border if this panel is focused
    let border_color = if focused {
        theme.account_highlight
    } else {
        theme.failed
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(
            " Down Nodes ({}) {} ",
            nodes.len(),
            if focused { "[Tab to switch]" } else { "" }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if nodes.is_empty() {
        let para = Paragraph::new("No down nodes")
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    let header_cells = ["Name", "Partition", "State", "Reason"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1);

    let selected_idx = app.problems_view.down_nodes_state.selected;
    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset = calculate_scroll_offset(selected_idx, available_height, nodes.len());

    let node_prefix = &app.config.display.node_prefix_strip;
    let rows: Vec<Row> = nodes
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, node)| {
            let reason = match &node.reason {
                ReasonInfo::Empty => String::new(),
                ReasonInfo::String(s) => truncate_string(s, 30),
                ReasonInfo::Object { description } => truncate_string(description, 30),
            };

            let row = Row::new(vec![
                Cell::from(shorten_node_name(node.name(), node_prefix).to_string()),
                Cell::from(node.partition.name.as_deref().unwrap_or("")),
                Cell::from(node.primary_state()).style(Style::default().fg(theme.failed)),
                Cell::from(reason),
            ]);

            // Highlight selected row if this panel is focused
            if focused && idx == selected_idx {
                row.style(Style::default().bg(theme.selected_bg))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(15),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Min(15),
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, inner);
}

fn render_draining_nodes_section(
    app: &App,
    nodes: &[&NodeInfo],
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
) {
    // Highlight border if this panel is focused
    let border_color = if focused {
        theme.account_highlight
    } else {
        theme.timeout
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(format!(
            " Draining Nodes ({}) {} ",
            nodes.len(),
            if focused { "[Tab to switch]" } else { "" }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if nodes.is_empty() {
        let para = Paragraph::new("No draining nodes")
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    let header_cells = ["Name", "Partition", "State", "Reason"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1);

    let selected_idx = app.problems_view.draining_nodes_state.selected;
    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset = calculate_scroll_offset(selected_idx, available_height, nodes.len());

    let node_prefix = &app.config.display.node_prefix_strip;
    let rows: Vec<Row> = nodes
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, node)| {
            let reason = match &node.reason {
                ReasonInfo::Empty => String::new(),
                ReasonInfo::String(s) => truncate_string(s, 30),
                ReasonInfo::Object { description } => truncate_string(description, 30),
            };

            let row = Row::new(vec![
                Cell::from(shorten_node_name(node.name(), node_prefix).to_string()),
                Cell::from(node.partition.name.as_deref().unwrap_or("")),
                Cell::from(node.primary_state()).style(Style::default().fg(theme.timeout)),
                Cell::from(reason),
            ]);

            // Highlight selected row if this panel is focused
            if focused && idx == selected_idx {
                row.style(Style::default().bg(theme.selected_bg))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(15),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Min(15),
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, inner);
}
