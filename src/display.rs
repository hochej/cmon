//! Display and formatting functions for cluster information

use crate::formatting::{
    format_bytes, format_bytes_mb, truncate_path, truncate_string,
};
use crate::models::{
    ClusterStatus, JobHistoryInfo, JobInfo, NodeInfo, PersonalSummary, format_duration_seconds,
};
use crate::slurm::{shorten_node_list, shorten_node_name};
use crate::utils::find_partition_key;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tabled::{
    Table, Tabled,
    settings::{Alignment, Modify, Style, Width, object::Rows},
};

// ============================================================================
// Box Drawing Constants
// ============================================================================
// All boxes use 78-character visible width (80 total with borders)

/// Type-safe color specification for box drawing.
///
/// Replaces stringly-typed color API (e.g., `"green"`) with compile-time checked
/// enum variants, preventing typos and ensuring exhaustive matching.
///
/// Note: `Yellow` and `Blue` are provided for API completeness. Currently only
/// `Green` and `Red` are used for colored boxes; the default `pad_line()` function
/// uses `.blue()` directly for non-colored borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Yellow and Blue kept for API completeness
pub enum BoxColor {
    Green,
    Red,
    Yellow,
    Blue,
}

impl BoxColor {
    /// Apply this color to the given text, returning a colored string.
    fn apply(&self, text: &str) -> String {
        match self {
            BoxColor::Green => text.green().to_string(),
            BoxColor::Red => text.red().to_string(),
            BoxColor::Yellow => text.yellow().to_string(),
            BoxColor::Blue => text.blue().to_string(),
        }
    }
}

/// Visible content width inside the box (excluding border characters)
const BOX_WIDTH: usize = 78;

/// Box border strings - pre-computed for efficiency
mod box_chars {
    /// Empty line (padding)
    pub const EMPTY: &str = "│                                                                              │";
    /// Bottom border
    pub const BOTTOM: &str = "╰──────────────────────────────────────────────────────────────────────────────╯";
    /// Left border character
    pub const LEFT: &str = "│";
    /// Right border character
    pub const RIGHT: &str = "│";
}

/// Create a centered title box top border
fn box_top(title: &str) -> String {
    // Calculate padding to center the title
    // BOX_WIDTH is the content width between corners, so dashes + title = BOX_WIDTH
    let title_len = title.len();
    let total_dashes = BOX_WIDTH - title_len;
    let left_dashes = total_dashes / 2;
    let right_dashes = total_dashes - left_dashes;

    format!(
        "╭{}{}{}╮",
        "─".repeat(left_dashes),
        title,
        "─".repeat(right_dashes)
    )
}

/// Create a box bottom border
fn box_bottom() -> &'static str {
    box_chars::BOTTOM
}

/// Create an empty box line
fn box_empty() -> &'static str {
    box_chars::EMPTY
}

/// Create a colored top border with title
fn box_top_colored(title: &str, color: BoxColor) -> String {
    color.apply(&box_top(title))
}

/// Create a colored bottom border
fn box_bottom_colored(color: BoxColor) -> String {
    color.apply(box_chars::BOTTOM)
}

/// Create a colored empty line
fn box_empty_colored(color: BoxColor) -> String {
    color.apply(box_chars::EMPTY)
}

/// Build a styled table with consistent formatting.
///
/// Applies the standard table style used throughout the CLI output:
/// - Rounded borders
/// - Word-preserving width wrapping
/// - Centered headers
///
/// # Arguments
/// * `rows` - Table rows implementing the `Tabled` trait
/// * `max_width` - Maximum table width in characters (typically 200, or 120 for compact views)
fn build_styled_table<T: Tabled>(rows: Vec<T>, max_width: usize) -> String {
    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(max_width).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));
    table.to_string()
}


/// Format node state with colored indicator
#[allow(clippy::if_same_then_else)]
pub fn format_node_state(node: &NodeInfo) -> String {
    let state_str = node.primary_state();

    // Choose symbol based on state category
    let symbol = if node.is_down() || node.is_fail() || node.is_failing() || node.is_inval() {
        "○".bright_red().to_string() // Empty circle for problem states
    } else if node.is_drained() || node.is_draining() || node.is_maint() {
        "◐".yellow().to_string() // Half-filled for maintenance
    } else if node.is_powered_down() || node.is_powering_down() || node.is_power_down() {
        "○".bright_black().to_string() // Empty for powered off
    } else if node.is_idle() {
        "●".green().to_string() // Filled green for available
    } else if node.is_mixed() || node.is_allocated() {
        "●".yellow().to_string() // Filled yellow for in-use
    } else if node.is_completing() {
        "◑".bright_yellow().to_string() // Transitional
    } else {
        "●".white().to_string() // Default filled
    };

    // Color the state text
    let colored_state = if node.is_down() || node.is_fail() || node.is_failing() || node.is_inval()
    {
        state_str.bright_red().to_string()
    } else if node.is_drained() {
        state_str.red().to_string()
    } else if node.is_draining() || node.is_maint() || node.is_reserved() {
        state_str.yellow().to_string()
    } else if node.is_reboot_requested() || node.is_reboot_issued() {
        state_str.magenta().to_string()
    } else if node.is_powered_down() || node.is_powering_down() || node.is_power_down() {
        state_str.bright_black().to_string()
    } else if node.is_powering_up() {
        state_str.cyan().to_string()
    } else if node.is_idle() {
        state_str.green().to_string()
    } else if node.is_mixed() {
        state_str.yellow().to_string()
    } else if node.is_allocated() {
        state_str.bright_yellow().to_string()
    } else if node.is_completing() {
        state_str.bright_yellow().to_string()
    } else if node.is_blocked() || node.is_perfctrs() {
        state_str.cyan().to_string()
    } else if node.is_future() || node.is_planned() {
        state_str.blue().to_string()
    } else if node.is_cloud() {
        state_str.bright_blue().to_string()
    } else if node.is_unknown() {
        state_str.white().to_string()
    } else {
        state_str.white().to_string()
    };

    format!("{} {}", symbol, colored_state)
}

