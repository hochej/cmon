//! cmon - Fast cluster monitoring tool for Slurm

mod display;
mod models;
mod slurm;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use slurm::SlurmInterface;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    cursor::{Hide, Show},
};

#[derive(Parser)]
#[command(name = "cmon")]
#[command(about = "Fast cluster monitoring tool for Slurm", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show job information
    Jobs {
        /// Show all jobs (not just running)
        #[arg(short, long)]
        all: bool,

        /// Filter by user
        #[arg(short, long)]
        user: Option<String>,

        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Filter by job states (comma-separated, e.g. RUNNING,PENDING,FAILED)
        #[arg(long, value_name = "STATES")]
        state: Option<String>,

        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Show node information
    Nodes {
        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Filter by node list
        #[arg(short, long)]
        nodelist: Option<String>,

        /// Show all partitions (including hidden)
        #[arg(short, long)]
        all: bool,

        /// Filter by node states (comma-separated, e.g. IDLE,MIXED,DOWN)
        #[arg(long, value_name = "STATES")]
        state: Option<String>,

        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Show cluster status
    Status {
        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Filter by user
        #[arg(short, long)]
        user: Option<String>,

        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Show partition utilization
    #[command(alias = "part")]
    Partitions {
        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Filter by user
        #[arg(short, long)]
        user: Option<String>,

        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Show personal dashboard (your jobs and statistics)
    #[command(alias = "my")]
    Me {
        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Show detailed information for a specific job
    Job {
        /// Job ID to inspect
        job_id: u64,
    },

    /// Show job history
    History {
        /// Number of days to look back (default: 7)
        #[arg(short, long, default_value = "7")]
        days: u32,

        /// Filter by job states (comma-separated, e.g. COMPLETED,FAILED,TIMEOUT)
        #[arg(long, value_name = "STATES")]
        state: Option<String>,

        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Show all users' jobs (not just your own)
        #[arg(short, long)]
        all: bool,

        /// Maximum number of jobs to show
        #[arg(short = 'n', long, default_value = "50")]
        limit: usize,
    },

    /// Show problematic nodes (down, draining, failed, maintenance)
    #[command(alias = "issues")]
    Down {
        /// Filter by partition
        #[arg(short, long)]
        partition: Option<String>,

        /// Show all problem states (including reserved, powered down)
        #[arg(short, long)]
        all: bool,

        /// Watch mode: refresh every N seconds
        #[arg(short, long, value_name = "SECONDS", default_value = "0")]
        watch: f64,
    },

    /// Launch interactive TUI mode
    #[command(alias = "ui")]
    Tui,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let slurm = SlurmInterface::new();

    // Test Slurm connection
    if !slurm.test_connection() {
        eprintln!("Error: Unable to connect to Slurm. Make sure sinfo/squeue are available.");
        std::process::exit(1);
    }

    match cli.command {
        Some(Commands::Jobs { all, user, partition, state, watch }) => {
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_jobs_command(&slurm, all, user.as_deref(), partition.as_deref(), state.as_deref())
                })?;
            } else {
                let output = handle_jobs_command(&slurm, all, user.as_deref(), partition.as_deref(), state.as_deref())?;
                println!("{}", output);
            }
        }
        Some(Commands::Nodes { partition, nodelist, all, state, watch }) => {
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_nodes_command(&slurm, partition.as_deref(), nodelist.as_deref(), all, state.as_deref())
                })?;
            } else {
                let output = handle_nodes_command(&slurm, partition.as_deref(), nodelist.as_deref(), all, state.as_deref())?;
                println!("{}", output);
            }
        }
        Some(Commands::Status { partition, user, watch }) => {
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_status_command(&slurm, partition.as_deref(), user.as_deref())
                })?;
            } else {
                let output = handle_status_command(&slurm, partition.as_deref(), user.as_deref())?;
                println!("{}", output);
            }
        }
        Some(Commands::Partitions { partition, user, watch }) => {
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_partitions_command(&slurm, partition.as_deref(), user.as_deref())
                })?;
            } else {
                let output = handle_partitions_command(&slurm, partition.as_deref(), user.as_deref())?;
                println!("{}", output);
            }
        }
        Some(Commands::Me { watch }) => {
            let username = SlurmInterface::get_current_user();
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_me_command(&slurm, &username)
                })?;
            } else {
                let output = handle_me_command(&slurm, &username)?;
                println!("{}", output);
            }
        }
        Some(Commands::Job { job_id }) => {
            let output = handle_job_command(&slurm, job_id)?;
            println!("{}", output);
        }
        Some(Commands::History { days, state, partition, all, limit }) => {
            let output = handle_history_command(&slurm, days, state.as_deref(), partition.as_deref(), all, limit)?;
            println!("{}", output);
        }
        Some(Commands::Down { partition, all, watch }) => {
            if watch > 0.0 {
                watch_loop(watch, || {
                    handle_down_command(&slurm, partition.as_deref(), all)
                })?;
            } else {
                let output = handle_down_command(&slurm, partition.as_deref(), all)?;
                println!("{}", output);
            }
        }
        Some(Commands::Tui) => {
            tui::run()?;
        }
        None => {
            // Default: show status
            let output = handle_status_command(&slurm, None, None)?;
            println!("{}", output);
        }
    }

    Ok(())
}

