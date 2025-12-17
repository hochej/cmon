//! Overlay and popup rendering
//!
//! Handles rendering of help, filter, detail popup, confirm dialog, sort menu, and toast notifications.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::formatting::{format_duration_hms, truncate_string};
use crate::models::JobState;
use crate::tui::app::{App, ClipboardFeedback, FilterType, ModalState};
use crate::tui::theme::Theme;

use super::widgets::{centered_rect, detail_row, section_header};

pub fn render_help_overlay(frame: &mut Frame, area: Rect, theme: &Theme) {
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

pub fn render_filter_overlay(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    // Get filter state from modal
    let (filter_input, cursor_pos, filter_type) = match &app.modal {
        ModalState::Filter {
            edit_buffer,
            cursor,
            filter_type,
        } => (edit_buffer.as_str(), *cursor, *filter_type),
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

/// Render the job detail popup
pub fn render_job_detail_popup(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(75, 80, area);
    frame.render_widget(Clear, popup_area);

    // Get the focused job (works from Jobs view or Personal view)
    let Some(job) = app.focused_job() else {
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

    // HEADER: State and basic info
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

    // TIME INFORMATION
    lines.push(Line::from(""));
    lines.push(section_header("Time Information", theme));

    lines.push(Line::from(vec![
        Span::styled("  Elapsed:     ", Style::default().bold()),
        Span::raw(job.elapsed_display()),
        Span::styled("  /  Limit: ", Style::default().fg(theme.border)),
        Span::raw(job.time_limit_display()),
    ]));

    // Time remaining with visual indicator
    if let Some(remaining) = job.time_remaining() {
        let remaining_str = format_duration_hms(remaining.as_secs());
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

    // RESOURCES
    lines.push(Line::from(""));
    lines.push(section_header("Resources", theme));

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

    // PATHS
    lines.push(Line::from(""));
    lines.push(section_header("Paths", theme));

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

    // DEPENDENCIES (if any)
    if !job.dependency.is_empty() {
        lines.push(Line::from(""));
        lines.push(section_header("Dependencies", theme));
        lines.push(detail_row("Depends on:  ", &truncate_string(&job.dependency, 55)));
    }

    // ARRAY JOB INFO
    if job.is_array_job() {
        lines.push(Line::from(""));
        lines.push(section_header("Array Job", theme));
        if let Some(array_id) = job.array_job_id {
            lines.push(detail_row("Array ID:    ", &array_id.to_string()));
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

    // PRIORITY (for pending jobs)
    if job.state == JobState::Pending {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Priority:    ", Style::default().bold()),
            Span::raw(format!("{}", job.priority)),
        ]));
    }

    // CONSTRAINT (if any)
    if !job.constraint.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Constraint:  ", Style::default().bold()),
            Span::styled(&job.constraint, Style::default().fg(theme.border)),
        ]));
    }

    // FOOTER with keybindings
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
pub fn render_confirm_dialog(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
            Span::styled(
                "[y/Enter]",
                Style::default().fg(theme.progress_warn).bold(),
            ),
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
pub fn render_sort_menu(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
pub fn render_clipboard_toast(feedback: &ClipboardFeedback, frame: &mut Frame, area: Rect, theme: &Theme) {
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
