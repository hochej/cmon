//! cmon - Fast cluster monitoring tool for Slurm

mod display;
pub mod formatting;
mod models;
mod slurm;
mod tui;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use slurm::{SlurmInterface, check_slurm_json_support_with_warnings};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

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

    /// Generate a template configuration file
    InitConfig {
        /// Overwrite existing config file
        #[arg(short, long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle init-config early (before Slurm connection check)
    if let Some(Commands::InitConfig { force }) = &cli.command {
        return generate_default_config(*force);
    }

    // Load config first (for display settings and slurm path)
    let (config, config_warnings) = models::TuiConfig::load();

    // Print config warnings for CLI mode (TUI mode displays them in status bar)
    for warning in &config_warnings {
        eprintln!("Warning: {}", warning);
    }

    // Create SlurmInterface with config path (auto-detects if not specified)
    let slurm = SlurmInterface::with_config(config.system.slurm_bin_path.as_deref());

    // Warn if using fallback path
    if slurm.is_fallback_path() {
        eprintln!(
            "Warning: Could not find Slurm binaries in PATH, using fallback path: {}",
            slurm.slurm_bin_path.display()
        );
    }

    // Test Slurm connection
    if let Err(err) = slurm.test_connection() {
        eprintln!("Error: Unable to connect to Slurm: {err}");
        eprintln!("Searched path: {}", slurm.slurm_bin_path.display());
        std::process::exit(1);
    }

    // Check Slurm version and fail if JSON output is not available
    if !check_slurm_json_support_with_warnings(&slurm.slurm_bin_path) {
        eprintln!("Error: This tool requires Slurm 21.08 or later for JSON output support.");
        std::process::exit(1);
    }

    // Extract config values for display functions
    let node_prefix = &config.display.node_prefix_strip;
    let partition_order = &config.display.partition_order;

    match cli.command {
        Some(Commands::Jobs {
            all,
            user,
            partition,
            state,
            watch,
        }) => {
            let prefix = node_prefix.clone();
            run_with_optional_watch(watch, move || {
                handle_jobs_command(
                    &slurm,
                    all,
                    user.as_deref(),
                    partition.as_deref(),
                    state.as_deref(),
                    &prefix,
                )
            })?;
        }
        Some(Commands::Nodes {
            partition,
            nodelist,
            all,
            state,
            watch,
        }) => {
            let prefix = node_prefix.clone();
            run_with_optional_watch(watch, move || {
                handle_nodes_command(
                    &slurm,
                    partition.as_deref(),
                    nodelist.as_deref(),
                    all,
                    state.as_deref(),
                    &prefix,
                )
            })?;
        }
        Some(Commands::Status {
            partition,
            user,
            watch,
        }) => {
            let prefix = node_prefix.clone();
            let order = partition_order.clone();
            run_with_optional_watch(watch, move || {
                handle_status_command(
                    &slurm,
                    partition.as_deref(),
                    user.as_deref(),
                    &order,
                    &prefix,
                )
            })?;
        }
        Some(Commands::Partitions {
            partition,
            user,
            watch,
        }) => {
            let order = partition_order.clone();
            run_with_optional_watch(watch, move || {
                handle_partitions_command(&slurm, partition.as_deref(), user.as_deref(), &order)
            })?;
        }
        Some(Commands::Me { watch }) => {
            let username = SlurmInterface::get_current_user();
            let prefix = node_prefix.clone();
            run_with_optional_watch(watch, move || {
                handle_me_command(&slurm, &username, &prefix)
            })?;
        }
        Some(Commands::Job { job_id }) => {
            let output = handle_job_command(&slurm, job_id, node_prefix)?;
            println!("{}", output);
        }
        Some(Commands::History {
            days,
            state,
            partition,
            all,
            limit,
        }) => {
            let output = handle_history_command(
                &slurm,
                days,
                state.as_deref(),
                partition.as_deref(),
                all,
                limit,
            )?;
            println!("{}", output);
        }
        Some(Commands::Down {
            partition,
            all,
            watch,
        }) => {
            let prefix = node_prefix.clone();
            run_with_optional_watch(watch, move || {
                handle_down_command(&slurm, partition.as_deref(), all, &prefix)
            })?;
        }
        Some(Commands::Tui) => {
            tui::run()?;
        }
        Some(Commands::InitConfig { .. }) => {
            unreachable!("InitConfig should be handled before this point")
        }
        None => {
            // Default: show status
            let output = handle_status_command(&slurm, None, None, partition_order, node_prefix)?;
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
    })
    .expect("Error setting Ctrl-C handler");

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
            write!(stdout, "\x1B[?2026h")?; // Begin synchronized update
            write!(stdout, "\x1B[H{}\x1B[J", screen_content)?;
            write!(stdout, "\x1B[?2026l")?; // End synchronized update
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

/// Execute a command once or in watch mode with optional interval.
///
/// If `watch` is > 0.0, runs the command repeatedly in a loop with flicker-free updates.
/// Otherwise, runs the command once and prints the result.
fn run_with_optional_watch<F>(watch: f64, render_fn: F) -> Result<()>
where
    F: Fn() -> Result<String>,
{
    if watch > 0.0 {
        watch_loop(watch, render_fn)
    } else {
        println!("{}", render_fn()?);
        Ok(())
    }
}

/// Generate a template configuration file at ~/.config/cmon/config.toml
fn generate_default_config(force: bool) -> Result<()> {
    let config_path = get_user_config_path()?;

    // Check if file already exists
    if config_path.exists() && !force {
        bail!(
            "Config file already exists at {}\nUse --force to overwrite.",
            config_path.display()
        );
    }

    // Create parent directories if needed
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Generate the template config content
    let config_content = generate_config_template();

    // Write the config file
    fs::write(&config_path, config_content)?;

    println!("Created config file at {}", config_path.display());
    println!();
    println!("Edit this file to customize cmon behavior.");
    println!("All settings are optional - delete any you don't need.");

    Ok(())
}

/// Get the user config file path (uses shared logic from TuiConfig)
fn get_user_config_path() -> Result<PathBuf> {
    models::TuiConfig::user_config_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory. Set HOME or XDG_CONFIG_HOME environment variable."))
}

/// Generate a well-documented template configuration file
fn generate_config_template() -> &'static str {
    include_str!("config_template.toml")
}