/// Watch loop that repeatedly executes a command with flicker-free updates
fn watch_loop<F>(interval: f64, command: F) -> Result<()>
where
    F: Fn() -> Result<String>,
{
    // Set up Ctrl+C handler
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    // Enter alternate screen buffer and hide cursor for clean display
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;

    // Ensure we clean up on exit
    let cleanup = || -> Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, Show, LeaveAlternateScreen)?;
        Ok(())
    };

    let result = (|| -> Result<()> {
        while running.load(std::sync::atomic::Ordering::SeqCst) {
            // Get current timestamp
            let now = chrono::Local::now();
            let timestamp = now.format("%Y-%m-%d %H:%M:%S");

            // Execute the command and capture output
            let output = match command() {
                Ok(s) => s,
                Err(e) => format!("Error: {}", e),
            };

            // Build complete screen content in memory
            let screen_content = format!(
                "{}\n\nLast updated: {} | Refreshing every {}s | Press Ctrl+C to exit",
                output, timestamp, interval
            );

            // Write everything at once with synchronized update (DEC private mode)
            // This prevents the terminal from rendering until the full frame is written
            write!(stdout, "\x1B[?2026h")?;  // Begin synchronized update
            write!(stdout, "\x1B[H{}\x1B[J", screen_content)?;
            write!(stdout, "\x1B[?2026l")?;  // End synchronized update
            stdout.flush()?;

            // Sleep for the specified interval
            thread::sleep(Duration::from_secs_f64(interval));
        }
        Ok(())
    })();

    // Always clean up terminal state
    cleanup()?;

    // Print exit message on main screen
    println!("Watch mode stopped.");

    result
}

fn handle_jobs_command(
    slurm: &SlurmInterface,
    show_all: bool,
    user: Option<&str>,
    partition: Option<&str>,
    state_filter: Option<&str>,
) -> Result<String> {
    let users = user.map(|u| vec![u.to_string()]);
    let partitions = partition.map(|p| vec![p.to_string()]);

    let states = if let Some(state_str) = state_filter {
        // User provided explicit state filter
        Some(state_str.split(',').map(|s| s.trim().to_uppercase()).collect())
    } else if show_all {
        None
    } else {
        Some(vec!["RUNNING".to_string()])
    };

    let jobs = slurm.get_jobs(
        users.as_deref(),
        None,
        partitions.as_deref(),
        states.as_deref(),
        None,
    )?;

    Ok(display::format_jobs(&jobs, show_all || state_filter.is_some()))
}

fn handle_nodes_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    nodelist: Option<&str>,
    all: bool,
    state_filter: Option<&str>,
) -> Result<String> {
    // Get all nodes first
    let mut nodes = slurm.get_nodes(partition, nodelist, None, all)?;

    // Apply client-side filtering based on primary_state()
    if let Some(state_str) = state_filter {
        let allowed_states: Vec<String> = state_str
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .collect();

        nodes.retain(|node| {
            let primary = node.primary_state().to_uppercase();
            allowed_states.contains(&primary)
        });
    }

    Ok(display::format_nodes(&nodes))
}

