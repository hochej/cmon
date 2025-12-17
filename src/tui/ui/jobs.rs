//! Jobs view rendering
//!
//! Handles rendering of the Jobs view, including flat list and grouped-by-account modes.

use std::collections::BTreeMap;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};

use crate::formatting::truncate_string;
use crate::models::JobState;
use crate::tui::app::{App, TuiJobInfo};
use crate::tui::theme::Theme;

use super::widgets::{calculate_scroll_offset, create_table_header};

pub fn render_jobs_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
        .data
        .jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| app.is_job_visible(job))
        .collect();

    // Table header
    let header = create_table_header(
        &["ID", "Name", "Account", "Part", "State", "Time", "GPUs"],
        theme,
    );

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
    // Group jobs by account
    let visible_jobs: Vec<&TuiJobInfo> = app
        .data
        .jobs
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
        let header_row = Row::new(vec![Cell::from(account_summary).style(
            Style::default()
                .fg(theme.account_highlight)
                .bold()
                .add_modifier(Modifier::UNDERLINED),
        )]);
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
    let header = create_table_header(
        &["ID/Account", "Name", "Part", "State", "Time", "GPUs"],
        theme,
    );

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