fn handle_jobs_command(
    slurm: &SlurmInterface,
    show_all: bool,
    user: Option<&str>,
    partition: Option<&str>,
    state_filter: Option<&str>,
    node_prefix_strip: &str,
) -> Result<String> {
    let users = user.map(|u| vec![u.to_string()]);
    let partitions = partition.map(|p| vec![p.to_string()]);

    let states = if let Some(state_str) = state_filter {
        // User provided explicit state filter
        Some(
            state_str
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .collect(),
        )
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

    Ok(display::format_jobs(
        &jobs,
        show_all || state_filter.is_some(),
        node_prefix_strip,
    ))
}

fn handle_nodes_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    nodelist: Option<&str>,
    all: bool,
    state_filter: Option<&str>,
    node_prefix_strip: &str,
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

    Ok(display::format_nodes(&nodes, node_prefix_strip))
}

fn handle_status_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    user: Option<&str>,
    partition_order: &[String],
    node_prefix_strip: &str,
) -> Result<String> {
    let status = slurm.get_cluster_status(partition, user, None)?;

    let mut output = String::new();
    output.push_str(&display::format_cluster_status(&status, partition_order));
    output.push_str("\n\n");
    output.push_str(&display::format_nodes(&status.nodes, node_prefix_strip));

    Ok(output)
}

fn handle_partitions_command(
    slurm: &SlurmInterface,
    partition: Option<&str>,
    user: Option<&str>,
    partition_order: &[String],
) -> Result<String> {
    let status = slurm.get_cluster_status(partition, user, None)?;

    // Only show cluster status and partition utilization, no node table
    Ok(display::format_cluster_status(&status, partition_order))
}

fn handle_me_command(
    slurm: &SlurmInterface,
    username: &str,
    node_prefix_strip: &str,
) -> Result<String> {
    let summary = slurm.get_personal_summary(username)?;
    Ok(display::format_personal_summary(
        &summary,
        node_prefix_strip,
    ))
}

fn handle_job_command(
    slurm: &SlurmInterface,
    job_id: u64,
    node_prefix_strip: &str,
) -> Result<String> {
    let job = slurm.get_job_details(job_id)?;
    Ok(display::format_job_details(&job, node_prefix_strip))
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
    let states: Option<Vec<String>> =
        state_filter.map(|s| s.split(',').map(|st| st.trim().to_uppercase()).collect());

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
    node_prefix_strip: &str,
) -> Result<String> {
    // Get all nodes - only use --all flag when no partition filter
    let include_hidden = partition.is_none();
    let nodes = slurm.get_nodes(partition, None, None, include_hidden)?;

    // Filter to only problem nodes
    let problem_states = if show_all {
        vec![
            "DOWN",
            "DRAIN",
            "DRAINED",
            "DRAINING",
            "FAIL",
            "MAINT",
            "NOT_RESPONDING",
            "RESERVED",
            "POWERED_DOWN",
            "POWERING_DOWN",
            "REBOOT_REQUESTED",
            "REBOOT_ISSUED",
        ]
    } else {
        // Default: most critical states only
        vec![
            "DOWN",
            "DRAIN",
            "DRAINED",
            "DRAINING",
            "FAIL",
            "MAINT",
            "NOT_RESPONDING",
        ]
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

    Ok(display::format_problem_nodes(
        &problem_nodes,
        show_all,
        node_prefix_strip,
    ))
}
