//! Partitions view rendering
//!
//! Handles rendering of the Partitions view with resource utilization cards.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::app::{App, PartitionStatus};
use crate::tui::theme::Theme;

use super::widgets::create_progress_bar;

pub fn render_partitions_view(app: &App, frame: &mut Frame, area: Rect, theme: &Theme) {
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
        partition.name, partition.total_nodes, status_indicator
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