/// Format CPU usage with coloring
pub fn format_cpu_usage(node: &NodeInfo) -> String {
    let allocated = node.cpus.allocated;
    let total = node.cpus.total;
    let usage = format!("{}/{}", allocated, total);

    if allocated == 0 {
        usage.green().to_string()
    } else if allocated == total {
        usage.red().to_string()
    } else {
        usage.yellow().to_string()
    }
}

/// Format memory usage with coloring
pub fn format_memory_usage(node: &NodeInfo) -> String {
    let total = node.memory_total();
    let free = node.memory_free();
    let used = total.saturating_sub(free);

    let usage = format!("{}/{}", format_bytes_mb(used), format_bytes_mb(total));

    let utilization = node.memory_utilization();
    if utilization < 10.0 {
        usage.green().to_string()
    } else if utilization > 80.0 {
        usage.red().to_string()
    } else {
        usage.green().to_string()
    }
}

/// Format GPU usage with coloring
pub fn format_gpu_usage(node: &NodeInfo) -> String {
    let gpu_info = node.gpu_info();

    if gpu_info.total == 0 {
        "-".white().to_string()
    } else {
        let gpu_type = gpu_info.gpu_type.to_lowercase();
        let usage = format!("{}/{} {}", gpu_info.used, gpu_info.total, gpu_type);

        if gpu_info.used == 0 {
            usage.green().to_string()
        } else if gpu_info.used == gpu_info.total {
            usage.red().to_string()
        } else {
            usage.yellow().to_string()
        }
    }
}

/// Format node reason with coloring
pub fn format_node_reason(node: &NodeInfo) -> String {
    let reason = node.reason_description();

    if reason.is_empty() || reason == "None" {
        "-".white().to_string()
    } else {
        // Color-code based on state and reason content
        if node.is_down() || node.is_fail() || node.is_failing() {
            reason.bright_red().to_string()
        } else if node.is_drained() {
            reason.red().to_string()
        } else if node.is_draining() || node.is_maint() {
            reason.yellow().to_string()
        } else {
            reason.white().to_string()
        }
    }
}

/// Table row for node display
#[derive(Tabled)]
struct NodeRow {
    #[tabled(rename = "Node")]
    node: String,

    #[tabled(rename = "State")]
    state: String,

    #[tabled(rename = "Reason")]
    reason: String,

    #[tabled(rename = "CPU")]
    cpu: String,

    #[tabled(rename = "Memory")]
    memory: String,

    #[tabled(rename = "GPU")]
    gpu: String,
}

/// Display nodes in a table format
///
/// # Arguments
/// * `nodes` - The nodes to display
/// * `node_prefix_strip` - Optional prefix to strip from node names (empty = no stripping)
pub fn format_nodes(nodes: &[NodeInfo], node_prefix_strip: &str) -> String {
    if nodes.is_empty() {
        return "No nodes found".yellow().to_string();
    }

    let rows: Vec<NodeRow> = nodes
        .iter()
        .map(|node| NodeRow {
            node: shorten_node_name(node.name(), node_prefix_strip).to_string(),
            state: format_node_state(node),
            reason: format_node_reason(node),
            cpu: format_cpu_usage(node),
            memory: format_memory_usage(node),
            gpu: format_gpu_usage(node),
        })
        .collect();

    build_styled_table(rows, 200)
}

/// Table row for job display
#[derive(Tabled)]
struct JobRow {
    #[tabled(rename = "JobID")]
    job_id: String,

    #[tabled(rename = "Name")]
    name: String,

    #[tabled(rename = "User")]
    user: String,

    #[tabled(rename = "Partition")]
    partition: String,

    #[tabled(rename = "State")]
    state: String,

    #[tabled(rename = "Reason")]
    reason: String,

    #[tabled(rename = "Nodes")]
    nodes: String,

    #[tabled(rename = "CPUs")]
    cpus: String,

    #[tabled(rename = "GPUs")]
    gpus: String,

    #[tabled(rename = "Time")]
    time: String,
}

/// Format job state with appropriate coloring
fn format_job_state(job: &JobInfo) -> String {
    let state_str = job.primary_state();

    // Color based on state category
    if job.is_running() {
        state_str.green().to_string()
    } else if job.is_pending() || job.is_configuring() {
        state_str.yellow().to_string()
    } else if job.is_completed() {
        state_str.bright_blue().to_string()
    } else if job.is_failed()
        || job.is_timeout()
        || job.is_node_fail()
        || job.is_boot_fail()
        || job.is_out_of_memory()
        || job.is_deadline()
    {
        state_str.red().to_string()
    } else if job.is_cancelled() || job.is_preempted() {
        state_str.magenta().to_string()
    } else if job.is_suspended() || job.is_stopped() || job.is_requeued() {
        state_str.cyan().to_string()
    } else if job.is_completing() || job.is_signaling() || job.is_stage_out() {
        state_str.bright_yellow().to_string()
    } else {
        state_str.white().to_string()
    }
}

/// Format state reason for display
fn format_state_reason(job: &JobInfo) -> String {
    if job.state_reason.is_empty() {
        "-".white().to_string()
    } else {
        // Color reasons based on type
        let reason = &job.state_reason;
        if reason.contains("Resources") || reason.contains("Priority") {
            reason.yellow().to_string()
        } else if reason.contains("Dependency") {
            reason.cyan().to_string()
        } else if reason.contains("QOS") || reason.contains("Association") {
            reason.magenta().to_string()
        } else {
            reason.white().to_string()
        }
    }
}

pub fn format_jobs(jobs: &[JobInfo], show_all: bool, node_prefix_strip: &str) -> String {
    let filtered_jobs: Vec<&JobInfo> = if show_all {
        jobs.iter().collect()
    } else {
        jobs.iter().filter(|j| j.is_running()).collect()
    };

    if filtered_jobs.is_empty() {
        return "No jobs found".yellow().to_string();
    }

    let rows: Vec<JobRow> = filtered_jobs
        .iter()
        .map(|job| {
            let cpus = job.cpus_per_task.number() * job.tasks.number();
            let gpu_info = job.gpu_type_info();

            JobRow {
                job_id: job.job_id.to_string(),
                name: truncate_string(&job.name, 35),
                user: job.user_name.clone(),
                partition: job.partition.clone(),
                state: format_job_state(job),
                reason: format_state_reason(job),
                nodes: shorten_node_list(&job.nodes, node_prefix_strip),
                cpus: cpus.to_string(),
                gpus: gpu_info.display,
                time: job.remaining_time_display(),
            }
        })
        .collect();

    build_styled_table(rows, 200)
}

