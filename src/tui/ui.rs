//! UI rendering for the TUI
//!
//! This module handles all rendering using ratatui. The rendering is event-driven -
//! we only render when an event triggers a state change, not at a fixed frame rate.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs};

use crate::slurm::shorten_node_name;
use crate::models::JobState;
use crate::tui::app::{
    App, ModalState, NodesViewMode, PartitionStatus, PersonalPanel, ProblemsPanel, TuiJobInfo,
    View,
};
use crate::tui::theme::Theme;

// ============================================================================
// Table Rendering Helpers
// ============================================================================

/// Create a styled table header row from column names
fn create_table_header<'a>(columns: &[&'a str], theme: &Theme) -> Row<'a> {
    let header_cells = columns
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1)
}

/// Render the entire TUI
pub fn render(app: &App, frame: &mut Frame) {
    // Use theme from configuration
    let theme = Theme::from_name(&app.config.display.theme);
    let area = frame.area();

    // Main layout: header, content, footer
    let layout = Layout::vertical([
        Constraint::Length(1), // Tab bar
        Constraint::Length(1), // Info bar
        Constraint::Min(0),    // Main content
        Constraint::Length(2), // Status bar
    ])
    .split(area);

    render_tab_bar(app, frame, layout[0], &theme);
    render_info_bar(app, frame, layout[1], &theme);
    render_content(app, frame, layout[2], &theme);
    render_status_bar(app, frame, layout[3], &theme);

    // Overlays (render in order of z-index)
    match &app.modal {
        ModalState::Help => render_help_overlay(frame, area, &theme),
        ModalState::Filter { .. } => render_filter_overlay(app, frame, area, &theme),
        ModalState::Detail => render_job_detail_popup(app, frame, area, &theme),
        ModalState::Confirm { .. } => render_confirm_dialog(app, frame, area, &theme),
        ModalState::Sort { .. } => render_sort_menu(app, frame, area, &theme),
        ModalState::None => {}
    }

    // Clipboard feedback toast (always on top)
    if let Some(feedback) = app.current_clipboard_feedback() {
        render_clipboard_toast(feedback, frame, area, &theme);
    }
}

fn render_tab_bar(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let titles: Vec<Line> = [
        View::Jobs,
        View::Nodes,
        View::Partitions,
        View::Personal,
        View::Problems,
    ]
    .iter()
    .enumerate()
    .map(|(i, view)| {
        let num = format!("[{}]", i + 1);
        let label = view.label();
        if *view == app.current_view {
            Line::from(vec![
                Span::styled(num, Style::default().fg(theme.account_highlight)),
                Span::styled(label, Style::default().fg(theme.selected_fg).bold()),
            ])
        } else {
            Line::from(vec![
                Span::styled(num, Style::default().fg(theme.border)),
                Span::raw(label),
            ])
        }
    })
    .collect();

    let tabs = Tabs::new(titles)
        .select(app.current_view as usize)
        .divider(" | ")
        .style(Style::default().fg(theme.fg))
        .highlight_style(Style::default().fg(theme.selected_fg).bold());

    frame.render_widget(tabs, area);
}

fn render_info_bar(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let job_mode = if app.show_all_jobs {
        "All Jobs"
    } else {
        "My Jobs"
    };

    let account_display = app.account_context.display();
    let job_count = app.data.jobs.len();

    let filter_info = if let Some(f) = app.data.get_filter() {
        format!(" | Filter: {}", f)
    } else {
        String::new()
    };

    // Scheduler stats indicator
    let scheduler_info = if let Some(stats) = &app.data.scheduler_stats {
        if stats.is_available() {
            let health = match stats.is_healthy() {
                Some(true) => "OK",
                Some(false) => "SLOW",
                None => "-",
            };
            format!(" | Sched: {} ({})", health, stats.mean_cycle_display())
        } else {
            String::new() // sdiag not available, don't show
        }
    } else {
        String::new()
    };

    let info = format!(
        " {} | Account: {} | {} jobs{}{}",
        job_mode, account_display, job_count, filter_info, scheduler_info
    );

    let stale = app.data.jobs.is_stale();
    let style = if stale {
        Style::default().fg(theme.stale_indicator)
    } else {
        Style::default().fg(theme.border)
    };

    let para = Paragraph::new(info).style(style);
    frame.render_widget(para, area);
}

fn render_content(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    match app.current_view {
        View::Jobs => render_jobs_view(app, frame, area, theme),
        View::Nodes => render_nodes_view(app, frame, area, theme),
        View::Partitions => render_partitions_view(app, frame, area, theme),
        View::Personal => render_personal_view(app, frame, area, theme),
        View::Problems => render_problems_view(app, frame, area, theme),
    }
}

fn render_jobs_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let title = if app.jobs_view.show_grouped_by_account {
        " Jobs (Grouped by Account) "
    } else {
        " Jobs "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.data.jobs.is_empty() {
        let msg = if app.data.jobs.last_updated.is_none() {
            "Loading jobs..."
        } else {
            "No jobs found"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    if app.jobs_view.show_grouped_by_account {
        render_jobs_grouped_by_account(app, frame, inner, theme);
    } else {
        render_jobs_flat_list(app, frame, inner, theme);
    }
}

fn render_jobs_flat_list(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    // Filter jobs based on array collapse state
    let visible_jobs: Vec<(usize, &TuiJobInfo)> = app
        .data.jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| app.is_job_visible(job))
        .collect();

    // Table header
    let header = create_table_header(&["ID", "Name", "Account", "Part", "State", "Time", "GPUs"], theme);

    // Calculate visible rows
    let available_height = area.height.saturating_sub(1) as usize; // -1 for header
    let selected = app.jobs_view.list_state.selected;
    let scroll_offset = calculate_scroll_offset(selected, available_height, visible_jobs.len());

    // Build rows
    let rows: Vec<Row> = visible_jobs
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(display_idx, (_, job))| {
            let is_selected = display_idx == selected;
            let is_collapsed_array =
                job.is_array_job() && app.jobs_view.is_array_collapsed(job.job_id.base_id.get());
            job_to_row(job, is_selected, is_collapsed_array, app, theme)
        })
        .collect();

    // Column widths
    let widths = [
        Constraint::Length(12), // ID (wider for array notation)
        Constraint::Min(15),    // Name
        Constraint::Length(10), // Account
        Constraint::Length(8),  // Partition
        Constraint::Length(12), // State (wider for array summary)
        Constraint::Length(11), // Time
        Constraint::Length(5),  // GPUs
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(theme.selected_bg));

    frame.render_widget(table, area);
}

