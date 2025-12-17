//! UI rendering for the TUI
//!
//! This module handles all rendering using ratatui. The rendering is event-driven -
//! we only render when an event triggers a state change, not at a fixed frame rate.

mod jobs;
mod nodes;
mod overlays;
mod partitions;
mod personal;
mod problems;
mod widgets;

use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Tabs};

use crate::tui::app::{App, JobSortColumn, ModalState, View};
use crate::tui::theme::Theme;

use jobs::render_jobs_view;
use nodes::render_nodes_view;
use overlays::{
    render_clipboard_toast, render_confirm_dialog, render_filter_overlay, render_help_overlay,
    render_job_detail_popup, render_sort_menu,
};
use partitions::render_partitions_view;
use personal::render_personal_view;
use problems::render_problems_view;

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
        && (app.jobs_view.sort_column != JobSortColumn::JobId || !app.jobs_view.sort_ascending)
    {
        let dir = if app.jobs_view.sort_ascending {
            "ASC"
        } else {
            "DESC"
        };
        let col_name = match app.jobs_view.sort_column {
            JobSortColumn::JobId => "ID",
            JobSortColumn::Name => "Name",
            JobSortColumn::Account => "Account",
            JobSortColumn::Partition => "Partition",
            JobSortColumn::State => "State",
            JobSortColumn::Time => "Time",
            JobSortColumn::Priority => "Priority",
            JobSortColumn::Gpus => "GPUs",
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