/// Format cluster status display
///
/// # Arguments
/// * `status` - The cluster status data
/// * `partition_order` - Optional ordering for partitions (empty = alphabetical)
pub fn format_cluster_status(status: &ClusterStatus, partition_order: &[String]) -> String {
    let total_nodes = status.total_nodes();
    let idle_nodes = status.idle_nodes();
    let mixed_nodes = status.mixed_nodes();
    let down_nodes = status.down_nodes();
    let allocated_nodes = total_nodes - idle_nodes - mixed_nodes - down_nodes;

    let total_cpus = status.total_cpus();
    let allocated_cpus = status.allocated_cpus();
    let cpu_util = status.cpu_utilization();

    let total_jobs = status.jobs.len();

    // Count GPUs
    let mut total_gpus = 0u32;
    let mut used_gpus = 0u32;
    for node in &status.nodes {
        let gpu_info = node.gpu_info();
        total_gpus += gpu_info.total;
        used_gpus += gpu_info.used;
    }
    let gpu_util = if total_gpus > 0 {
        (used_gpus as f64 / total_gpus as f64) * 100.0
    } else {
        0.0
    };

    let mut output = String::new();

    output.push_str(&format!("\n{}\n", box_top(" Cluster Status ").blue()));
    output.push_str(&format!("{}\n", box_empty().blue()));

    let header = format!(
        "  {} (as of {})",
        "Cluster Overview".bold(),
        chrono::Local::now().format("%H:%M:%S")
    );
    output.push_str(&format!("{}\n", pad_line(&header)));

    output.push_str(&format!("{}\n", box_empty().blue()));

    let nodes_line = format!(
        "  {}: {} total • {} idle • {} mixed • {} allocated • {}",
        "Nodes".green(),
        total_nodes,
        idle_nodes,
        mixed_nodes,
        allocated_nodes,
        format!("{} down", down_nodes).red()
    );
    output.push_str(&format!("{}\n", pad_line(&nodes_line)));

    let cpus_line = format!(
        "  {}: {}/{} cores ({:.1}% utilized)",
        "CPUs".blue(),
        allocated_cpus,
        total_cpus,
        cpu_util
    );
    output.push_str(&format!("{}\n", pad_line(&cpus_line)));

    let jobs_line = format!("  {}: {} running", "Jobs".yellow(), total_jobs);
    output.push_str(&format!("{}\n", pad_line(&jobs_line)));

    if total_gpus > 0 {
        let gpus_line = format!(
            "  {}: {}/{} ({:.1}% utilized)",
            "GPUs".magenta(),
            used_gpus,
            total_gpus,
            gpu_util
        );
        output.push_str(&format!("{}\n", pad_line(&gpus_line)));
    }

    output.push_str(&format!("{}\n", box_empty().blue()));
    output.push_str(&format!("{}\n", box_bottom().blue()));

    // Partition stats
    output.push_str(&format!(
        "\n{}\n",
        box_top(" Partition Utilization ").blue()
    ));
    output.push_str(&format!("{}\n", box_empty().blue()));

    output.push_str(&format_partition_stats(status, partition_order));

    output.push_str(&format!("{}\n", box_bottom().blue()));

    output
}

/// Strip ANSI color codes to calculate visible width
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape && ch == 'm' {
            in_escape = false;
        } else if !in_escape {
            result.push(ch);
        }
    }

    result
}

/// Pad a line to fit within the box (78 chars visible width)
fn pad_line(content: &str) -> String {
    let visible_len = strip_ansi(content).chars().count();
    let padding = if visible_len < BOX_WIDTH {
        " ".repeat(BOX_WIDTH - visible_len)
    } else {
        String::new()
    };

    format!(
        "{}{}{}",
        box_chars::LEFT.blue(),
        content,
        padding + &box_chars::RIGHT.blue().to_string()
    )
}

/// Display partition statistics
///
/// Groups nodes by their actual Slurm partition field (portable across clusters).
/// Partition order can be configured; defaults to alphabetical.
fn format_partition_stats(status: &ClusterStatus, partition_order: &[String]) -> String {
    let mut output = String::new();
    let mut partitions: HashMap<String, Vec<&NodeInfo>> = HashMap::new();

    // Group nodes by their actual partition name from Slurm (preserves original case)
    for node in &status.nodes {
        partitions
            .entry(node.partition_name())
            .or_default()
            .push(node);
    }

    // Determine display order: configured order first, then remaining alphabetically
    let mut ordered_names: Vec<String> = Vec::new();

    // Add configured partitions in order (case-insensitive match to actual partition names)
    for config_name in partition_order {
        if let Some(actual_name) = find_partition_key(partitions.keys(), config_name) {
            ordered_names.push(actual_name.clone());
        }
    }

    // Add remaining partitions alphabetically (case-insensitive sort)
    let mut remaining: Vec<&String> = partitions
        .keys()
        .filter(|k| find_partition_key(ordered_names.iter(), k).is_none())
        .collect();
    remaining.sort_by_key(|a| a.to_lowercase());
    for name in remaining {
        ordered_names.push(name.clone());
    }

    for partition_name in ordered_names {
        let nodes = match partitions.get(&partition_name) {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };

        let node_count = nodes.len();
        let total_cpus: u32 = nodes.iter().map(|n| n.cpus.total).sum();
        let allocated_cpus: u32 = nodes.iter().map(|n| n.cpus.allocated).sum();
        let cpu_util = if total_cpus > 0 {
            (allocated_cpus as f64 / total_cpus as f64) * 100.0
        } else {
            0.0
        };

        let total_mem: u64 = nodes.iter().map(|n| n.memory_total()).sum();
        let allocated_mem: u64 = nodes.iter().map(|n| n.memory.allocated).sum();
        let mem_util = if total_mem > 0 {
            (allocated_mem as f64 / total_mem as f64) * 100.0
        } else {
            0.0
        };

        // Use partition name as-is from the cluster (preserve original casing)
        let header = format!("  {} ({} nodes):", partition_name.bold(), node_count);
        output.push_str(&format!("{}\n", pad_line(&header)));

        let cpu_bar = create_bar(cpu_util);
        let cpu_line = format!(
            "    CPUs:    {}   {:.0}% ({}/{})",
            cpu_bar, cpu_util, allocated_cpus, total_cpus
        );
        output.push_str(&format!("{}\n", pad_line(&cpu_line)));

        let mem_bar = create_bar(mem_util);
        let mem_line = format!(
            "    Memory:  {}   {:.0}% ({}/{} GB)",
            mem_bar,
            mem_util,
            format_bytes_mb(allocated_mem),
            format_bytes_mb(total_mem)
        );
        output.push_str(&format!("{}\n", pad_line(&mem_line)));

        // GPU stats (shown only if partition has GPUs)
        // Collect all GPU info in a single pass to avoid calling gpu_info() multiple times per node
        let (total_gpus, used_gpus, gpu_type) = nodes.iter().fold(
            (0u32, 0u32, None::<String>),
            |(total, used, gtype), node| {
                let info = node.gpu_info();
                let new_gtype = gtype.or_else(|| {
                    if info.gpu_type.is_empty() {
                        None
                    } else {
                        Some(info.gpu_type.to_uppercase())
                    }
                });
                (total + info.total, used + info.used, new_gtype)
            },
        );
        if total_gpus > 0 {
            let gpu_util = (used_gpus as f64 / total_gpus as f64) * 100.0;
            let gpu_type = gpu_type.unwrap_or_else(|| "GPU".to_string());

            let gpu_bar = create_bar(gpu_util);
            let gpu_line = format!(
                "    GPUs:    {}   {:.0}% ({}/{} {})",
                gpu_bar, gpu_util, used_gpus, total_gpus, gpu_type
            );
            output.push_str(&format!("{}\n", pad_line(&gpu_line)));
        }

        output.push_str(&format!(
            "{}\n",
            "│                                                                              │"
                .blue()
        ));
    }

    output
}