fn render_jobs_grouped_by_account(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    use std::collections::BTreeMap;

    // Group jobs by account
    let visible_jobs: Vec<&TuiJobInfo> = app
        .data.jobs
        .iter()
        .filter(|job| app.is_job_visible(job))
        .collect();

    let mut by_account: BTreeMap<&str, Vec<&TuiJobInfo>> = BTreeMap::new();
    for job in &visible_jobs {
        by_account.entry(&job.account).or_default().push(job);
    }

    // Build rows with account headers
    let mut rows: Vec<Row> = Vec::new();
    for (account, jobs) in by_account.iter() {
        let running = jobs.iter().filter(|j| j.state == JobState::Running).count();
        let pending = jobs.iter().filter(|j| j.state == JobState::Pending).count();
        let total_gpus: u32 = jobs.iter().map(|j| j.gpu_count).sum();

        // Account header row
        let account_summary = format!(
            "{} ({} jobs: {} R, {} P, {} GPUs)",
            account,
            jobs.len(),
            running,
            pending,
            total_gpus
        );
        let header_row = Row::new(vec![
            Cell::from(account_summary).style(
                Style::default()
                    .fg(theme.account_highlight)
                    .bold()
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]);
        rows.push(header_row);

        // Job rows under this account (indented)
        for job in jobs {
            let state_color = theme.job_state_color(job.state);
            rows.push(Row::new(vec![
                Cell::from(format!("  {}", job.job_id)),
                Cell::from(truncate_string(&job.name, 15)),
                Cell::from(job.partition.clone()),
                Cell::from(job.state.as_str()).style(Style::default().fg(state_color)),
                Cell::from(job.elapsed_display()),
                Cell::from(if job.gpu_count > 0 {
                    format!("{}", job.gpu_count)
                } else {
                    "-".to_string()
                }),
            ]));
        }
    }

    // Table header
    let header = create_table_header(&["ID/Account", "Name", "Part", "State", "Time", "GPUs"], theme);

    // Column widths (slightly different for grouped view)
    let widths = [
        Constraint::Length(14), // ID/Account header
        Constraint::Min(15),    // Name
        Constraint::Length(8),  // Partition
        Constraint::Length(10), // State
        Constraint::Length(11), // Time
        Constraint::Length(5),  // GPUs
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(theme.selected_bg));

    frame.render_widget(table, area);
}

fn job_to_row<'a>(
    job: &'a TuiJobInfo,
    is_selected: bool,
    is_collapsed_array: bool,
    app: &App,
    theme: &Theme,
) -> Row<'a> {
    let state_color = theme.job_state_color(job.state);

    // For collapsed array jobs, show aggregated info
    let (id_str, state_str, state_style) = if is_collapsed_array {
        let (running, pending, completed, _) = app.array_job_summary(job.job_id.base_id.get());
        let total = running + pending + completed;

        // Show with collapse indicator
        let id = format!("v {}[{}]", job.job_id.base_id.get(), total);

        // Show aggregated state
        let state = if running > 0 && pending > 0 {
            format!("R:{} P:{}", running, pending)
        } else if running > 0 {
            format!("RUN:{}", running)
        } else if pending > 0 {
            format!("PEND:{}", pending)
        } else {
            format!("DONE:{}", completed)
        };

        let color = if running > 0 {
            theme.running
        } else if pending > 0 {
            theme.pending
        } else {
            theme.completed
        };

        (id, state, Style::default().fg(color))
    } else if job.is_array_job() {
        // Expanded array - show with expand indicator
        let id = format!("^ {}", job.job_id);
        (
            id,
            job.state.short_str().to_string(),
            Style::default().fg(state_color),
        )
    } else {
        (
            job.job_id.to_string(),
            job.state.short_str().to_string(),
            Style::default().fg(state_color),
        )
    };

    // For pending jobs, show estimated start instead of elapsed time
    let time_display = if job.state == JobState::Pending {
        let est = job.estimated_start_display();
        if est != "N/A" && est != "-" {
            est // Show estimated start
        } else {
            // Show wait time since submission
            job.elapsed_display()
        }
    } else {
        job.elapsed_display()
    };

    let cells = vec![
        Cell::from(id_str),
        Cell::from(truncate_string(&job.name, 20)),
        Cell::from(job.account.clone()),
        Cell::from(job.partition.clone()),
        Cell::from(state_str).style(state_style),
        Cell::from(time_display),
        Cell::from(if job.gpu_count > 0 {
            job.gpu_count.to_string()
        } else {
            "-".to_string()
        }),
    ];

    let row = Row::new(cells);
    if is_selected {
        row.style(Style::default().bg(theme.selected_bg))
    } else {
        row
    }
}

fn render_nodes_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
    let header = create_table_header(&["Name", "Partition", "State", "CPUs", "Memory", "GPUs"], theme);

    let available_height = chunks[0].height.saturating_sub(1) as usize;
    let selected = app.nodes_view.list_state.selected;
    let scroll_offset = calculate_scroll_offset(selected, available_height, app.data.nodes.len());

    let node_prefix = &app.config.display.node_prefix_strip;
    let rows: Vec<Row> = app
        .data.nodes
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
    use std::collections::BTreeMap;

    // Split into grid area and detail footer
    let chunks = Layout::vertical([
        Constraint::Min(5),    // Grid view
        Constraint::Length(4), // Node detail footer + legend
    ])
    .split(area);

    // Group nodes by partition for organized display (normalized to lowercase)
    let mut nodes_by_partition: BTreeMap<String, Vec<&crate::models::NodeInfo>> = BTreeMap::new();
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
                format_memory(node.memory.allocated),
                format_memory(node.memory.minimum)
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

fn state_color_for_node(node: &crate::models::NodeInfo, theme: &Theme) -> Color {
    theme.node_state_color(node.primary_state())
}

