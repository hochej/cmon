//! Display and formatting functions for cluster information

use crate::models::{ClusterStatus, JobHistoryInfo, JobInfo, NodeInfo, PersonalSummary, format_duration_seconds};
use crate::slurm::{shorten_node_name, shorten_node_list};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tabled::{
    settings::{object::Rows, Alignment, Modify, Style, Width},
    Table, Tabled,
};

/// Format megabytes to human-readable size (input is in MB)
pub fn format_bytes(mb: u64) -> String {
    const KB: u64 = 1024;
    const GB: u64 = KB;
    const TB: u64 = GB * 1024;

    if mb >= TB {
        format!("{:.1}T", mb as f64 / TB as f64)
    } else if mb >= GB {
        format!("{:.1}G", mb as f64 / GB as f64)
    } else {
        format!("{}M", mb)
    }
}

/// Format node state with colored indicator
#[allow(clippy::if_same_then_else)]
pub fn format_node_state(node: &NodeInfo) -> String {
    let state_str = node.primary_state();

    // Choose symbol based on state category
    let symbol = if node.is_down() || node.is_fail() || node.is_failing() || node.is_inval() {
        "○".bright_red().to_string()  // Empty circle for problem states
    } else if node.is_drained() || node.is_draining() || node.is_maint() {
        "◐".yellow().to_string()  // Half-filled for maintenance
    } else if node.is_powered_down() || node.is_powering_down() || node.is_power_down() {
        "○".bright_black().to_string()  // Empty for powered off
    } else if node.is_idle() {
        "●".green().to_string()  // Filled green for available
    } else if node.is_mixed() || node.is_allocated() {
        "●".yellow().to_string()  // Filled yellow for in-use
    } else if node.is_completing() {
        "◑".bright_yellow().to_string()  // Transitional
    } else {
        "●".white().to_string()  // Default filled
    };

    // Color the state text
    let colored_state = if node.is_down() || node.is_fail() || node.is_failing() || node.is_inval() {
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

    let usage = format!("{}/{}", format_bytes(used), format_bytes(total));

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

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(200).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    table.to_string()
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
        || job.is_deadline() {
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
            let cpus = job.cpus_per_task.number * job.tasks.number;
            let gpu_info = job.gpu_type_info();

            JobRow {
                job_id: job.job_id.to_string(),
                name: job.name.clone(),
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

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(200).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    table.to_string()
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

    output.push_str(&format!("\n{}\n", "╭─────────────────────────────── Cluster Status ───────────────────────────────╮".blue()));
    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    let header = format!("  {} (as of {})", "Cluster Overview".bold(), chrono::Local::now().format("%H:%M:%S"));
    output.push_str(&format!("{}\n", pad_line(&header)));

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

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

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));
    output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".blue()));

    // Partition stats
    output.push_str(&format!("\n{}\n", "╭─────────────────────────── Partition Utilization ────────────────────────────╮".blue()));
    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    output.push_str(&format_partition_stats(status, partition_order));

    output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".blue()));

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
    const BOX_WIDTH: usize = 78;
    let visible_len = strip_ansi(content).chars().count();
    let padding = if visible_len < BOX_WIDTH {
        " ".repeat(BOX_WIDTH - visible_len)
    } else {
        String::new()
    };

    format!("{}{}{}", "│".blue(), content, padding + &"│".blue().to_string())
}

/// Display partition statistics
///
/// Groups nodes by their actual Slurm partition field (portable across clusters).
/// Partition order can be configured; defaults to alphabetical.
fn format_partition_stats(status: &ClusterStatus, partition_order: &[String]) -> String {
    let mut output = String::new();
    let mut partitions: HashMap<String, Vec<&NodeInfo>> = HashMap::new();

    // Group nodes by their actual partition name from Slurm
    for node in &status.nodes {
        let partition_name = node.partition.name.clone().unwrap_or_else(|| "unknown".to_string());
        partitions.entry(partition_name).or_default().push(node);
    }

    // Determine display order: configured order first, then remaining alphabetically
    let mut ordered_names: Vec<String> = Vec::new();

    // Add configured partitions in order (if they exist)
    for name in partition_order {
        let name_lower = name.to_lowercase();
        if partitions.contains_key(&name_lower) {
            ordered_names.push(name_lower);
        }
    }

    // Add remaining partitions alphabetically
    let mut remaining: Vec<&String> = partitions.keys()
        .filter(|k| !ordered_names.contains(k))
        .collect();
    remaining.sort();
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

        // Use partition name with first letter capitalized for display
        let display_name = capitalize_first(&partition_name);
        let header = format!("  {} ({} nodes):", display_name.bold(), node_count);
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
            mem_bar, mem_util, format_bytes(allocated_mem), format_bytes(total_mem)
        );
        output.push_str(&format!("{}\n", pad_line(&mem_line)));

        // GPU stats (shown only if partition has GPUs)
        let total_gpus: u32 = nodes.iter().map(|n| n.gpu_info().total).sum();
        if total_gpus > 0 {
            let used_gpus: u32 = nodes.iter().map(|n| n.gpu_info().used).sum();
            let gpu_util = if total_gpus > 0 {
                (used_gpus as f64 / total_gpus as f64) * 100.0
            } else {
                0.0
            };

            let gpu_type = nodes
                .iter()
                .find_map(|n| {
                    let info = n.gpu_info();
                    if !info.gpu_type.is_empty() {
                        Some(info.gpu_type.to_uppercase())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "GPU".to_string());

            let gpu_bar = create_bar(gpu_util);
            let gpu_line = format!(
                "    GPUs:    {}   {:.0}% ({}/{} {})",
                gpu_bar, gpu_util, used_gpus, total_gpus, gpu_type
            );
            output.push_str(&format!("{}\n", pad_line(&gpu_line)));
        }

        output.push_str(&format!("{}\n", "│                                                                              │".blue()));
    }

    output
}

/// Capitalize the first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
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
    output.push_str(&format!("\n{}\n", "╭────────────────────────────────── My Dashboard ──────────────────────────────╮".blue()));
    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    let header = format!("  {} {} (as of {})",
        "User:".bold(),
        summary.username.cyan(),
        chrono::Local::now().format("%H:%M:%S")
    );
    output.push_str(&format!("{}\n", pad_line(&header)));

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Current Jobs Section
    let jobs_header = format!("  {}", "Current Jobs".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&jobs_header)));

    let running_str = if summary.running_jobs > 0 {
        format!("{} running", summary.running_jobs).green().to_string()
    } else {
        "0 running".white().to_string()
    };

    let pending_str = if summary.pending_jobs > 0 {
        format!("{} pending", summary.pending_jobs).yellow().to_string()
    } else {
        "0 pending".white().to_string()
    };

    let jobs_line = format!("    {} | {}", running_str, pending_str);
    output.push_str(&format!("{}\n", pad_line(&jobs_line)));

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Last 24 Hours Section
    let history_header = format!("  {}", "Last 24 Hours".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&history_header)));

    let completed_str = format!("{} completed", summary.completed_24h).green().to_string();
    let failed_str = if summary.failed_24h > 0 {
        format!("{} failed", summary.failed_24h).red().to_string()
    } else {
        "0 failed".white().to_string()
    };
    let timeout_str = if summary.timeout_24h > 0 {
        format!("{} timeout", summary.timeout_24h).yellow().to_string()
    } else {
        "0 timeout".white().to_string()
    };
    let cancelled_str = if summary.cancelled_24h > 0 {
        format!("{} cancelled", summary.cancelled_24h).magenta().to_string()
    } else {
        "0 cancelled".white().to_string()
    };

    let history_line = format!("    {} | {} | {} | {}",
        completed_str, failed_str, timeout_str, cancelled_str);
    output.push_str(&format!("{}\n", pad_line(&history_line)));

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

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

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Efficiency Section
    let efficiency_header = format!("  {}", "Efficiency Metrics".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&efficiency_header)));

    let cpu_eff = match summary.avg_cpu_efficiency {
        Some(eff) => {
            let eff_str = format!("{:.1}%", eff);
            if eff < 30.0 {
                format!("CPU: {}", eff_str.red())
            } else if eff < 70.0 {
                format!("CPU: {}", eff_str.yellow())
            } else {
                format!("CPU: {}", eff_str.green())
            }
        }
        None => "CPU: -".white().to_string(),
    };

    let mem_eff = match summary.avg_memory_efficiency {
        Some(eff) => {
            let eff_str = format!("{:.1}%", eff);
            if eff < 30.0 {
                format!("Memory: {}", eff_str.red())
            } else if eff < 70.0 {
                format!("Memory: {}", eff_str.yellow())
            } else {
                format!("Memory: {}", eff_str.green())
            }
        }
        None => "Memory: -".white().to_string(),
    };

    let wait_time = match summary.avg_wait_time_seconds {
        Some(secs) => format!("Avg Wait: {}", format_duration_seconds(secs)),
        None => "Avg Wait: -".to_string(),
    };

    let efficiency_line = format!("    {} | {} | {}", cpu_eff, mem_eff, wait_time);
    output.push_str(&format!("{}\n", pad_line(&efficiency_line)));

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));
    output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".blue()));

    // Current jobs table (if any)
    if !summary.current_jobs.is_empty() {
        output.push_str("\n");
        output.push_str(&format!("{}\n", "Current Jobs:".bold()));
        output.push_str(&format_jobs(&summary.current_jobs, true, node_prefix_strip));
    }

    // Recent jobs table (limit to last 10)
    let recent: Vec<&JobHistoryInfo> = summary.recent_jobs
        .iter()
        .filter(|j| !j.is_running() && !j.is_pending())
        .take(10)
        .collect();

    if !recent.is_empty() {
        output.push_str("\n");
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

            let cpu_eff = job.cpu_efficiency()
                .map(|e| format_efficiency(e))
                .unwrap_or_else(|| "-".white().to_string());

            let mem_eff = job.memory_efficiency()
                .map(|e| format_efficiency(e))
                .unwrap_or_else(|| "-".white().to_string());

            let exit_display = if job.is_completed() {
                "0".green().to_string()
            } else if job.exit_code.return_code.set && job.exit_code.return_code.number != 0 {
                format!("{}", job.exit_code.return_code.number).red().to_string()
            } else if !job.exit_code.signal.name.is_empty() {
                format!("SIG{}", job.exit_code.signal.name).yellow().to_string()
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

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(200).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    table.to_string()
}

/// Format a single job's detailed information
pub fn format_job_details(job: &JobHistoryInfo, node_prefix_strip: &str) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("\n{}\n", "╭──────────────────────────────────── Job Details ─────────────────────────────╮".blue()));
    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Basic Info
    let job_id_line = format!("  {} {}", "Job ID:".bold(), job.job_id.to_string().cyan());
    output.push_str(&format!("{}\n", pad_line(&job_id_line)));

    let name_line = format!("  {} {}", "Name:".bold(), job.name);
    output.push_str(&format!("{}\n", pad_line(&name_line)));

    let state_display = format_history_state(job);
    let state_line = format!("  {} {}", "State:".bold(), state_display);
    output.push_str(&format!("{}\n", pad_line(&state_line)));

    let user_line = format!("  {} {} ({})", "User:".bold(), job.user, job.account);
    output.push_str(&format!("{}\n", pad_line(&user_line)));

    let partition_line = format!("  {} {} (QoS: {})", "Partition:".bold(), job.partition, job.qos);
    output.push_str(&format!("{}\n", pad_line(&partition_line)));

    if !job.nodes.is_empty() && job.nodes != "None assigned" {
        let nodes_line = format!("  {} {}", "Nodes:".bold(), shorten_node_list(&job.nodes, node_prefix_strip));
        output.push_str(&format!("{}\n", pad_line(&nodes_line)));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Time Information
    let time_header = format!("  {}", "Time Information".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&time_header)));

    let submit_line = format!("    Submitted:  {}", job.submit_time_display());
    output.push_str(&format!("{}\n", pad_line(&submit_line)));

    if job.time.start > 0 {
        let start_line = format!("    Started:    {}", job.start_time_display());
        output.push_str(&format!("{}\n", pad_line(&start_line)));
    }

    if job.time.end > 0 {
        let end_line = format!("    Ended:      {}", job.end_time_display());
        output.push_str(&format!("{}\n", pad_line(&end_line)));
    }

    let elapsed_line = format!("    Elapsed:    {} / {}", job.elapsed_display(), job.time_limit_display());
    output.push_str(&format!("{}\n", pad_line(&elapsed_line)));

    if let Some(wait) = job.wait_time() {
        let wait_line = format!("    Wait Time:  {}", format_duration_seconds(wait));
        output.push_str(&format!("{}\n", pad_line(&wait_line)));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Resource Allocation
    let resource_header = format!("  {}", "Resource Allocation".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&resource_header)));

    let cpu_line = format!("    CPUs:       {}", job.required.cpus);
    output.push_str(&format!("{}\n", pad_line(&cpu_line)));

    let requested_mem = job.requested_memory();
    if requested_mem > 0 {
        let mem_line = format!("    Memory:     {}", format_bytes_from_bytes(requested_mem));
        output.push_str(&format!("{}\n", pad_line(&mem_line)));
    }

    let gpus = job.allocated_gpus();
    if gpus > 0 {
        let gpu_type = job.gpu_type().unwrap_or_else(|| "GPU".to_string());
        let gpu_line = format!("    GPUs:       {}x {}", gpus, gpu_type);
        output.push_str(&format!("{}\n", pad_line(&gpu_line)));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Efficiency Metrics
    let efficiency_header = format!("  {}", "Efficiency Metrics".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&efficiency_header)));

    // CPU Efficiency with bar
    if let Some(cpu_eff) = job.cpu_efficiency() {
        let bar = create_efficiency_bar(cpu_eff);
        let cpu_eff_line = format!("    CPU:        {} {:.1}%", bar, cpu_eff);
        output.push_str(&format!("{}\n", pad_line(&cpu_eff_line)));
    } else {
        let cpu_eff_line = "    CPU:        (no data)".to_string();
        output.push_str(&format!("{}\n", pad_line(&cpu_eff_line)));
    }

    // Memory Efficiency with bar
    let max_mem = job.max_memory_used();
    if max_mem > 0 && requested_mem > 0 {
        if let Some(mem_eff) = job.memory_efficiency() {
            let bar = create_efficiency_bar(mem_eff);
            let mem_eff_line = format!("    Memory:     {} {:.1}% ({} / {})",
                bar, mem_eff,
                format_bytes_from_bytes(max_mem),
                format_bytes_from_bytes(requested_mem)
            );
            output.push_str(&format!("{}\n", pad_line(&mem_eff_line)));
        }
    } else {
        let mem_eff_line = "    Memory:     (no data)".to_string();
        output.push_str(&format!("{}\n", pad_line(&mem_eff_line)));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Exit Information
    if !job.is_running() && !job.is_pending() {
        let exit_header = format!("  {}", "Exit Information".bold().underline());
        output.push_str(&format!("{}\n", pad_line(&exit_header)));

        let exit_code_display = if job.exit_code.return_code.set {
            let code = job.exit_code.return_code.number;
            if code == 0 {
                "0 (Success)".green().to_string()
            } else {
                format!("{} (Error)", code).red().to_string()
            }
        } else {
            "-".white().to_string()
        };
        let exit_line = format!("    Exit Code:  {}", exit_code_display);
        output.push_str(&format!("{}\n", pad_line(&exit_line)));

        if !job.exit_code.signal.name.is_empty() {
            let signal_line = format!("    Signal:     {}", job.exit_code.signal.name.red());
            output.push_str(&format!("{}\n", pad_line(&signal_line)));
        }

        output.push_str(&format!("{}\n", "│                                                                              │".blue()));
    }

    // Paths
    let paths_header = format!("  {}", "Paths".bold().underline());
    output.push_str(&format!("{}\n", pad_line(&paths_header)));

    if !job.working_directory.is_empty() {
        let wd_line = format!("    Work Dir:   {}", truncate_path(&job.working_directory, 50));
        output.push_str(&format!("{}\n", pad_line(&wd_line)));
    }

    if !job.stdout.is_empty() {
        let stdout_line = format!("    Stdout:     {}", truncate_path(&job.stdout, 50));
        output.push_str(&format!("{}\n", pad_line(&stdout_line)));
    }

    if !job.stderr.is_empty() && job.stderr != job.stdout {
        let stderr_line = format!("    Stderr:     {}", truncate_path(&job.stderr, 50));
        output.push_str(&format!("{}\n", pad_line(&stderr_line)));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".blue()));

    // Submit Line
    if !job.submit_line.is_empty() {
        let submit_header = format!("  {}", "Submit Command".bold().underline());
        output.push_str(&format!("{}\n", pad_line(&submit_header)));

        // Wrap long submit lines - 74 chars max (78 box width - 4 indent)
        let submit_wrapped = wrap_text_smart(&job.submit_line, 72);
        for line in submit_wrapped {
            let submit_line = format!("    {}", line.bright_black());
            output.push_str(&format!("{}\n", pad_line(&submit_line)));
        }

        output.push_str(&format!("{}\n", "│                                                                              │".blue()));
    }

    output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".blue()));

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
                    .map(|e| format_efficiency(e))
                    .unwrap_or_else(|| "-".white().to_string())
            } else {
                "-".white().to_string()
            };

            let mem_eff = if show_efficiency {
                job.memory_efficiency()
                    .map(|e| format_efficiency(e))
                    .unwrap_or_else(|| "-".white().to_string())
            } else {
                "-".white().to_string()
            };

            let exit_display = if job.is_completed() {
                "0".green().to_string()
            } else if job.exit_code.return_code.set && job.exit_code.return_code.number != 0 {
                format!("{}", job.exit_code.return_code.number).red().to_string()
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

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(200).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    table.to_string()
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

/// Format efficiency percentage with color
fn format_efficiency(eff: f64) -> String {
    let eff_str = format!("{:.0}%", eff);
    if eff < 30.0 {
        eff_str.red().to_string()
    } else if eff < 70.0 {
        eff_str.yellow().to_string()
    } else {
        eff_str.green().to_string()
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

/// Truncate a string to a maximum length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Truncate a path, keeping the end visible
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len().saturating_sub(max_len - 3)..])
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

/// Find a good break point for a long string (prefer breaking after / or -)
fn find_break_point(s: &str, max_width: usize) -> usize {
    let search_range = &s[..max_width];

    // Look for the last path separator or hyphen within the allowed width
    if let Some(pos) = search_range.rfind('/') {
        if pos > 0 {
            return pos + 1; // Break after the separator
        }
    }
    if let Some(pos) = search_range.rfind('-') {
        if pos > 0 {
            return pos + 1;
        }
    }
    if let Some(pos) = search_range.rfind('_') {
        if pos > 0 {
            return pos + 1;
        }
    }

    // No good break point found, just break at max width
    max_width
}

/// Format problem nodes summary
pub fn format_problem_nodes(nodes: &[NodeInfo], show_all: bool, node_prefix_strip: &str) -> String {
    if nodes.is_empty() {
        let mut output = String::new();
        output.push_str(&format!("\n{}\n", "╭───────────────────────────── Cluster Health ─────────────────────────────────╮".green()));
        output.push_str(&format!("{}\n", "│                                                                              │".green()));
        output.push_str(&format!("{}\n", pad_line_colored("  All nodes are healthy! No issues detected.", "green")));
        output.push_str(&format!("{}\n", "│                                                                              │".green()));
        output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".green()));
        return output;
    }

    let mut output = String::new();

    // Count nodes by state category
    let mut down_nodes: Vec<&NodeInfo> = Vec::new();
    let mut drain_nodes: Vec<&NodeInfo> = Vec::new();
    let mut maint_nodes: Vec<&NodeInfo> = Vec::new();
    let mut other_nodes: Vec<&NodeInfo> = Vec::new();

    for node in nodes {
        let states: Vec<String> = node.node_state.state.iter().map(|s| s.to_uppercase()).collect();

        if states.iter().any(|s| s.contains("DOWN") || s.contains("FAIL")) {
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
    output.push_str(&format!("\n{}\n", "╭───────────────────────────── Problem Nodes ──────────────────────────────────╮".red()));
    output.push_str(&format!("{}\n", "│                                                                              │".red()));

    // Summary counts
    let total_cpus: u32 = nodes.iter().map(|n| n.cpus.total).sum();
    let total_gpus: u32 = nodes.iter().map(|n| n.gpu_info().total).sum();

    let summary = format!("  {} problem node(s) affecting {} CPUs{}",
        nodes.len().to_string().red().bold(),
        total_cpus.to_string().yellow(),
        if total_gpus > 0 { format!(" and {} GPUs", total_gpus.to_string().magenta()) } else { String::new() }
    );
    output.push_str(&format!("{}\n", pad_line_colored(&summary, "red")));

    output.push_str(&format!("{}\n", "│                                                                              │".red()));

    // Breakdown by category
    let breakdown_header = format!("  {}", "Breakdown:".bold().underline());
    output.push_str(&format!("{}\n", pad_line_colored(&breakdown_header, "red")));

    if !down_nodes.is_empty() {
        let down_cpus: u32 = down_nodes.iter().map(|n| n.cpus.total).sum();
        let down_line = format!("    {} DOWN/FAILED: {} nodes ({} CPUs)",
            "●".bright_red(),
            down_nodes.len().to_string().red(),
            down_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&down_line, "red")));
    }

    if !drain_nodes.is_empty() {
        let drain_cpus: u32 = drain_nodes.iter().map(|n| n.cpus.total).sum();
        let drain_line = format!("    {} DRAINING: {} nodes ({} CPUs)",
            "◐".yellow(),
            drain_nodes.len().to_string().yellow(),
            drain_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&drain_line, "red")));
    }

    if !maint_nodes.is_empty() {
        let maint_cpus: u32 = maint_nodes.iter().map(|n| n.cpus.total).sum();
        let maint_line = format!("    {} MAINTENANCE: {} nodes ({} CPUs)",
            "◐".cyan(),
            maint_nodes.len().to_string().cyan(),
            maint_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&maint_line, "red")));
    }

    if show_all && !other_nodes.is_empty() {
        let other_cpus: u32 = other_nodes.iter().map(|n| n.cpus.total).sum();
        let other_line = format!("    {} OTHER: {} nodes ({} CPUs)",
            "○".bright_black(),
            other_nodes.len().to_string().bright_black(),
            other_cpus
        );
        output.push_str(&format!("{}\n", pad_line_colored(&other_line, "red")));
    }

    output.push_str(&format!("{}\n", "│                                                                              │".red()));
    output.push_str(&format!("{}\n", "╰──────────────────────────────────────────────────────────────────────────────╯".red()));

    // Detailed table
    output.push_str("\n");
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
            let colored_state = if states.iter().any(|s| s.contains("DOWN") || s.contains("FAIL")) {
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

    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(120).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));

    table.to_string()
}

/// Pad line with colored borders
fn pad_line_colored(content: &str, color: &str) -> String {
    const BOX_WIDTH: usize = 78;
    let visible_len = strip_ansi(content).chars().count();
    let padding = if visible_len < BOX_WIDTH {
        " ".repeat(BOX_WIDTH - visible_len)
    } else {
        String::new()
    };

    match color {
        "green" => format!("{}{}{}{}", "│".green(), content, padding, "│".green()),
        "red" => format!("{}{}{}{}", "│".red(), content, padding, "│".red()),
        _ => format!("{}{}{}{}", "│".blue(), content, padding, "│".blue()),
    }
}

/// Format bytes from bytes (not MB) to human-readable
fn format_bytes_from_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512M");
        assert_eq!(format_bytes(1024), "1.0G");
        assert_eq!(format_bytes(1536), "1.5G");
        assert_eq!(format_bytes(1048576), "1.0T");
        assert_eq!(format_bytes(1572864), "1.5T");
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