/// Create a utilization bar
fn create_bar(utilization: f64) -> String {
    let bar_length = 20;
    let filled = ((utilization / 100.0) * bar_length as f64) as usize;
    let empty = bar_length - filled;

    let filled_part = "█".repeat(filled);
    let empty_part = "░".repeat(empty);

    if utilization > 80.0 {
        format!("{}{}", filled_part.red(), empty_part.white())
    } else if utilization > 50.0 {
        format!("{}{}", filled_part.yellow(), empty_part.white())
    } else {
        format!("{}{}", filled_part.green(), empty_part.white())
    }
}

/// Format personal summary dashboard
pub fn format_personal_summary(summary: &PersonalSummary, node_prefix_strip: &str) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("\n{}\n", box_top(" My Dashboard ").blue()));
    output.push_str(&format!("{}\n", box_empty().blue()));

    let header = format!(
        "  {} {} (as of {})",
        "User:".bold(),
        summary.username.cyan(),
        chrono::Local::now().format("%H:%M:%S")
    );
    output.push_str(&format!("{}\n", pad_line(&header)));

    output.push_str(&format!("{}\n", box_empty().blue()));

    // Current Jobs Section
    let jobs_header = format!("  {}", "Current Jobs".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&jobs_header)));

    let running_str = if summary.running_jobs > 0 {
        format!("{} running", summary.running_jobs)
            .green()
            .to_string()
    } else {
        "0 running".white().to_string()
    };

    let pending_str = if summary.pending_jobs > 0 {
        format!("{} pending", summary.pending_jobs)
            .yellow()
            .to_string()
    } else {
        "0 pending".white().to_string()
    };

    let jobs_line = format!("    {} | {}", running_str, pending_str);
    output.push_str(&format!("{}\n", pad_line(&jobs_line)));

    output.push_str(&format!("{}\n", box_empty().blue()));

    // Last 24 Hours Section
    let history_header = format!("  {}", "Last 24 Hours".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&history_header)));

    let completed_str = format!("{} completed", summary.completed_24h)
        .green()
        .to_string();
    let failed_str = if summary.failed_24h > 0 {
        format!("{} failed", summary.failed_24h).red().to_string()
    } else {
        "0 failed".white().to_string()
    };
    let timeout_str = if summary.timeout_24h > 0 {
        format!("{} timeout", summary.timeout_24h)
            .yellow()
            .to_string()
    } else {
        "0 timeout".white().to_string()
    };
    let cancelled_str = if summary.cancelled_24h > 0 {
        format!("{} cancelled", summary.cancelled_24h)
            .magenta()
            .to_string()
    } else {
        "0 cancelled".white().to_string()
    };

    let history_line = format!(
        "    {} | {} | {} | {}",
        completed_str, failed_str, timeout_str, cancelled_str
    );
    output.push_str(&format!("{}\n", pad_line(&history_line)));

    output.push_str(&format!("{}\n", box_empty().blue()));

    // Resource Usage Section
    let resource_header = format!("  {}", "Resource Usage (24h)".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&resource_header)));

    let cpu_hours = format!("{:.1} CPU-hours", summary.total_cpu_hours_24h);
    let gpu_hours = if summary.total_gpu_hours_24h > 0.0 {
        format!(" | {:.1} GPU-hours", summary.total_gpu_hours_24h)
    } else {
        String::new()
    };
    let resource_line = format!("    {}{}", cpu_hours.cyan(), gpu_hours.magenta());
    output.push_str(&format!("{}\n", pad_line(&resource_line)));

    output.push_str(&format!("{}\n", box_empty().blue()));

    // Efficiency Section
    let efficiency_header = format!("  {}", "Efficiency Metrics".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&efficiency_header)));

    let cpu_eff = format_labeled_efficiency("CPU", summary.avg_cpu_efficiency);
    let mem_eff = format_labeled_efficiency("Memory", summary.avg_memory_efficiency);

    let wait_time = match summary.avg_wait_time_seconds {
        Some(secs) => format!("Avg Wait: {}", format_duration_seconds(secs)),
        None => "Avg Wait: -".to_string(),
    };

    let efficiency_line = format!("    {} | {} | {}", cpu_eff, mem_eff, wait_time);
    output.push_str(&format!("{}\n", pad_line(&efficiency_line)));

    output.push_str(&format!("{}\n", box_empty().blue()));
    output.push_str(&format!("{}\n", box_bottom().blue()));

    // Current jobs table (if any)
    if !summary.current_jobs.is_empty() {
        output.push('\n');
        output.push_str(&format!("{}\n", "Current Jobs:".bold()));
        output.push_str(&format_jobs(&summary.current_jobs, true, node_prefix_strip));
    }

    // Recent jobs table (limit to last 10)
    let recent: Vec<&JobHistoryInfo> = summary
        .recent_jobs
        .iter()
        .filter(|j| !j.is_running() && !j.is_pending())
        .take(10)
        .collect();

    if !recent.is_empty() {
        output.push('\n');
        output.push_str(&format!("{}\n", "Recent Completed Jobs:".bold()));
        output.push_str(&format_job_history_brief(&recent));
    }

    output
}