fn node_to_row<'a>(
    node: &'a crate::models::NodeInfo,
    is_selected: bool,
    theme: &Theme,
    node_prefix_strip: &str,
) -> Row<'a> {
    let state = node.primary_state();
    let state_color = theme.node_state_color(state);

    let partition = node.partition.name.clone().unwrap_or_default();
    let cpu_info = format!("{}/{}", node.cpus.allocated, node.cpus.total);
    let mem_info = format!(
        "{}/{}",
        format_memory(node.memory.allocated),
        format_memory(node.memory.minimum)
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
        Cell::from(truncate_string(&partition, 10)),
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

fn render_partitions_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .title(" Partitions ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.data.nodes.is_empty() {
        let msg = if app.data.nodes.last_updated.is_none() {
            "Loading partition data..."
        } else {
            "No partition data available"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    let partition_stats = app.compute_partition_stats();

    if partition_stats.is_empty() {
        let para = Paragraph::new("No partitions found")
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Calculate partition card heights (vary based on GPU presence)
    let partition_heights: Vec<u16> = partition_stats
        .iter()
        .map(|p| if p.total_gpus > 0 { 9 } else { 8 }) // Extra line for GPUs
        .collect();

    let constraints: Vec<Constraint> = partition_heights
        .iter()
        .map(|h| Constraint::Length(*h))
        .collect();

    let partition_areas = Layout::vertical(constraints).split(inner);

    for (i, partition) in partition_stats.iter().enumerate() {
        if i < partition_areas.len() {
            render_partition_card(partition, frame, partition_areas[i], theme);
        }
    }
}

/// Render a single partition card with rich information
fn render_partition_card(
    partition: &PartitionStatus,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
) {
    // Build partition title with status indicators
    let status_indicator = if partition.down_nodes > 0 {
        format!(" {} down", partition.down_nodes)
    } else {
        String::new()
    };

    let title = format!(
        " {} ({} nodes{}) ",
        partition.name,
        partition.total_nodes,
        status_indicator
    );

    let border_color = if partition.down_nodes > 0 {
        theme.failed
    } else if partition.cpu_utilization() > 90.0 {
        theme.progress_warn
    } else {
        theme.border
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into left (metrics) and right (node states) sections
    let chunks =
        Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)]).split(inner);

    // Left side: Resource utilization bars
    let mut metric_lines: Vec<Line> = Vec::new();

    // CPU bar with inline stats
    let cpu_util = partition.cpu_utilization();
    let cpu_bar = create_progress_bar(cpu_util, 25, theme);
    metric_lines.push(Line::from(vec![
        Span::styled(" CPU  ", Style::default().fg(theme.header_fg).bold()),
        cpu_bar,
        Span::raw(format!(" {:>5.1}%", cpu_util)),
        Span::styled(
            format!("  {}/{}", partition.allocated_cpus, partition.total_cpus),
            Style::default().fg(theme.border),
        ),
    ]));

    // Memory bar
    let mem_util = partition.memory_utilization();
    let mem_bar = create_progress_bar(mem_util, 25, theme);
    metric_lines.push(Line::from(vec![
        Span::styled(" Mem  ", Style::default().fg(theme.header_fg).bold()),
        mem_bar,
        Span::raw(format!(" {:>5.1}%", mem_util)),
        Span::styled(
            format!(
                "  {:.0}/{:.0}T",
                partition.allocated_memory_gb / 1024.0,
                partition.total_memory_gb / 1024.0
            ),
            Style::default().fg(theme.border),
        ),
    ]));

    // GPU bar (if available)
    if partition.total_gpus > 0 {
        let gpu_util = partition.gpu_utilization();
        let gpu_bar = create_progress_bar(gpu_util, 25, theme);
        let gpu_label = partition.gpu_type.as_deref().unwrap_or("GPU");
        metric_lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<4} ", &gpu_label[..gpu_label.len().min(4)]),
                Style::default().fg(theme.header_fg).bold(),
            ),
            gpu_bar,
            Span::raw(format!(" {:>5.1}%", gpu_util)),
            Span::styled(
                format!("  {}/{}", partition.allocated_gpus, partition.total_gpus),
                Style::default().fg(theme.border),
            ),
        ]));
    }

    // Add job queue info
    metric_lines.push(Line::from(""));
    metric_lines.push(Line::from(vec![
        Span::styled(" Jobs ", Style::default().fg(theme.header_fg).bold()),
        Span::styled(
            format!("{} running", partition.running_jobs),
            Style::default().fg(theme.running),
        ),
        Span::raw("  "),
        if partition.pending_jobs > 0 {
            Span::styled(
                format!("{} pending", partition.pending_jobs),
                Style::default().fg(theme.pending),
            )
        } else {
            Span::styled("0 pending", Style::default().fg(theme.border))
        },
    ]));

    let metrics_para = Paragraph::new(metric_lines);
    frame.render_widget(metrics_para, chunks[0]);

    // Right side: Node state breakdown
    let mut state_lines: Vec<Line> = Vec::new();

    state_lines.push(Line::from(Span::styled(
        "Node States",
        Style::default().fg(theme.header_fg).bold(),
    )));

    // Node state mini-visualization
    let node_states = render_node_state_bar(partition, theme);
    state_lines.push(Line::from(node_states));

    // Detailed breakdown
    state_lines.push(Line::from(vec![
        Span::styled("[I]", Style::default().fg(theme.idle)),
        Span::raw(format!(" {:>3} idle  ", partition.idle_nodes)),
        Span::styled("[A]", Style::default().fg(theme.running)),
        Span::raw(format!(" {:>3} alloc", partition.allocated_nodes)),
    ]));

    state_lines.push(Line::from(vec![
        Span::styled("[M]", Style::default().fg(theme.mixed)),
        Span::raw(format!(" {:>3} mixed ", partition.mixed_nodes)),
        Span::styled("[D]", Style::default().fg(theme.draining)),
        Span::raw(format!(" {:>3} drain", partition.draining_nodes)),
    ]));

    if partition.down_nodes > 0 {
        state_lines.push(Line::from(vec![
            Span::styled("[X]", Style::default().fg(theme.failed)),
            Span::raw(format!(" {:>3} down", partition.down_nodes)),
        ]));
    }

    let states_para = Paragraph::new(state_lines);
    frame.render_widget(states_para, chunks[1]);
}

/// Create a visual node state bar showing distribution
fn render_node_state_bar<'a>(partition: &PartitionStatus, theme: &Theme) -> Vec<Span<'a>> {
    let total = partition.total_nodes as usize;
    if total == 0 {
        return vec![Span::raw("-")];
    }

    // Calculate proportions for a 20-char bar
    let bar_width: usize = 20;
    let scale = |count: u32| -> usize {
        ((count as f64 / total as f64) * bar_width as f64).round() as usize
    };

    let idle_chars = scale(partition.idle_nodes);
    let alloc_chars = scale(partition.allocated_nodes);
    let mixed_chars = scale(partition.mixed_nodes);
    let drain_chars = scale(partition.draining_nodes);
    let down_chars = scale(partition.down_nodes);

    // Ensure we fill the bar (account for rounding)
    let used = idle_chars + alloc_chars + mixed_chars + drain_chars + down_chars;
    let remainder = bar_width.saturating_sub(used);

    let mut spans = Vec::new();
    spans.push(Span::raw("["));

    if idle_chars > 0 {
        spans.push(Span::styled(
            "I".repeat(idle_chars),
            Style::default().fg(theme.idle),
        ));
    }
    if alloc_chars > 0 {
        spans.push(Span::styled(
            "A".repeat(alloc_chars),
            Style::default().fg(theme.running),
        ));
    }
    if mixed_chars > 0 {
        spans.push(Span::styled(
            "M".repeat(mixed_chars),
            Style::default().fg(theme.mixed),
        ));
    }
    if drain_chars > 0 {
        spans.push(Span::styled(
            "D".repeat(drain_chars),
            Style::default().fg(theme.draining),
        ));
    }
    if down_chars > 0 {
        spans.push(Span::styled(
            "X".repeat(down_chars),
            Style::default().fg(theme.failed),
        ));
    }
    // Fill remainder with dots for any rounding discrepancy
    if remainder > 0 {
        spans.push(Span::styled(
            ".".repeat(remainder),
            Style::default().fg(theme.border),
        ));
    }

    spans.push(Span::raw("]"));
    spans
}