fn handle_status_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    user: Option<&str>,
) -> Result<String> {
    let status = slurm.get_cluster_status(partition, user, None)?;

    let mut output = String::new();
    output.push_str(&display::format_cluster_status(&status));
    output.push_str("\n\n");
    output.push_str(&display::format_nodes(&status.nodes));

    Ok(output)
}

fn handle_partitions_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    user: Option<&str>,
) -> Result<String> {
    let status = slurm.get_cluster_status(partition, user, None)?;

    // Only show cluster status and partition utilization, no node table
    Ok(display::format_cluster_status(&status))
}

fn handle_me_command(
    slurm: &SlurmInterface,
    username: &str,
) -> Result<String> {
    let summary = slurm.get_personal_summary(username)?;
    Ok(display::format_personal_summary(&summary))
}

fn handle_job_command(
    slurm: &SlurmInterface,
    job_id: u64,
) -> Result<String> {
    let job = slurm.get_job_details(job_id)?;
    Ok(display::format_job_details(&job))
}

fn handle_history_command(
    slurm: &SlurmInterface,
    days: u32,
    state_filter: Option<&str>,
    partition: Option<&str>,
    all_users: bool,
    limit: usize,
) -> Result<String> {
    // Calculate start time
    let now = chrono::Utc::now();
    let start = now - chrono::Duration::days(days as i64);
    let start_time = start.format("%Y-%m-%dT%H:%M:%S").to_string();

    // Get current user if not showing all
    let username = if all_users {
        None
    } else {
        Some(SlurmInterface::get_current_user())
    };

    // Parse state filter
    let states: Option<Vec<String>> = state_filter.map(|s| {
        s.split(',')
            .map(|st| st.trim().to_uppercase())
            .collect()
    });

    let mut jobs = slurm.get_job_history(
        username.as_deref(),
        Some(&start_time),
        None,
        states.as_deref(),
        None,
        all_users,
    )?;

    // Filter by partition if specified
    if let Some(part) = partition {
        jobs.retain(|j| j.partition.eq_ignore_ascii_case(part));
    }

    // Sort by job_id descending (most recent first)
    jobs.sort_by(|a, b| b.job_id.cmp(&a.job_id));

    // Limit results
    jobs.truncate(limit);

    let mut output = String::new();

    // Header
    let user_info = if all_users {
        "all users".to_string()
    } else {
        format!("user {}", username.as_deref().unwrap_or("unknown"))
    };

    output.push_str(&format!(
        "\nJob History ({}, last {} days, {} jobs)\n\n",
        user_info,
        days,
        jobs.len()
    ));

    output.push_str(&display::format_job_history(&jobs, true));

    Ok(output)
}

fn handle_down_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    show_all: bool,
) -> Result<String> {
    // Get all nodes - only use --all flag when no partition filter
    let include_hidden = partition.is_none();
    let nodes = slurm.get_nodes(partition, None, None, include_hidden)?;

    // Filter to only problem nodes
    let problem_states = if show_all {
        vec![
            "DOWN", "DRAIN", "DRAINED", "DRAINING", "FAIL", "MAINT",
            "NOT_RESPONDING", "RESERVED", "POWERED_DOWN", "POWERING_DOWN",
            "REBOOT_REQUESTED", "REBOOT_ISSUED",
        ]
    } else {
        // Default: most critical states only
        vec!["DOWN", "DRAIN", "DRAINED", "DRAINING", "FAIL", "MAINT", "NOT_RESPONDING"]
    };

    let problem_nodes: Vec<_> = nodes
        .into_iter()
        .filter(|node| {
            let state = node.primary_state().to_uppercase();
            problem_states.iter().any(|s| state.contains(s))
                || node.node_state.state.iter().any(|s| {
                    let s_upper = s.to_uppercase();
                    problem_states.iter().any(|ps| s_upper.contains(ps))
                })
        })
        .collect();

    Ok(display::format_problem_nodes(&problem_nodes, show_all))
}