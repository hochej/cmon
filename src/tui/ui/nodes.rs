//! Nodes view rendering
//!
//! Handles rendering of the Nodes view in both list and grid modes.

use std::collections::BTreeMap;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::formatting::{format_bytes_mb, truncate_string};
use crate::models::NodeInfo;
use crate::slurm::shorten_node_name;
use crate::tui::app::{App, NodesViewMode};
use crate::tui::theme::Theme;

use super::widgets::{calculate_scroll_offset, create_table_header};

pub fn render_nodes_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let view_mode_indicator = match app.nodes_view.view_mode {
        NodesViewMode::List => "List",
        NodesViewMode::Grid => "Grid",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .title(format!(
            " Nodes ({}) [v:{}] ",
            app.data.nodes.len(),
            view_mode_indicator
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.data.nodes.is_empty() {
        let msg = if app.data.nodes.last_updated.is_none() {
            "Loading nodes..."
        } else {
            "No nodes found"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Dispatch to appropriate view mode renderer
    match app.nodes_view.view_mode {
        NodesViewMode::List => render_nodes_list(app, frame, inner, theme),
        NodesViewMode::Grid => render_nodes_grid(app, frame, inner, theme),
    }
}

fn render_nodes_list(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    // Split into main table and footer detail
    let chunks = Layout::vertical([
        Constraint::Min(5),    // Node table
        Constraint::Length(3), // Node detail footer
    ])
    .split(area);

    // Node table
    let header = create_table_header(
        &["Name", "Partition", "State", "CPUs", "Memory", "GPUs"],
        theme,
    );

    let available_height = chunks[0].height.saturating_sub(1) as usize;
    let selected = app.nodes_view.list_state.selected;
    let scroll_offset = calculate_scroll_offset(selected, available_height, app.data.nodes.len());

    let node_prefix = &app.config.display.node_prefix_strip;
    let rows: Vec<Row> = app
        .data
        .nodes
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(i, node)| {
            let is_selected = i == selected;
            node_to_row(node, is_selected, theme, node_prefix)
        })
        .collect();

    let widths = [
        Constraint::Length(15), // Name
        Constraint::Length(10), // Partition
        Constraint::Length(10), // State
        Constraint::Length(12), // CPUs
        Constraint::Length(12), // Memory
        Constraint::Length(12), // GPUs
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(theme.selected_bg));

    frame.render_widget(table, chunks[0]);

    // Node detail footer
    render_node_detail_footer(app, frame, chunks[1], theme);
}

/// Render nodes as a visual grid - each node is a colored cell
/// Color represents node state: green=idle, blue=allocated, yellow=mixed, red=down
fn render_nodes_grid(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    // Split into grid area and detail footer
    let chunks = Layout::vertical([
        Constraint::Min(5),    // Grid view
        Constraint::Length(4), // Node detail footer + legend
    ])
    .split(area);

    // Group nodes by partition for organized display (normalized to lowercase)
    let mut nodes_by_partition: BTreeMap<String, Vec<&NodeInfo>> = BTreeMap::new();
    for node in app.data.nodes.iter() {
        nodes_by_partition
            .entry(node.partition_name())
            .or_default()
            .push(node);
    }

    // Calculate grid dimensions
    // Each node cell is 3 chars wide (to fit short status)
    let cell_width = 4u16;
    let available_width = chunks[0].width.saturating_sub(2); // Account for borders
    let cells_per_row = (available_width / cell_width).max(1) as usize;

    let mut lines: Vec<Line> = Vec::new();

    // Use configured partition display order (or alphabetical if empty)
    let partition_order = &app.config.display.partition_order;
    let mut ordered_partitions: Vec<String> = Vec::new();

    // Add configured partitions in order (if they exist)
    for name in partition_order {
        let name_lower = name.to_lowercase();
        if nodes_by_partition.contains_key(&name_lower) {
            ordered_partitions.push(name_lower);
        }
    }
    // Add remaining partitions alphabetically
    for name in nodes_by_partition.keys() {
        if !ordered_partitions.contains(name) {
            ordered_partitions.push(name.clone());
        }
    }

    for partition_name in ordered_partitions {
        if let Some(nodes) = nodes_by_partition.get(&partition_name) {
            // Partition header
            let idle_count = nodes.iter().filter(|n| n.is_idle()).count();
            let alloc_count = nodes
                .iter()
                .filter(|n| n.is_allocated() || n.is_mixed())
                .count();
            let down_count = nodes.iter().filter(|n| n.is_down() || n.is_fail()).count();

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", partition_name),
                    Style::default().fg(theme.account_highlight).bold(),
                ),
                Span::styled(
                    format!("({} nodes: ", nodes.len()),
                    Style::default().fg(theme.border),
                ),
                Span::styled(
                    format!("{} idle", idle_count),
                    Style::default().fg(theme.running),
                ),
                Span::raw(", "),
                Span::styled(
                    format!("{} alloc", alloc_count),
                    Style::default().fg(theme.completed),
                ),
                if down_count > 0 {
                    Span::styled(
                        format!(", {} down", down_count),
                        Style::default().fg(theme.failed),
                    )
                } else {
                    Span::raw("")
                },
                Span::styled(")", Style::default().fg(theme.border)),
            ]));

            // Build grid rows for this partition
            for chunk in nodes.chunks(cells_per_row) {
                let mut spans: Vec<Span> = vec![Span::raw(" ")];

                for node in chunk {
                    let state = node.primary_state();
                    let (cell_char, color) = match state {
                        "IDLE" => ("[I]", theme.running),
                        "ALLOCATED" => ("[A]", theme.completed),
                        "MIXED" => ("[M]", theme.pending),
                        "DOWN" | "FAIL" | "FAILING" => ("[X]", theme.failed),
                        "DRAINING" | "DRAINED" => ("[D]", theme.timeout),
                        "RESERVED" => ("[R]", theme.account_highlight),
                        _ => ("[?]", theme.border),
                    };

                    spans.push(Span::styled(cell_char, Style::default().fg(color)));
                    spans.push(Span::raw(" ")); // Spacing between cells
                }

                lines.push(Line::from(spans));
            }

            lines.push(Line::from("")); // Spacing between partitions
        }
    }

    let para = Paragraph::new(lines).style(Style::default().fg(theme.fg));
    frame.render_widget(para, chunks[0]);

    // Footer with legend and selected node info
    render_grid_footer(app, frame, chunks[1], theme);
}