fn render_personal_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .title(format!(" Personal Dashboard - {} ", app.username));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Determine if we should show fairshare based on available data
    let show_fairshare = !app.data.fairshare_tree.is_empty();

    // Split into sections based on available data
    let chunks = if show_fairshare {
        Layout::vertical([
            Constraint::Length(6),      // Summary section
            Constraint::Length(12),     // Fairshare tree
            Constraint::Percentage(50), // Running jobs
            Constraint::Percentage(50), // Pending jobs
        ])
        .split(inner)
    } else {
        Layout::vertical([
            Constraint::Length(6),      // Summary section
            Constraint::Percentage(50), // Running jobs
            Constraint::Percentage(50), // Pending jobs
        ])
        .split(inner)
    };

    // Summary section - highlight if selected
    let summary_focused = app.personal_view.selected_panel == PersonalPanel::Summary;
    render_personal_summary(app, frame, chunks[0], theme, summary_focused);

    if show_fairshare {
        // Fairshare tree section
        let fairshare_focused = app.personal_view.selected_panel == PersonalPanel::Fairshare;
        render_fairshare_tree(app, frame, chunks[1], theme, fairshare_focused);
        // Running jobs section
        let running_focused = app.personal_view.selected_panel == PersonalPanel::Running;
        render_personal_running_jobs(app, frame, chunks[2], theme, running_focused);
        // Pending jobs section
        let pending_focused = app.personal_view.selected_panel == PersonalPanel::Pending;
        render_personal_pending_jobs(app, frame, chunks[3], theme, pending_focused);
    } else {
        // Running jobs section
        let running_focused = app.personal_view.selected_panel == PersonalPanel::Running;
        render_personal_running_jobs(app, frame, chunks[1], theme, running_focused);
        // Pending jobs section
        let pending_focused = app.personal_view.selected_panel == PersonalPanel::Pending;
        render_personal_pending_jobs(app, frame, chunks[2], theme, pending_focused);
    }
}