/// Format brief job history table
fn format_job_history_brief(jobs: &[&JobHistoryInfo]) -> String {
    if jobs.is_empty() {
        return "No recent jobs".yellow().to_string();
    }

    #[derive(Tabled)]
    struct HistoryRow {
        #[tabled(rename = "JobID")]
        job_id: String,

        #[tabled(rename = "Name")]
        name: String,

        #[tabled(rename = "State")]
        state: String,

        #[tabled(rename = "Exit")]
        exit_code: String,

        #[tabled(rename = "Elapsed")]
        elapsed: String,

        #[tabled(rename = "CPU%")]
        cpu_eff: String,

        #[tabled(rename = "Mem%")]
        mem_eff: String,
    }

    let rows: Vec<HistoryRow> = jobs
        .iter()
        .map(|job| {
            let colored_state = format_history_state(job);

            let cpu_eff = job
                .cpu_efficiency()
                .map(format_efficiency)
                .unwrap_or_else(|| "-".white().to_string());

            let mem_eff = job
                .memory_efficiency()
                .map(format_efficiency)
                .unwrap_or_else(|| "-".white().to_string());

            let exit_display = if job.is_completed() {
                "0".green().to_string()
            } else if let Some(code) = job.exit_code.return_code.value() {
                if code != 0 {
                    format!("{}", code).red().to_string()
                } else {
                    "-".white().to_string()
                }
            } else if !job.exit_code.signal.name.is_empty() {
                format!("SIG{}", job.exit_code.signal.name)
                    .yellow()
                    .to_string()
            } else {
                "-".white().to_string()
            };

            HistoryRow {
                job_id: job.job_id.to_string(),
                name: truncate_string(&job.name, 20),
                state: colored_state,
                exit_code: exit_display,
                elapsed: job.elapsed_display(),
                cpu_eff,
                mem_eff,
            }
        })
        .collect();

    build_styled_table(rows, 200)
}

// ============================================================================
// Job Details Section Builders
// ============================================================================
// These helpers decompose format_job_details() into logical sections,
// each returning padded lines ready to be joined into the final output.

/// Build the basic info section (Job ID, Name, State, User, Partition, Nodes)
fn build_job_basic_info(job: &JobHistoryInfo, node_prefix_strip: &str) -> Vec<String> {
    let mut lines = vec![
        pad_line(&format!("  {} {}", "Job ID:".bold(), job.job_id.to_string().cyan())),
        pad_line(&format!("  {} {}", "Name:".bold(), job.name)),
        pad_line(&format!("  {} {}", "State:".bold(), format_history_state(job))),
        pad_line(&format!("  {} {} ({})", "User:".bold(), job.user, job.account)),
        pad_line(&format!(
            "  {} {} (QoS: {})",
            "Partition:".bold(),
            job.partition,
            job.qos
        )),
    ];

    if !job.nodes.is_empty() && job.nodes != "None assigned" {
        lines.push(pad_line(&format!(
            "  {} {}",
            "Nodes:".bold(),
            shorten_node_list(&job.nodes, node_prefix_strip)
        )));
    }

    lines
}

/// Build the time information section (Submit, Start, End, Elapsed, Wait Time)
fn build_job_time_info(job: &JobHistoryInfo) -> Vec<String> {
    let mut lines = vec![
        pad_line(&format!("  {}", "Time Information".bold().underline())),
        pad_line(&format!("    Submitted:  {}", job.submit_time_display())),
    ];

    if job.time.start > 0 {
        lines.push(pad_line(&format!("    Started:    {}", job.start_time_display())));
    }

    if job.time.end > 0 {
        lines.push(pad_line(&format!("    Ended:      {}", job.end_time_display())));
    }

    lines.push(pad_line(&format!(
        "    Elapsed:    {} / {}",
        job.elapsed_display(),
        job.time_limit_display()
    )));

    if let Some(wait) = job.wait_time() {
        lines.push(pad_line(&format!("    Wait Time:  {}", format_duration_seconds(wait))));
    }

    lines
}

/// Build the resource allocation section (CPUs, Memory, GPUs)
fn build_job_resources(job: &JobHistoryInfo) -> Vec<String> {
    let mut lines = vec![
        pad_line(&format!("  {}", "Resource Allocation".bold().underline())),
        pad_line(&format!("    CPUs:       {}", job.required.cpus)),
    ];

    let requested_mem = job.requested_memory();
    if requested_mem > 0 {
        lines.push(pad_line(&format!("    Memory:     {}", format_bytes(requested_mem))));
    }

    let gpus = job.allocated_gpus();
    if gpus > 0 {
        let gpu_type = job.gpu_type().unwrap_or_else(|| "GPU".to_string());
        lines.push(pad_line(&format!("    GPUs:       {}x {}", gpus, gpu_type)));
    }

    lines
}