fn render_grid_footer(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Legend
        Constraint::Length(3), // Selected node detail
    ])
    .split(area);

    // Legend line
    let legend = Line::from(vec![
        Span::raw(" Legend: "),
        Span::styled("[I]", Style::default().fg(theme.running)),
        Span::raw("=Idle "),
        Span::styled("[A]", Style::default().fg(theme.completed)),
        Span::raw("=Allocated "),
        Span::styled("[M]", Style::default().fg(theme.pending)),
        Span::raw("=Mixed "),
        Span::styled("[D]", Style::default().fg(theme.timeout)),
        Span::raw("=Draining "),
        Span::styled("[X]", Style::default().fg(theme.failed)),
        Span::raw("=Down "),
        Span::raw(" | "),
        Span::styled("v", Style::default().fg(theme.account_highlight)),
        Span::raw(":toggle view"),
    ]);
    let legend_para = Paragraph::new(legend).style(Style::default().fg(theme.border));
    frame.render_widget(legend_para, chunks[0]);

    // Selected node detail (reuse existing footer)
    render_node_detail_footer(app, frame, chunks[1], theme);
}

fn render_node_detail_footer(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.border))
        .title(" Selected Node ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(node) = app.selected_node() {
        let gpu_info = node.gpu_info();
        let features = &node.features.total;

        let detail_line = Line::from(vec![
            Span::styled(" Node: ", Style::default().bold()),
            Span::raw(node.name()),
            Span::raw(" | "),
            Span::styled("State: ", Style::default().bold()),
            Span::styled(
                node.primary_state(),
                Style::default().fg(state_color_for_node(node, theme)),
            ),
            Span::raw(" | "),
            Span::styled("CPUs: ", Style::default().bold()),
            Span::raw(format!("{}/{}", node.cpus.allocated, node.cpus.total)),
            Span::raw(" | "),
            Span::styled("Mem: ", Style::default().bold()),
            Span::raw(format!(
                "{}/{}",
                format_bytes_mb(node.memory.allocated),
                format_bytes_mb(node.memory.minimum)
            )),
            if gpu_info.total > 0 {
                Span::raw(format!(
                    " | GPUs: {}/{} {}",
                    gpu_info.used,
                    gpu_info.total,
                    gpu_info.gpu_type.to_uppercase()
                ))
            } else {
                Span::raw("")
            },
            if !features.is_empty() {
                Span::raw(format!(" | Features: {}", truncate_string(features, 20)))
            } else {
                Span::raw("")
            },
        ]);

        let para = Paragraph::new(detail_line).style(Style::default().fg(theme.fg));
        frame.render_widget(para, inner);
    } else {
        let para = Paragraph::new(" Select a node to see details")
            .style(Style::default().fg(theme.border));
        frame.render_widget(para, inner);
    }
}

fn state_color_for_node(node: &NodeInfo, theme: &Theme) -> Color {
    theme.node_state_color(node.primary_state())
}

fn node_to_row<'a>(
    node: &'a NodeInfo,
    is_selected: bool,
    theme: &Theme,
    node_prefix_strip: &str,
) -> Row<'a> {
    let state = node.primary_state();
    let state_color = theme.node_state_color(state);

    let partition = node.partition.name.as_deref().unwrap_or("");
    let cpu_info = format!("{}/{}", node.cpus.allocated, node.cpus.total);
    let mem_info = format!(
        "{}/{}",
        format_bytes_mb(node.memory.allocated),
        format_bytes_mb(node.memory.minimum)
    );

    let gpu = node.gpu_info();
    let gpu_info_str = if gpu.total > 0 {
        if gpu.gpu_type.is_empty() {
            format!("{}/{}", gpu.used, gpu.total)
        } else {
            format!("{}/{} {}", gpu.used, gpu.total, gpu.gpu_type)
        }
    } else {
        "-".to_string()
    };

    let cells = vec![
        Cell::from(shorten_node_name(node.name(), node_prefix_strip).to_string()),
        Cell::from(truncate_string(partition, 10)),
        Cell::from(state).style(Style::default().fg(state_color)),
        Cell::from(cpu_info),
        Cell::from(mem_info),
        Cell::from(gpu_info_str),
    ];

    let row = Row::new(cells);
    if is_selected {
        row.style(Style::default().bg(theme.selected_bg))
    } else {
        row
    }
}