fn render_personal_summary(app: &App, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
    let running_jobs = app.my_running_jobs();
    let pending_jobs = app.my_pending_jobs();

    let total_cpus: u32 = running_jobs.iter().map(|j| j.cpus).sum();
    let total_gpus: u32 = running_jobs.iter().map(|j| j.gpu_count).sum();

    let summary_lines = vec![
        Line::from(vec![
            Span::styled("  Running Jobs: ", Style::default().bold()),
            Span::styled(
                format!("{}", running_jobs.len()),
                Style::default().fg(theme.running),
            ),
            Span::raw("    "),
            Span::styled("Pending Jobs: ", Style::default().bold()),
            Span::styled(
                format!("{}", pending_jobs.len()),
                Style::default().fg(theme.pending),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Total CPUs:   ", Style::default().bold()),
            Span::raw(format!("{}", total_cpus)),
            Span::raw("    "),
            Span::styled("Total GPUs:   ", Style::default().bold()),
            Span::raw(format!("{}", total_gpus)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Account:      ", Style::default().bold()),
            Span::styled(
                app.account_context.display(),
                Style::default().fg(theme.account_highlight),
            ),
            Span::raw(format!(
                " ({} accounts)",
                app.account_context.user_accounts.len()
            )),
        ]),
    ];

    // Highlight border if this panel is focused
    let border_style = if focused {
        Style::default().fg(theme.account_highlight)
    } else {
        Style::default().fg(theme.border)
    };

    let summary_block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(border_style)
        .title(" Summary ");

    let para = Paragraph::new(summary_lines)
        .block(summary_block)
        .style(Style::default().fg(theme.fg));
    frame.render_widget(para, area);
}

fn render_personal_running_jobs(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
) {
    let running_jobs = app.my_running_jobs();

    // Highlight border if this panel is focused
    let border_style = if focused {
        Style::default().fg(theme.account_highlight)
    } else {
        Style::default().fg(theme.border)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(format!(
            " Running Jobs ({}) {} ",
            running_jobs.len(),
            if focused { "[Tab to switch]" } else { "" }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if running_jobs.is_empty() {
        let msg = if app.data.jobs.last_updated.is_none() {
            "Loading jobs..."
        } else {
            "No running jobs"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Table header
    let header = create_table_header(&["ID", "Name", "Account", "Part", "Elapsed", "Remaining"], theme);

    let selected_idx = app.personal_view.running_jobs_state.selected;
    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset = calculate_scroll_offset(selected_idx, available_height, running_jobs.len());

    let rows: Vec<Row> = running_jobs
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, job)| {
            // Calculate time remaining with color coding
            let (remaining_str, remaining_color) = if let Some(remaining) = job.time_remaining() {
                let secs = remaining.as_secs();
                let color = if secs < 3600 {
                    theme.progress_crit // Less than 1 hour - critical (red)
                } else if secs < 6 * 3600 {
                    theme.progress_warn // Less than 6 hours - warning (orange)
                } else {
                    theme.progress_full // Plenty of time (green)
                };
                (format_duration_display(secs), color)
            } else {
                ("N/A".to_string(), theme.border)
            };

            let row = Row::new(vec![
                Cell::from(job.job_id.to_string()),
                Cell::from(truncate_string(&job.name, 18)),
                Cell::from(job.account.clone()),
                Cell::from(job.partition.clone()),
                Cell::from(job.elapsed_display()),
                Cell::from(remaining_str).style(Style::default().fg(remaining_color)),
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
        Constraint::Length(10), // ID
        Constraint::Min(12),    // Name
        Constraint::Length(10), // Account
        Constraint::Length(8),  // Partition
        Constraint::Length(11), // Elapsed
        Constraint::Length(12), // Remaining
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, inner);
}

fn render_personal_pending_jobs(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
) {
    let pending_jobs = app.my_pending_jobs();

    // Highlight border if this panel is focused
    let border_style = if focused {
        Style::default().fg(theme.account_highlight)
    } else {
        Style::default().fg(theme.border)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(format!(
            " Pending Jobs ({}) {} ",
            pending_jobs.len(),
            if focused { "[Tab to switch]" } else { "" }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if pending_jobs.is_empty() {
        let msg = if app.data.jobs.last_updated.is_none() {
            "Loading jobs..."
        } else {
            "No pending jobs"
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Table header
    let header = create_table_header(&["ID", "Name", "Account", "Part", "Reason", "Est.Start"], theme);

    let selected_idx = app.personal_view.pending_jobs_state.selected;
    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset = calculate_scroll_offset(selected_idx, available_height, pending_jobs.len());

    let rows: Vec<Row> = pending_jobs
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, job)| {
            let row = Row::new(vec![
                Cell::from(job.job_id.to_string()),
                Cell::from(truncate_string(&job.name, 18)),
                Cell::from(job.account.clone()),
                Cell::from(job.partition.clone()),
                Cell::from(truncate_string(&job.state_reason, 12)),
                Cell::from(job.estimated_start_display()),
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
        Constraint::Length(10), // ID
        Constraint::Min(12),    // Name
        Constraint::Length(10), // Account
        Constraint::Length(8),  // Partition
        Constraint::Length(12), // Reason
        Constraint::Length(10), // Est.Start
    ];

    let table = Table::new(rows, widths).header(header);
    frame.render_widget(table, inner);
}

fn render_fairshare_tree(app: &App, frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
    // Highlight border if this panel is focused
    let border_style = if focused {
        Style::default().fg(theme.account_highlight)
    } else {
        Style::default().fg(theme.border)
    };

    let title = if focused {
        " Fairshare Tree [Tab to switch] "
    } else {
        " Fairshare Tree "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.data.fairshare_tree.is_empty() {
        let para = Paragraph::new("Loading fairshare data...")
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Create table header
    let header_cells = [
        "Account/User",
        "Share%",
        "Fairshare",
        "CPU Hours",
        "GPU Hours",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1);

    let selected_idx = app.personal_view.fairshare_state.selected;
    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset =
        calculate_scroll_offset(selected_idx, available_height, app.data.fairshare_tree.len());

    // Create rows from flattened fairshare tree
    let rows: Vec<Row> = app
        .data.fairshare_tree
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, row)| {
            // Build indented name with tree prefix
            let name = row.display_name();

            // Choose color based on entity type
            let name_style = if row.is_current_user {
                Style::default().fg(theme.running).bold()
            } else if row.is_user {
                Style::default().fg(theme.fg)
            } else {
                Style::default().fg(theme.account_highlight)
            };

            // Fairshare factor color (green=good, red=bad)
            let fairshare_style = Style::default().fg(theme.fairshare_color(row.fairshare_factor));

            let row = Row::new(vec![
                Cell::from(name).style(name_style),
                Cell::from(format!("{:.1}%", row.shares_percent)),
                Cell::from(format!("{:.3}", row.fairshare_factor)).style(fairshare_style),
                Cell::from(format_hours(row.cpu_hours)),
                Cell::from(format_hours(row.gpu_hours)),
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
        Constraint::Min(20),    // Account/User (with indentation)
        Constraint::Length(8),  // Share%
        Constraint::Length(10), // Fairshare
        Constraint::Length(12), // CPU Hours
        Constraint::Length(12), // GPU Hours
    ];

    let table = Table::new(rows, widths).header(header);

    frame.render_widget(table, inner);
}

/// Format hours for display (e.g., "1.2K" for 1234 hours)
fn format_hours(hours: f64) -> String {
    if hours < 0.01 {
        "-".to_string()
    } else if hours < 1.0 {
        format!("{:.1}m", hours * 60.0) // Show as minutes if < 1 hour
    } else if hours < 1000.0 {
        format!("{:.1}h", hours)
    } else if hours < 10000.0 {
        format!("{:.1}Kh", hours / 1000.0)
    } else {
        format!("{:.0}Kh", hours / 1000.0)
    }
}

fn render_problems_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
    nodes: &[&crate::models::NodeInfo],
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
                crate::models::ReasonInfo::Empty => String::new(),
                crate::models::ReasonInfo::String(s) => truncate_string(s, 30),
                crate::models::ReasonInfo::Object { description } => {
                    truncate_string(description, 30)
                }
            };

            let row = Row::new(vec![
                Cell::from(shorten_node_name(node.name(), node_prefix).to_string()),
                Cell::from(node.partition.name.clone().unwrap_or_default()),
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
    nodes: &[&crate::models::NodeInfo],
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
                crate::models::ReasonInfo::Empty => String::new(),
                crate::models::ReasonInfo::String(s) => truncate_string(s, 30),
                crate::models::ReasonInfo::Object { description } => {
                    truncate_string(description, 30)
                }
            };

            let row = Row::new(vec![
                Cell::from(shorten_node_name(node.name(), node_prefix).to_string()),
                Cell::from(node.partition.name.clone().unwrap_or_default()),
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

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let layout = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    // Keybindings line - context-sensitive
    let keybinds = match app.current_view {
        View::Jobs => {
            " j/k:move  Enter:detail  c:cancel  y:yank  s:sort  /:search  a:toggle  ?:help  q:quit "
        }
        View::Nodes => " j/k:move  v:view-mode  ?:help  q:quit ",
        View::Personal => " j/k:move  Tab:switch panel  ?:help  q:quit ",
        View::Problems => " j/k:move  Tab:switch panel  ?:help  q:quit ",
        View::Partitions => " j/k:move  ?:help  q:quit ",
    };
    let keybinds_para = Paragraph::new(keybinds).style(Style::default().fg(theme.border));
    frame.render_widget(keybinds_para, layout[0]);

    // Status line with more info
    let mut status_parts = Vec::new();

    // Show current mode if in modal
    if app.modal.is_blocking() {
        let mode_name = match &app.modal {
            ModalState::Detail => "DETAIL",
            ModalState::Confirm { .. } => "CONFIRM",
            ModalState::Sort { .. } => "SORT",
            _ => "",
        };
        if !mode_name.is_empty() {
            status_parts.push(Span::styled(
                format!(" [{}]", mode_name),
                Style::default().fg(theme.progress_warn).bold(),
            ));
        }
    }

    // Show sort info if not default
    if app.current_view == View::Jobs
        && (app.jobs_view.sort_column != crate::tui::app::JobSortColumn::JobId
            || !app.jobs_view.sort_ascending)
    {
        let dir = if app.jobs_view.sort_ascending {
            "ASC"
        } else {
            "DESC"
        };
        let col_name = match app.jobs_view.sort_column {
            crate::tui::app::JobSortColumn::JobId => "ID",
            crate::tui::app::JobSortColumn::Name => "Name",
            crate::tui::app::JobSortColumn::Account => "Account",
            crate::tui::app::JobSortColumn::Partition => "Partition",
            crate::tui::app::JobSortColumn::State => "State",
            crate::tui::app::JobSortColumn::Time => "Time",
            crate::tui::app::JobSortColumn::Priority => "Priority",
            crate::tui::app::JobSortColumn::Gpus => "GPUs",
        };
        status_parts.push(Span::styled(
            format!(" Sort:{}/{}", col_name, dir),
            Style::default().fg(theme.account_highlight),
        ));
    }

    // Jobs summary
    let running = app.running_job_count();
    let pending = app.pending_job_count();
    status_parts.push(Span::styled(
        " Jobs: ".to_string(),
        Style::default().fg(theme.border),
    ));
    status_parts.push(Span::styled(
        format!("{} running", running),
        Style::default().fg(theme.running),
    ));
    status_parts.push(Span::raw(", "));
    status_parts.push(Span::styled(
        format!("{} pending", pending),
        Style::default().fg(theme.pending),
    ));

    // Problem nodes indicator
    let down = app.down_nodes().len();
    let draining = app.draining_nodes().len();
    if down > 0 || draining > 0 {
        status_parts.push(Span::raw(" | "));
        if down > 0 {
            status_parts.push(Span::styled(
                format!("{} down", down),
                Style::default().fg(theme.failed),
            ));
        }
        if down > 0 && draining > 0 {
            status_parts.push(Span::raw(", "));
        }
        if draining > 0 {
            status_parts.push(Span::styled(
                format!("{} draining", draining),
                Style::default().fg(theme.timeout),
            ));
        }
    }

    // Last update time
    status_parts.push(Span::raw(" | "));
    if let Some(age) = app.data.jobs.age() {
        let age_secs = age.as_secs();
        let age_str = if age_secs < 60 {
            format!("{}s", age_secs)
        } else {
            format!("{}m", age_secs / 60)
        };

        if app.data.jobs.is_stale() {
            status_parts.push(Span::styled(
                format!("Updated: {} (*STALE*)", age_str),
                Style::default().fg(theme.stale_indicator),
            ));
        } else {
            status_parts.push(Span::styled(
                format!("Updated: {}", age_str),
                Style::default().fg(theme.border),
            ));
        }
    } else {
        status_parts.push(Span::styled(
            "Loading...",
            Style::default().fg(theme.pending),
        ));
    }

    // Config warnings display (persistent until fixed)
    if !app.feedback.config_warnings.is_empty() {
        // Show first warning with count if multiple
        let warning_text = if app.feedback.config_warnings.len() == 1 {
            format!(" | WARN: {}", app.feedback.config_warnings[0])
        } else {
            format!(
                " | WARN: {} (+{} more)",
                app.feedback.config_warnings[0],
                app.feedback.config_warnings.len() - 1
            )
        };
        status_parts.push(Span::styled(
            warning_text,
            Style::default().fg(theme.progress_warn),
        ));
    }

    // Error display (temporary, auto-dismisses)
    if let Some(error) = app.current_error() {
        status_parts.push(Span::styled(
            format!(" | ERROR: {} ", error),
            Style::default().fg(theme.failed),
        ));
    }

    let status_line = Line::from(status_parts);
    let status_para = Paragraph::new(status_line);
    frame.render_widget(status_para, layout[1]);
}

fn render_help_overlay(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(65, 80, area);

    // Clear the area first
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(vec![Span::styled(
            "cmon TUI - Keyboard Shortcuts",
            Style::default().bold(),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().fg(theme.account_highlight).bold(),
        )]),
        Line::from("  j / Down       Move selection down"),
        Line::from("  k / Up         Move selection up"),
        Line::from("  g / Home       Jump to top"),
        Line::from("  G / End        Jump to bottom"),
        Line::from("  Ctrl+d / PgDn  Page down"),
        Line::from("  Ctrl+u / PgUp  Page up"),
        Line::from("  Mouse click    Select row"),
        Line::from("  Scroll wheel   Navigate up/down"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Views",
            Style::default().fg(theme.account_highlight).bold(),
        )]),
        Line::from("  1              Jobs view"),
        Line::from("  2              Nodes view"),
        Line::from("  3              Partitions view"),
        Line::from("  4              Personal (Me) view"),
        Line::from("  5              Problems view"),
        Line::from("  Tab            Cycle to next view"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Job Actions (Jobs view)",
            Style::default().fg(theme.account_highlight).bold(),
        )]),
        Line::from("  Enter          View job details"),
        Line::from("  c              Cancel selected job"),
        Line::from("  y              Copy job ID to clipboard"),
        Line::from("  s              Open sort menu"),
        Line::from("  /              Quick search (filter by text)"),
        Line::from("  f              Advanced filter (field:value syntax)"),
        Line::from("  a              Toggle My Jobs / All Jobs"),
        Line::from("  A              Cycle account context"),
        Line::from("  Ctrl+g         Toggle group by account"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Node Actions (Nodes view)",
            Style::default().fg(theme.account_highlight).bold(),
        )]),
        Line::from("  v              Toggle list/grid view"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default().fg(theme.account_highlight).bold(),
        )]),
        Line::from("  r              Force data refresh"),
        Line::from("  e              Export current view to JSON"),
        Line::from("  E (shift)      Export current view to CSV"),
        Line::from("  ?/F1           Show this help"),
        Line::from("  Esc            Close overlay / cancel"),
        Line::from("  q              Quit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press ? or Esc to close this help",
            Style::default().fg(theme.border),
        )]),
    ];

    let help_para = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_focused))
                .title(" Help "),
        )
        .style(Style::default().fg(theme.fg));

    frame.render_widget(help_para, popup_area);
}

fn render_filter_overlay(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    use crate::tui::app::FilterType;

    // Get filter state from modal
    let (filter_input, cursor_pos, filter_type) = match &app.modal {
        ModalState::Filter { edit_buffer, cursor, filter_type } => {
            (edit_buffer.as_str(), *cursor, *filter_type)
        }
        _ => return,
    };

    let is_advanced = filter_type == FilterType::Advanced;

    // Advanced filter shows syntax hints
    let popup_height = if is_advanced { 6 } else { 3 };
    let popup_area = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4).min(60),
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let title = if is_advanced {
        " Advanced Filter "
    } else {
        " Quick Search "
    };
    let prefix = if is_advanced { "filter: " } else { "/" };
    let input_text = format!("{}{}", prefix, filter_input);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.account_highlight))
        .title(title);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if is_advanced {
        // Show input and syntax hints
        let lines = vec![
            Line::from(input_text),
            Line::from(""),
            Line::from(vec![
                Span::styled("Syntax: ", Style::default().fg(theme.border)),
                Span::raw("user:name  partition:gpu  state:running  gpu:4"),
            ]),
            Line::from(vec![
                Span::styled("Negate: ", Style::default().fg(theme.border)),
                Span::raw("!partition:cpu  "),
                Span::styled("Combine: ", Style::default().fg(theme.border)),
                Span::raw("user:john state:pending"),
            ]),
        ];
        let para = Paragraph::new(lines).style(Style::default().fg(theme.fg));
        frame.render_widget(para, inner);

        // Show cursor on first line
        frame.set_cursor_position((
            inner.x + prefix.len() as u16 + cursor_pos as u16,
            inner.y,
        ));
    } else {
        let para = Paragraph::new(input_text).style(Style::default().fg(theme.fg));
        frame.render_widget(para, inner);

        // Show cursor
        frame.set_cursor_position((
            inner.x + prefix.len() as u16 + cursor_pos as u16,
            inner.y,
        ));
    }
}

/// Calculate scroll offset to keep selection visible
fn calculate_scroll_offset(selected: usize, visible_height: usize, total: usize) -> usize {
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
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

/// Truncate a string with ellipsis
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Format memory in human-readable format
fn format_memory(mb: u64) -> String {
    if mb >= 1024 * 1024 {
        format!("{:.1}T", mb as f64 / 1024.0 / 1024.0)
    } else if mb >= 1024 {
        format!("{:.0}G", mb as f64 / 1024.0)
    } else {
        format!("{}M", mb)
    }
}

/// Format duration in human-readable format (HH:MM:SS or D-HH:MM:SS)
fn format_duration_display(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours >= 24 {
        let days = hours / 24;
        let hours = hours % 24;
        format!("{}-{:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}

/// Create a progress bar as a Span
fn create_progress_bar(percent: f64, width: usize, theme: &Theme) -> Span<'static> {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);

    let color = theme.progress_color(percent);

    let bar = format!("[{}{}]", "=".repeat(filled), ".".repeat(empty));

    Span::styled(bar, Style::default().fg(color))
}

// ============================================================================
// Phase 3: Dialog and Overlay Implementations
// ============================================================================

/// Render the job detail popup
fn render_job_detail_popup(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(75, 80, area);
    frame.render_widget(Clear, popup_area);

    // Get job from Jobs view or Personal view
    let Some(job) = app.detail_job() else {
        return;
    };

    let state_color = theme.job_state_color(job.state);

    // Build title with state indicator
    let title = format!(
        " Job {} - {} [{}] ",
        job.job_id,
        job.name,
        job.state.as_str()
    );

    let border_color = match job.state {
        JobState::Running | JobState::Completing => theme.running,
        JobState::Pending => theme.pending,
        JobState::Failed | JobState::OutOfMemory | JobState::NodeFail => theme.failed,
        _ => theme.border_focused,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Split popup into scrollable content
    let mut lines = Vec::new();

    // ═══════════════════════════════════════════════════════════════
    // HEADER: State and basic info
    // ═══════════════════════════════════════════════════════════════
    lines.push(Line::from(vec![
        Span::styled("  State:       ", Style::default().bold()),
        Span::styled(job.state.as_str(), Style::default().fg(state_color).bold()),
        if !job.state_reason.is_empty() && job.state_reason != "None" {
            Span::styled(
                format!("  ({})", job.state_reason),
                Style::default().fg(theme.border),
            )
        } else {
            Span::raw("")
        },
    ]));
    lines.push(Line::from(vec![
        Span::styled("  User:        ", Style::default().bold()),
        Span::raw(&job.user_name),
        Span::raw("    "),
        Span::styled("Account: ", Style::default().bold()),
        Span::styled(&job.account, Style::default().fg(theme.account_highlight)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Partition:   ", Style::default().bold()),
        Span::raw(&job.partition),
        Span::raw("    "),
        Span::styled("QOS: ", Style::default().bold()),
        Span::raw(&job.qos),
    ]));

    // ═══════════════════════════════════════════════════════════════
    // TIME INFORMATION
    // ═══════════════════════════════════════════════════════════════
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Time Information",
            Style::default()
                .fg(theme.account_highlight)
                .bold()
                .underlined(),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Elapsed:     ", Style::default().bold()),
        Span::raw(job.elapsed_display()),
        Span::styled("  /  Limit: ", Style::default().fg(theme.border)),
        Span::raw(job.time_limit_display()),
    ]));

    // Time remaining with visual indicator
    if let Some(remaining) = job.time_remaining() {
        let remaining_str = format_duration_display(remaining.as_secs());
        let remaining_color = if remaining.as_secs() < 3600 {
            theme.progress_crit
        } else if remaining.as_secs() < 6 * 3600 {
            theme.progress_warn
        } else {
            theme.progress_full
        };
        lines.push(Line::from(vec![
            Span::styled("  Remaining:   ", Style::default().bold()),
            Span::styled(remaining_str, Style::default().fg(remaining_color).bold()),
        ]));
    }

    // Submit and start times
    if let Some(submit_dt) = job.submit_time.as_datetime() {
        lines.push(Line::from(vec![
            Span::styled("  Submitted:   ", Style::default().bold()),
            Span::raw(submit_dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }
    if let Some(start_dt) = job.start_time.as_datetime() {
        lines.push(Line::from(vec![
            Span::styled("  Started:     ", Style::default().bold()),
            Span::raw(start_dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }
    // End time (for completed/failed jobs)
    if let Some(end_dt) = job.end_time.as_datetime() {
        lines.push(Line::from(vec![
            Span::styled("  Ended:       ", Style::default().bold()),
            Span::raw(end_dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // RESOURCES
    // ═══════════════════════════════════════════════════════════════
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Resources",
            Style::default()
                .fg(theme.account_highlight)
                .bold()
                .underlined(),
        ),
    ]));

    // CPUs
    lines.push(Line::from(vec![
        Span::styled("  CPUs:        ", Style::default().bold()),
        Span::raw(format!("{}", job.cpus)),
        Span::styled("  (", Style::default().fg(theme.border)),
        Span::raw(format!("{} tasks", job.ntasks)),
        Span::styled(" x ", Style::default().fg(theme.border)),
        Span::raw(format!("{} cpus/task", job.cpus_per_task)),
        Span::styled(")", Style::default().fg(theme.border)),
    ]));

    // Memory
    if job.memory_gb > 0.01 {
        lines.push(Line::from(vec![
            Span::styled("  Memory:      ", Style::default().bold()),
            Span::raw(format!("{:.1} GB", job.memory_gb)),
        ]));
    }

    // GPUs
    if job.gpu_count > 0 {
        lines.push(Line::from(vec![
            Span::styled("  GPUs:        ", Style::default().bold()),
            Span::styled(
                format!("{}", job.gpu_count),
                Style::default().fg(theme.running),
            ),
            if let Some(ref gpu_type) = job.gpu_type {
                Span::styled(
                    format!(" ({})", gpu_type),
                    Style::default().fg(theme.border),
                )
            } else {
                Span::raw("")
            },
        ]));
    }

    // Nodes
    if !job.nodes.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Nodes:       ", Style::default().bold()),
            Span::raw(&job.nodes),
            Span::styled(
                format!(
                    "  ({} node{})",
                    job.node_count,
                    if job.node_count != 1 { "s" } else { "" }
                ),
                Style::default().fg(theme.border),
            ),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // PATHS
    // ═══════════════════════════════════════════════════════════════
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Paths",
            Style::default()
                .fg(theme.account_highlight)
                .bold()
                .underlined(),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  Work Dir:    ", Style::default().bold()),
        Span::styled(
            truncate_string(&job.working_directory, 55),
            Style::default().fg(theme.fg),
        ),
    ]));

    // Stdout path
    if !job.stdout_path.is_empty() && job.stdout_path != "/dev/null" {
        lines.push(Line::from(vec![
            Span::styled("  Stdout:      ", Style::default().bold()),
            Span::styled(
                truncate_string(&job.stdout_path, 55),
                Style::default().fg(theme.border),
            ),
        ]));
    }

    // Stderr path (only show if different from stdout)
    if !job.stderr_path.is_empty()
        && job.stderr_path != "/dev/null"
        && job.stderr_path != job.stdout_path
    {
        lines.push(Line::from(vec![
            Span::styled("  Stderr:      ", Style::default().bold()),
            Span::styled(
                truncate_string(&job.stderr_path, 55),
                Style::default().fg(theme.border),
            ),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // DEPENDENCIES (if any)
    // ═══════════════════════════════════════════════════════════════
    if !job.dependency.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "Dependencies",
                Style::default()
                    .fg(theme.account_highlight)
                    .bold()
                    .underlined(),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Depends on:  ", Style::default().bold()),
            Span::raw(truncate_string(&job.dependency, 55)),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // ARRAY JOB INFO
    // ═══════════════════════════════════════════════════════════════
    if job.is_array_job() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "Array Job",
                Style::default()
                    .fg(theme.account_highlight)
                    .bold()
                    .underlined(),
            ),
        ]));
        if let Some(array_id) = job.array_job_id {
            lines.push(Line::from(vec![
                Span::styled("  Array ID:    ", Style::default().bold()),
                Span::raw(format!("{}", array_id)),
            ]));
        }
        // Show task progress
        let running = job.array_tasks_running.unwrap_or(0);
        let pending = job.array_tasks_pending.unwrap_or(0);
        let completed = job.array_tasks_completed.unwrap_or(0);
        let total = job.array_task_count.unwrap_or(0);
        lines.push(Line::from(vec![
            Span::styled("  Tasks:       ", Style::default().bold()),
            Span::styled(
                format!("{} running", running),
                Style::default().fg(theme.running),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} pending", pending),
                Style::default().fg(theme.pending),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} done", completed),
                Style::default().fg(theme.completed),
            ),
            Span::styled(
                format!("  (/{} total)", total),
                Style::default().fg(theme.border),
            ),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // PRIORITY (for pending jobs)
    // ═══════════════════════════════════════════════════════════════
    if job.state == JobState::Pending {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Priority:    ", Style::default().bold()),
            Span::raw(format!("{}", job.priority)),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // CONSTRAINT (if any)
    // ═══════════════════════════════════════════════════════════════
    if !job.constraint.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Constraint:  ", Style::default().bold()),
            Span::styled(&job.constraint, Style::default().fg(theme.border)),
        ]));
    }

    // ═══════════════════════════════════════════════════════════════
    // FOOTER with keybindings
    // ═══════════════════════════════════════════════════════════════
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [Esc/Enter]", Style::default().fg(theme.border).bold()),
        Span::styled(" Close   ", Style::default().fg(theme.border)),
        Span::styled("[c]", Style::default().fg(theme.border).bold()),
        Span::styled(" Cancel Job   ", Style::default().fg(theme.border)),
        Span::styled("[y]", Style::default().fg(theme.border).bold()),
        Span::styled(" Copy ID", Style::default().fg(theme.border)),
    ]));

    let para = Paragraph::new(lines).style(Style::default().fg(theme.fg));
    frame.render_widget(para, inner);
}

/// Render the confirmation dialog
fn render_confirm_dialog(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(50, 25, area);
    frame.render_widget(Clear, popup_area);

    let Some(action) = app.modal.confirm_action() else {
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.progress_warn))
        .title(" Confirm Action ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(action.description(), Style::default().bold()),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Press "),
            Span::styled("[y/Enter]", Style::default().fg(theme.progress_warn).bold()),
            Span::raw(" to confirm, "),
            Span::styled("[n/Esc]", Style::default().fg(theme.border).bold()),
            Span::raw(" to cancel"),
        ]),
    ];

    let para = Paragraph::new(lines)
        .style(Style::default().fg(theme.fg))
        .alignment(Alignment::Left);
    frame.render_widget(para, inner);
}