/// Build the efficiency metrics section (CPU and Memory efficiency with bars)
fn build_job_efficiency(job: &JobHistoryInfo) -> Vec<String> {
    let mut lines = vec![
        pad_line(&format!("  {}", "Efficiency Metrics".bold().underline())),
    ];

    // CPU Efficiency
    let cpu_line = if let Some(cpu_eff) = job.cpu_efficiency() {
        format!("    CPU:        {} {:.1}%", create_efficiency_bar(cpu_eff), cpu_eff)
    } else {
        "    CPU:        (no data)".to_string()
    };
    lines.push(pad_line(&cpu_line));

    // Memory Efficiency
    let requested_mem = job.requested_memory();
    let max_mem = job.max_memory_used();
    let mem_line = if max_mem > 0 && requested_mem > 0 {
        if let Some(mem_eff) = job.memory_efficiency() {
            format!(
                "    Memory:     {} {:.1}% ({} / {})",
                create_efficiency_bar(mem_eff),
                mem_eff,
                format_bytes(max_mem),
                format_bytes(requested_mem)
            )
        } else {
            "    Memory:     (no data)".to_string()
        }
    } else {
        "    Memory:     (no data)".to_string()
    };
    lines.push(pad_line(&mem_line));

    lines
}

/// Build the exit information section (Exit Code, Signal) - only for completed jobs
fn build_job_exit_info(job: &JobHistoryInfo) -> Option<Vec<String>> {
    if job.is_running() || job.is_pending() {
        return None;
    }

    let mut lines = vec![
        pad_line(&format!("  {}", "Exit Information".bold().underline())),
    ];

    let exit_code_display = if let Some(code) = job.exit_code.return_code.value() {
        if code == 0 {
            "0 (Success)".green().to_string()
        } else {
            format!("{} (Error)", code).red().to_string()
        }
    } else {
        "-".white().to_string()
    };
    lines.push(pad_line(&format!("    Exit Code:  {}", exit_code_display)));

    if !job.exit_code.signal.name.is_empty() {
        lines.push(pad_line(&format!("    Signal:     {}", job.exit_code.signal.name.red())));
    }

    Some(lines)
}

/// Build the paths section (Work Dir, Stdout, Stderr)
fn build_job_paths(job: &JobHistoryInfo) -> Vec<String> {
    let mut lines = vec![
        pad_line(&format!("  {}", "Paths".bold().underline())),
    ];

    if !job.working_directory.is_empty() {
        lines.push(pad_line(&format!(
            "    Work Dir:   {}",
            truncate_path(&job.working_directory, 50)
        )));
    }

    if !job.stdout.is_empty() {
        lines.push(pad_line(&format!("    Stdout:     {}", truncate_path(&job.stdout, 50))));
    }

    if !job.stderr.is_empty() && job.stderr != job.stdout {
        lines.push(pad_line(&format!("    Stderr:     {}", truncate_path(&job.stderr, 50))));
    }

    lines
}

/// Build the submit command section - only if submit_line is present
fn build_job_submit_line(job: &JobHistoryInfo) -> Option<Vec<String>> {
    if job.submit_line.is_empty() {
        return None;
    }

    let mut lines = vec![
        pad_line(&format!("  {}", "Submit Command".bold().underline())),
    ];

    // Wrap long submit lines - 72 chars max (78 box width - 4 indent - 2 borders)
    for wrapped_line in wrap_text_smart(&job.submit_line, 72) {
        lines.push(pad_line(&format!("    {}", wrapped_line.bright_black())));
    }

    Some(lines)
}

/// Format a single job's detailed information
pub fn format_job_details(job: &JobHistoryInfo, node_prefix_strip: &str) -> String {
    let mut output = String::new();
    let empty_line = format!("{}\n", box_empty().blue());

    // Header
    output.push_str(&format!("\n{}\n", box_top(" Job Details ").blue()));
    output.push_str(&empty_line);

    // Basic Info
    for line in build_job_basic_info(job, node_prefix_strip) {
        output.push_str(&format!("{line}\n"));
    }
    output.push_str(&empty_line);

    // Time Information
    for line in build_job_time_info(job) {
        output.push_str(&format!("{line}\n"));
    }
    output.push_str(&empty_line);

    // Resource Allocation
    for line in build_job_resources(job) {
        output.push_str(&format!("{line}\n"));
    }
    output.push_str(&empty_line);

    // Efficiency Metrics
    for line in build_job_efficiency(job) {
        output.push_str(&format!("{line}\n"));
    }
    output.push_str(&empty_line);

    // Exit Information (conditional)
    if let Some(exit_lines) = build_job_exit_info(job) {
        for line in exit_lines {
            output.push_str(&format!("{line}\n"));
        }
        output.push_str(&empty_line);
    }

    // Paths
    for line in build_job_paths(job) {
        output.push_str(&format!("{line}\n"));
    }
    output.push_str(&empty_line);

    // Submit Command (conditional)
    if let Some(submit_lines) = build_job_submit_line(job) {
        for line in submit_lines {
            output.push_str(&format!("{line}\n"));
        }
        output.push_str(&empty_line);
    }

    // Footer
    output.push_str(&format!("{}\n", box_bottom().blue()));

    output
}

/// Format job history table
pub fn format_job_history(jobs: &[JobHistoryInfo], show_efficiency: bool) -> String {
    if jobs.is_empty() {
        return "No jobs found".yellow().to_string();
    }

    #[derive(Tabled)]
    struct HistoryRow {
        #[tabled(rename = "JobID")]
        job_id: String,

        #[tabled(rename = "Name")]
        name: String,

        #[tabled(rename = "Partition")]
        partition: String,

        #[tabled(rename = "State")]
        state: String,

        #[tabled(rename = "Exit")]
        exit_code: String,

        #[tabled(rename = "Elapsed")]
        elapsed: String,

        #[tabled(rename = "Wait")]
        wait: String,

        #[tabled(rename = "CPUs")]
        cpus: String,

        #[tabled(rename = "CPU%")]
        cpu_eff: String,

        #[tabled(rename = "Mem%")]
        mem_eff: String,
    }

    let rows: Vec<HistoryRow> = jobs
        .iter()
        .map(|job| {
            let colored_state = format_history_state(job);

            let cpu_eff = if show_efficiency {
                job.cpu_efficiency()
                    .map(format_efficiency)
                    .unwrap_or_else(|| "-".white().to_string())
            } else {
                "-".white().to_string()
            };

            let mem_eff = if show_efficiency {
                job.memory_efficiency()
                    .map(format_efficiency)
                    .unwrap_or_else(|| "-".white().to_string())
            } else {
                "-".white().to_string()
            };

            let exit_display = if job.is_completed() {
                "0".green().to_string()
            } else if let Some(code) = job.exit_code.return_code.value() {
                if code != 0 {
                    format!("{}", code).red().to_string()
                } else {
                    "-".white().to_string()
                }
            } else if !job.exit_code.signal.name.is_empty() {
                job.exit_code.signal.name.clone().yellow().to_string()
            } else {
                "-".white().to_string()
            };

            HistoryRow {
                job_id: job.job_id.to_string(),
                name: truncate_string(&job.name, 20),
                partition: job.partition.clone(),
                state: colored_state,
                exit_code: exit_display,
                elapsed: job.elapsed_display(),
                wait: job.wait_time_display(),
                cpus: job.required.cpus.to_string(),
                cpu_eff,
                mem_eff,
            }
        })
        .collect();

    build_styled_table(rows, 200)
}

