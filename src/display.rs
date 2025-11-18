//! Display and formatting functions for cluster information

use crate::models::{ClusterStatus, JobInfo, NodeInfo};
use crate::slurm::shorten_node_name;
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
pub fn format_nodes(nodes: &[NodeInfo]) -> String {
    if nodes.is_empty() {
        return "No nodes found".yellow().to_string();
    }

    let rows: Vec<NodeRow> = nodes
        .iter()
        .map(|node| NodeRow {
            node: shorten_node_name(node.name()).to_string(),
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

pub fn format_jobs(jobs: &[JobInfo], show_all: bool) -> String {
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
                nodes: crate::slurm::shorten_node_list(&job.nodes),
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

pub fn format_cluster_status(status: &ClusterStatus) -> String {
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

    output.push_str(&format_partition_stats(status));

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
fn format_partition_stats(status: &ClusterStatus) -> String {
    let mut output = String::new();
    let mut partitions: HashMap<&str, Vec<&NodeInfo>> = HashMap::new();
    partitions.insert("CPU Nodes", vec![]);
    partitions.insert("Fat Nodes", vec![]);
    partitions.insert("GPU Nodes", vec![]);
    partitions.insert("VDI Nodes", vec![]);

    // Group nodes by type
    for node in &status.nodes {
        let name = node.name();
        if name.starts_with("demu4xcpu") {
            partitions.get_mut("CPU Nodes").unwrap().push(node);
        } else if name.starts_with("demu4xfat") {
            partitions.get_mut("Fat Nodes").unwrap().push(node);
        } else if name.starts_with("demu4xgpu") {
            partitions.get_mut("GPU Nodes").unwrap().push(node);
        } else if name.starts_with("demu4xvdi") {
            partitions.get_mut("VDI Nodes").unwrap().push(node);
        }
    }

    // Display each partition in a consistent order
    let partition_order = ["CPU Nodes", "GPU Nodes", "Fat Nodes", "VDI Nodes"];
    for name in partition_order {
        let nodes = &partitions[name];
        if nodes.is_empty() {
            continue;
        }

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

        let header = format!("  {} ({}):", name.bold(), node_count);
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

        // GPU stats for GPU/Fat partitions
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