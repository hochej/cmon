//! Personal dashboard view rendering
//!
//! Handles rendering of the Personal view with user's jobs and fairshare information.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::formatting::{format_duration_hms, truncate_string};
use crate::tui::app::{App, PersonalPanel};
use crate::tui::theme::Theme;

use super::widgets::calculate_scroll_offset;

pub fn render_personal_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
        render_personal_jobs_panel(app, frame, chunks[2], theme, running_focused, PersonalPanel::Running);
        // Pending jobs section
        let pending_focused = app.personal_view.selected_panel == PersonalPanel::Pending;
        render_personal_jobs_panel(app, frame, chunks[3], theme, pending_focused, PersonalPanel::Pending);
    } else {
        // Running jobs section
        let running_focused = app.personal_view.selected_panel == PersonalPanel::Running;
        render_personal_jobs_panel(app, frame, chunks[1], theme, running_focused, PersonalPanel::Running);
        // Pending jobs section
        let pending_focused = app.personal_view.selected_panel == PersonalPanel::Pending;
        render_personal_jobs_panel(app, frame, chunks[2], theme, pending_focused, PersonalPanel::Pending);
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

/// Unified rendering function for personal job panels (Running and Pending).
/// Uses the existing `PersonalPanel` enum to distinguish between panel types.
fn render_personal_jobs_panel(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
    panel: PersonalPanel,
) {
    // Get jobs and panel-specific configuration based on panel type
    let (jobs, title_prefix, empty_msg, selected_idx) = match panel {
        PersonalPanel::Running => (
            app.my_running_jobs(),
            "Running Jobs",
            "No running jobs",
            app.personal_view.running_jobs_state.selected,
        ),
        PersonalPanel::Pending => (
            app.my_pending_jobs(),
            "Pending Jobs",
            "No pending jobs",
            app.personal_view.pending_jobs_state.selected,
        ),
        // Summary and Fairshare panels are handled by separate functions
        _ => return,
    };

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
            " {} ({}) {} ",
            title_prefix,
            jobs.len(),
            if focused { "[Tab to switch]" } else { "" }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if jobs.is_empty() {
        let msg = if app.data.jobs.last_updated.is_none() {
            "Loading jobs..."
        } else {
            empty_msg
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(theme.border))
            .alignment(Alignment::Center);
        frame.render_widget(para, inner);
        return;
    }

    // Table headers differ by panel type
    let headers: &[&str] = match panel {
        PersonalPanel::Running => &["ID", "Name", "Account", "Part", "Elapsed", "Remaining"],
        PersonalPanel::Pending => &["ID", "Name", "Account", "Part", "Reason", "Est.Start"],
        _ => return,
    };

    let header_cells = headers
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(theme.header_fg).bold()));
    let header = Row::new(header_cells)
        .style(Style::default().bg(theme.header_bg))
        .height(1);

    let available_height = inner.height.saturating_sub(1) as usize;
    let scroll_offset = calculate_scroll_offset(selected_idx, available_height, jobs.len());

    let rows: Vec<Row> = jobs
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(available_height)
        .map(|(idx, job)| {
            // First 4 columns are common
            let common_cells = vec![
                Cell::from(job.job_id.to_string()),
                Cell::from(truncate_string(&job.name, 18)),
                Cell::from(job.account.clone()),
                Cell::from(job.partition.clone()),
            ];

            // Last 2 columns differ by panel type
            let row = match panel {
                PersonalPanel::Running => {
                    // Calculate time remaining with color coding
                    let (remaining_str, remaining_color) =
                        if let Some(remaining) = job.time_remaining() {
                            let secs = remaining.as_secs();
                            let color = if secs < 3600 {
                                theme.progress_crit // Less than 1 hour - critical (red)
                            } else if secs < 6 * 3600 {
                                theme.progress_warn // Less than 6 hours - warning (orange)
                            } else {
                                theme.progress_full // Plenty of time (green)
                            };
                            (format_duration_hms(secs), color)
                        } else {
                            ("N/A".to_string(), theme.border)
                        };

                    let mut cells = common_cells;
                    cells.push(Cell::from(job.elapsed_display()));
                    cells.push(
                        Cell::from(remaining_str).style(Style::default().fg(remaining_color)),
                    );
                    Row::new(cells)
                }
                PersonalPanel::Pending => {
                    let mut cells = common_cells;
                    cells.push(Cell::from(truncate_string(&job.state_reason, 12)));
                    cells.push(Cell::from(job.estimated_start_display()));
                    Row::new(cells)
                }
                _ => return Row::new(common_cells),
            };

            // Highlight selected row if this panel is focused
            if focused && idx == selected_idx {
                row.style(Style::default().bg(theme.selected_bg))
            } else {
                row
            }
        })
        .collect();

    // Column widths differ slightly by panel type
    let widths: [Constraint; 6] = match panel {
        PersonalPanel::Running => [
            Constraint::Length(10), // ID
            Constraint::Min(12),    // Name
            Constraint::Length(10), // Account
            Constraint::Length(8),  // Partition
            Constraint::Length(11), // Elapsed
            Constraint::Length(12), // Remaining
        ],
        PersonalPanel::Pending => [
            Constraint::Length(10), // ID
            Constraint::Min(12),    // Name
            Constraint::Length(10), // Account
            Constraint::Length(8),  // Partition
            Constraint::Length(12), // Reason
            Constraint::Length(10), // Est.Start
        ],
        _ => return,
    };

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
        .data
        .fairshare_tree
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