/// Format history job state with coloring
fn format_history_state(job: &JobHistoryInfo) -> String {
    let state_str = job.primary_state();

    if job.is_completed() {
        state_str.green().to_string()
    } else if job.is_running() {
        state_str.bright_green().to_string()
    } else if job.is_pending() {
        state_str.yellow().to_string()
    } else if job.is_failed() || job.is_out_of_memory() {
        state_str.red().to_string()
    } else if job.is_timeout() {
        state_str.bright_red().to_string()
    } else if job.is_cancelled() {
        state_str.magenta().to_string()
    } else {
        state_str.white().to_string()
    }
}

/// Format efficiency percentage with color (no decimal places)
fn format_efficiency(eff: f64) -> String {
    format_efficiency_with_precision(eff, 0)
}

/// Format efficiency percentage with color and specified precision
fn format_efficiency_with_precision(eff: f64, decimal_places: usize) -> String {
    let eff_str = match decimal_places {
        0 => format!("{:.0}%", eff),
        1 => format!("{:.1}%", eff),
        _ => format!("{:.2}%", eff),
    };
    if eff < 30.0 {
        eff_str.red().to_string()
    } else if eff < 70.0 {
        eff_str.yellow().to_string()
    } else {
        eff_str.green().to_string()
    }
}

/// Format efficiency with a label prefix (e.g., "CPU: 85.5%")
fn format_labeled_efficiency(label: &str, eff: Option<f64>) -> String {
    match eff {
        Some(e) => format!("{}: {}", label, format_efficiency_with_precision(e, 1)),
        None => format!("{}: -", label).white().to_string(),
    }
}

/// Create an efficiency bar
fn create_efficiency_bar(efficiency: f64) -> String {
    let bar_length: usize = 15;
    let filled = ((efficiency / 100.0) * bar_length as f64).round() as usize;
    let empty = bar_length.saturating_sub(filled);

    let filled_part = "█".repeat(filled);
    let empty_part = "░".repeat(empty);

    if efficiency < 30.0 {
        format!("{}{}", filled_part.red(), empty_part.white())
    } else if efficiency < 70.0 {
        format!("{}{}", filled_part.yellow(), empty_part.white())
    } else {
        format!("{}{}", filled_part.green(), empty_part.white())
    }
}

/// Wrap text smartly - handles both words and long paths without spaces
fn wrap_text_smart(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if word.len() <= width {
            // Normal word - fits in one line
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        } else {
            // Long word (like a path) - needs to be broken up
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }

            // Break the long word into chunks
            let mut remaining = word;
            while !remaining.is_empty() {
                if remaining.len() <= width {
                    current_line = remaining.to_string();
                    break;
                } else {
                    // Try to break at a path separator if possible
                    let break_at = find_break_point(remaining, width);
                    lines.push(remaining[..break_at].to_string());
                    remaining = &remaining[break_at..];
                }
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(text.to_string());
    }

    lines
}

/// Find a good break point for a long string (prefer breaking after / or - or _)
fn find_break_point(s: &str, max_width: usize) -> usize {
    let search_range = &s[..max_width];

    // Look for the last path separator, hyphen, or underscore within the allowed width
    // Check separators in order of preference: path > hyphen > underscore
    for separator in ['/', '-', '_'] {
        if let Some(pos) = search_range.rfind(separator)
            && pos > 0
        {
            return pos + 1; // Break after the separator
        }
    }

    // No good break point found, just break at max width
    max_width
}