/// Render the sort menu
fn render_sort_menu(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(30, 45, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .title(" Sort By ");

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let Some(sort_menu) = app.modal.sort_menu() else {
        return;
    };

    let mut lines = vec![Line::from("")];

    for (i, (label, column)) in sort_menu.columns.iter().enumerate() {
        let is_selected = i == sort_menu.selected;
        let is_current = *column == app.jobs_view.sort_column;

        let prefix = if is_selected { "> " } else { "  " };
        let suffix = if is_current {
            if app.jobs_view.sort_ascending {
                " [ASC]"
            } else {
                " [DESC]"
            }
        } else {
            ""
        };

        let style = if is_selected {
            Style::default().fg(theme.selected_fg).bg(theme.selected_bg)
        } else if is_current {
            Style::default().fg(theme.account_highlight)
        } else {
            Style::default()
        };

        lines.push(Line::from(Span::styled(
            format!("{}{}{}", prefix, label, suffix),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  [Enter] Select  [Esc] Cancel",
        Style::default().fg(theme.border),
    )]));

    let para = Paragraph::new(lines).style(Style::default().fg(theme.fg));
    frame.render_widget(para, inner);
}

/// Render clipboard feedback toast
fn render_clipboard_toast(
    feedback: &crate::tui::app::ClipboardFeedback,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
) {
    // Position toast at bottom-right
    let toast_width = (feedback.message.len() + 4).min(40) as u16;
    let toast_area = Rect {
        x: area.width.saturating_sub(toast_width + 2),
        y: area.height.saturating_sub(4),
        width: toast_width,
        height: 3,
    };

    frame.render_widget(Clear, toast_area);

    let border_color = if feedback.success {
        theme.running
    } else {
        theme.failed
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let para = Paragraph::new(format!(" {} ", feedback.message))
        .block(block)
        .style(Style::default().fg(theme.fg))
        .alignment(Alignment::Center);

    frame.render_widget(para, toast_area);
}