/// Format problem nodes summary
pub fn format_problem_nodes(nodes: &[NodeInfo], show_all: bool, node_prefix_strip: &str) -> String {
    if nodes.is_empty() {
        let mut output = String::new();
        output.push_str(&format!("\n{}\n", box_top_colored(" Cluster Health ", BoxColor::Green)));
        output.push_str(&format!("{}\n", box_empty_colored(BoxColor::Green)));
        output.push_str(&format!(
            "{}\n",
            pad_line_colored("  All nodes are healthy! No issues detected.", BoxColor::Green)
        ));
        output.push_str(&format!("{}\n", box_empty_colored(BoxColor::Green)));
        output.push_str(&format!("{}\n", box_bottom_colored(BoxColor::Green)));
        return output;
    }

    let mut output = String::new();

    // Count nodes by state category
    let mut down_nodes: Vec<&NodeInfo> = Vec::new();
    let mut drain_nodes: Vec<&NodeInfo> = Vec::new();
    let mut maint_nodes: Vec<&NodeInfo> = Vec::new();
    let mut other_nodes: Vec<&NodeInfo> = Vec::new();

    for node in nodes {
        let states: Vec<String> = node
            .node_state
            .state
            .iter()
            .map(|s| s.to_uppercase())
            .collect();

        if states
            .iter()
            .any(|s| s.contains("DOWN") || s.contains("FAIL"))
        {
            down_nodes.push(node);
        } else if states.iter().any(|s| s.contains("DRAIN")) {
            drain_nodes.push(node);
        } else if states.iter().any(|s| s.contains("MAINT")) {
            maint_nodes.push(node);
        } else {
            other_nodes.push(node);
        }
    }

    // Summary header
    output.push_str(&format!("\n{}\n", box_top_colored(" Problem Nodes ", BoxColor::Red)));
    output.push_str(&format!("{}\n", box_empty_colored(BoxColor::Red)));

    // Summary counts
    let total_cpus: u32 = nodes.iter().map(|n| n.cpus.total).sum();
    let total_gpus: u32 = nodes.iter().map(|n| n.gpu_info().total).sum();

    let summary = format!(
        "  {} problem node(s) affecting {} CPUs{}",
        nodes.len().to_string().red().bold(),
        total_cpus.to_string().yellow(),
        if total_gpus > 0 {
            format!(" and {} GPUs", total_gpus.to_string().magenta())
        } else {
            String::new()
        }
    );
    output.push_str(&format!("{}\n", pad_line_colored(&summary, BoxColor::Red)));

    output.push_str(&format!("{}\n", box_empty_colored(BoxColor::Red)));

    // Breakdown by category
    let breakdown_header = format!("  {}", "Breakdown:".bold().underline());
    output.push_str(&format!("{}\n", pad_line_colored(&breakdown_header, BoxColor::Red)));

    if !down_nodes.is_empty() {
        let down_cpus: u32 = down_nodes.iter().map(|n| n.cpus.total).sum();
        let down_line = format!(
            "    {} DOWN/FAILED: {} nodes ({} CPUs)",
            "●".bright_red(),
            down_nodes.len().to_string().red(),
            down_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&down_line, BoxColor::Red)));
    }

    if !drain_nodes.is_empty() {
        let drain_cpus: u32 = drain_nodes.iter().map(|n| n.cpus.total).sum();
        let drain_line = format!(
            "    {} DRAINING: {} nodes ({} CPUs)",
            "◐".yellow(),
            drain_nodes.len().to_string().yellow(),
            drain_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&drain_line, BoxColor::Red)));
    }

    if !maint_nodes.is_empty() {
        let maint_cpus: u32 = maint_nodes.iter().map(|n| n.cpus.total).sum();
        let maint_line = format!(
            "    {} MAINTENANCE: {} nodes ({} CPUs)",
            "◐".cyan(),
            maint_nodes.len().to_string().cyan(),
            maint_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&maint_line, BoxColor::Red)));
    }

    if show_all && !other_nodes.is_empty() {
        let other_cpus: u32 = other_nodes.iter().map(|n| n.cpus.total).sum();
        let other_line = format!(
            "    {} OTHER: {} nodes ({} CPUs)",
            "○".bright_black(),
            other_nodes.len().to_string().bright_black(),
            other_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&other_line, BoxColor::Red)));
    }

    output.push_str(&format!("{}\n", box_empty_colored(BoxColor::Red)));
    output.push_str(&format!("{}\n", box_bottom_colored(BoxColor::Red)));

    // Detailed table
    output.push('\n');
    output.push_str(&format_problem_nodes_table(nodes, node_prefix_strip));

    output
}

/// Format problem nodes table with reason column
fn format_problem_nodes_table(nodes: &[NodeInfo], node_prefix_strip: &str) -> String {
    if nodes.is_empty() {
        return String::new();
    }

    #[derive(Tabled)]
    struct ProblemNodeRow {
        #[tabled(rename = "Node")]
        node: String,

        #[tabled(rename = "Partition")]
        partition: String,

        #[tabled(rename = "State")]
        state: String,

        #[tabled(rename = "CPUs")]
        cpus: String,

        #[tabled(rename = "GPUs")]
        gpus: String,

        #[tabled(rename = "Reason")]
        reason: String,
    }

    let rows: Vec<ProblemNodeRow> = nodes
        .iter()
        .map(|node| {
            let state_str = node.primary_state();
            let states = &node.node_state.state;

            // Color state based on severity
            let colored_state = if states
                .iter()
                .any(|s| s.contains("DOWN") || s.contains("FAIL"))
            {
                state_str.red().to_string()
            } else if states.iter().any(|s| s.contains("DRAIN")) {
                state_str.yellow().to_string()
            } else if states.iter().any(|s| s.contains("MAINT")) {
                state_str.cyan().to_string()
            } else {
                state_str.bright_black().to_string()
            };

            let gpu_info = node.gpu_info();
            let gpus = if gpu_info.total > 0 {
                format!("{}x{}", gpu_info.total, gpu_info.gpu_type.to_uppercase())
            } else {
                "-".to_string()
            };

            let reason = node.reason_description();
            let reason_display = if reason.is_empty() {
                "-".to_string()
            } else {
                truncate_string(reason, 30)
            };

            ProblemNodeRow {
                node: shorten_node_name(node.name(), node_prefix_strip).to_string(),
                partition: node.partition.name.clone().unwrap_or_default(),
                state: colored_state,
                cpus: node.cpus.total.to_string(),
                gpus,
                reason: reason_display,
            }
        })
        .collect();

    build_styled_table(rows, 120)
}

/// Pad line with colored borders
fn pad_line_colored(content: &str, color: BoxColor) -> String {
    let visible_len = strip_ansi(content).chars().count();
    let padding = if visible_len < BOX_WIDTH {
        " ".repeat(BOX_WIDTH - visible_len)
    } else {
        String::new()
    };

    format!(
        "{}{}{}{}",
        color.apply(box_chars::LEFT),
        content,
        padding,
        color.apply(box_chars::RIGHT)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_mb() {
        // Tests for format_bytes_mb (MB input -> human readable)
        assert_eq!(format_bytes_mb(512), "512M");
        assert_eq!(format_bytes_mb(1024), "1.0G");
        assert_eq!(format_bytes_mb(1536), "1.5G");
        assert_eq!(format_bytes_mb(1048576), "1.0T");
        assert_eq!(format_bytes_mb(1572864), "1.5T");
    }

    #[test]
    fn test_strip_ansi() {
        let plain = "Hello World";
        assert_eq!(strip_ansi(plain), "Hello World");

        let colored = "\x1b[31mRed\x1b[0m Text";
        assert_eq!(strip_ansi(colored), "Red Text");

        let complex = "\x1b[1;32mGreen Bold\x1b[0m Normal";
        assert_eq!(strip_ansi(complex), "Green Bold Normal");
    }

    #[test]
    fn test_pad_line() {
        let short = "  Test";
        let padded = pad_line(short);
        let visible = strip_ansi(&padded);
        // Should be padded to 78 chars + 2 border chars = 80
        assert_eq!(visible.chars().count(), 80);
        assert!(visible.starts_with("│"));
        assert!(visible.ends_with("│"));
    }
}
