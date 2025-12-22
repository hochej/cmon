//! Interactive SLURM job launcher
//!
//! This module provides an interactive interface for starting SLURM development sessions.
//! It supports both CLI mode (with optional interactive account selection) and full TUI mode
//! where all job parameters can be configured interactively.

use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use crate::slurm::SlurmInterface;

/// Job configuration for interactive SLURM session
#[derive(Debug, Clone)]
pub struct JobConfig {
    pub cpus: u32,
    pub mem: String,
    pub time: String,
    pub partition: String,
    pub account: String,
    pub ntasks: u32,
    pub nodes: u32,
    pub gres: Option<String>,
}

/// CLI arguments for devrun command
#[derive(Args, Debug, Clone)]
pub struct DevrunArgs {
    /// Number of CPUs per task
    #[arg(short = 'c', long = "cpus-per-task", default_value = "2")]
    cpus: u32,

    /// Memory allocation (e.g., 20Gb, 100Gb)
    #[arg(short = 'm', long = "mem", default_value = "20Gb")]
    mem: String,

    /// Time limit in HH:MM:SS format
    #[arg(short = 't', long = "time", default_value = "04:00:00")]
    time: String,

    /// Partition name (cpu, gpu, fat, etc.)
    #[arg(short = 'p', long = "partition", default_value = "cpu")]
    partition: String,

    /// Account name (interactive selection if not provided)
    #[arg(short = 'a', long = "account")]
    account: Option<String>,

    /// Number of tasks
    #[arg(short = 'n', long = "ntasks", default_value = "1")]
    ntasks: u32,

    /// Number of nodes
    #[arg(long = "nodes", default_value = "1")]
    nodes: u32,

    /// Generic resources (default: gpu:1 for gpu/fat partitions)
    #[arg(short = 'g', long = "gres")]
    gres: Option<String>,

    /// Use TUI mode with interactive configuration
    #[arg(long)]
    tui: bool,
}

/// Main entry point for devrun command
pub fn run_devrun(args: DevrunArgs) -> Result<()> {
    let slurm = SlurmInterface::new();

    if args.tui {
        // TUI mode: full interactive configuration
        let config = configure_job_tui(&slurm, args)?;
        display_config(&config);

        if !confirm_execution()? {
            println!("{}", "✗ Cancelled by user".yellow());
            std::process::exit(0);
        }

        execute_srun(config)
    } else {
        // CLI mode: standard flow with optional account selection
        run_cli_mode(&slurm, args)
    }
}

/// CLI account selection with numbered list
fn select_account_cli(slurm: &SlurmInterface) -> Result<String> {
    println!("{}", "No account specified.".yellow());

    // Show spinner while fetching accounts
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .context("Failed to set spinner template")?
    );
    spinner.set_message("Fetching available accounts...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    let accounts = slurm.get_accounts()?;
    spinner.finish_and_clear();

    if accounts.is_empty() {
        anyhow::bail!("No accounts found. Please specify account with -a option.");
    }

    println!("\n{}", "Available accounts:".green().bold());
    for (i, acc) in accounts.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).cyan().bold(), acc);
    }

    print!("\n{} ", "Select account number:".yellow());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selection: usize = input.trim().parse()
        .context("Invalid input: please enter a number")?;

    if selection < 1 || selection > accounts.len() {
        anyhow::bail!("Invalid selection: must be between 1 and {}", accounts.len());
    }

    let account = accounts[selection - 1].clone();
    println!("{} {}\n", "✓ Selected account:".green(), account.bold());

    Ok(account)
}

/// Interactive TUI configuration wizard
fn configure_job_tui(slurm: &SlurmInterface, args: DevrunArgs) -> Result<JobConfig> {
    // Print section header
    println!();
    println!("{}", "═".repeat(60).bright_cyan());
    println!("{}", "Interactive Job Configuration".bright_cyan().bold());
    println!("{}", "═".repeat(60).bright_cyan());
    println!();

    println!("{}", "Fetching SLURM configuration...".cyan());

    // Fetch accounts and partitions
    let accounts = slurm.get_accounts()?;
    let partitions = slurm.get_partitions()?;

    if accounts.is_empty() {
        anyhow::bail!("No accounts found.");
    }

    println!();

    // A. Account Selection
    let account_default = accounts.iter()
        .position(|a| Some(a) == args.account.as_ref())
        .unwrap_or(0);

    let account_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Account")
        .items(&accounts)
        .default(account_default)
        .interact()?;
    let account = accounts[account_selection].clone();

    // B. Partition Selection
    let partition_default = partitions.iter()
        .position(|p| p == &args.partition)
        .unwrap_or(0);

    let partition_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Partition")
        .items(&partitions)
        .default(partition_default)
        .interact()?;
    let partition = partitions[partition_selection].clone();

    // C. CPU Selection
    let cpu_options = vec!["1", "2", "4", "8", "16", "32", "64", "Custom"];
    let cpu_default = cpu_options.iter()
        .position(|&c| c.parse::<u32>().ok() == Some(args.cpus))
        .unwrap_or(1);

    let cpu_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("CPUs per task")
        .items(&cpu_options)
        .default(cpu_default)
        .interact()?;

    let cpus = if cpu_selection == 7 {
        Input::<u32>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter custom CPU count")
            .validate_with(|input: &u32| -> Result<(), &str> {
                if *input > 0 { Ok(()) } else { Err("Must be greater than 0") }
            })
            .interact()?
    } else {
        cpu_options[cpu_selection].parse::<u32>().unwrap()
    };

    // D. Memory Selection
    let mem_options = vec!["10Gb", "20Gb", "40Gb", "80Gb", "100Gb", "200Gb", "Custom"];
    let mem_default = mem_options.iter()
        .position(|&m| m == args.mem)
        .unwrap_or(1);

    let mem_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Memory")
        .items(&mem_options)
        .default(mem_default)
        .interact()?;

    let mem = if mem_selection == 6 {
        Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter custom memory (e.g., 50Gb, 500Mb)")
            .validate_with(|input: &String| -> Result<(), &str> {
                let upper = input.to_uppercase();
                if upper.ends_with("GB") || upper.ends_with("MB") {
                    Ok(())
                } else {
                    Err("Format must be like '50Gb' or '500Mb'")
                }
            })
            .interact()?
    } else {
        mem_options[mem_selection].to_string()
    };

    // E. Time Limit Selection
    let time_options = vec![
        "01:00:00 (1 hour)",
        "02:00:00 (2 hours)",
        "04:00:00 (4 hours)",
        "08:00:00 (8 hours)",
        "12:00:00 (12 hours)",
        "24:00:00 (1 day)",
        "48:00:00 (2 days)",
        "Custom"
    ];

    let time_default = time_options.iter()
        .position(|t| t.starts_with(&args.time))
        .unwrap_or(2);

    let time_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Time limit")
        .items(&time_options)
        .default(time_default)
        .interact()?;

    let time = if time_selection == 7 {
        Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter time in HH:MM:SS format")
            .validate_with(|input: &String| -> Result<(), &str> {
                let parts: Vec<&str> = input.split(':').collect();
                if parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
                    Ok(())
                } else {
                    Err("Invalid format. Use HH:MM:SS")
                }
            })
            .interact()?
    } else {
        time_options[time_selection].split_whitespace().next().unwrap().to_string()
    };

    // F. GPU/GRES Selection (Conditional)
    let gres = if partition == "gpu" || partition == "fat" {
        let gres_options = vec![
            "No GPU",
            "gpu:1 (1 GPU)",
            "gpu:2 (2 GPUs)",
            "gpu:4 (4 GPUs)",
            "Custom GRES"
        ];

        let gres_default = match args.gres.as_deref() {
            Some("gpu:1") => 1,
            Some("gpu:2") => 2,
            Some("gpu:4") => 3,
            Some(_) => 4,
            None => 1,
        };

        let gres_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("GPU resources")
            .items(&gres_options)
            .default(gres_default)
            .interact()?;

        match gres_selection {
            0 => None,
            1 => Some("gpu:1".to_string()),
            2 => Some("gpu:2".to_string()),
            3 => Some("gpu:4".to_string()),
            4 => {
                let custom = Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter custom GRES specification")
                    .interact()?;
                Some(custom)
            }
            _ => None,
        }
    } else {
        let specify_gres = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Specify GRES resources?")
            .default(false)
            .interact()?;

        if specify_gres {
            let custom = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt("Enter GRES specification")
                .interact()?;
            Some(custom)
        } else {
            None
        }
    };

    // G. Tasks Selection
    let ntasks: u32 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of tasks")
        .default(args.ntasks)
        .validate_with(|input: &u32| -> Result<(), &str> {
            if *input > 0 { Ok(()) } else { Err("Must be greater than 0") }
        })
        .interact()?;

    // H. Nodes Selection
    let nodes: u32 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Number of nodes")
        .default(args.nodes)
        .validate_with(|input: &u32| -> Result<(), &str> {
            if *input > 0 { Ok(()) } else { Err("Must be greater than 0") }
        })
        .interact()?;

    println!();
    println!("{}", "✓ Configuration complete!".green().bold());
    println!();

    Ok(JobConfig {
        cpus,
        mem,
        time,
        partition,
        account,
        ntasks,
        nodes,
        gres,
    })
}

/// Display job configuration summary
fn display_config(config: &JobConfig) {
    println!("{}", "═".repeat(60).bright_blue());
    println!("{}", "Starting Interactive HPC Session".bright_blue().bold());
    println!("{}", "═".repeat(60).bright_blue());
    println!();
    println!("  {} {}", "CPUs:".cyan(), config.cpus.to_string().bold());
    println!("  {} {}", "Memory:".cyan(), config.mem.bold());
    println!("  {} {}", "Time Limit:".cyan(), config.time.bold());
    println!("  {} {}", "Partition:".cyan(), config.partition.bold());
    println!("  {} {}", "Account:".cyan(), config.account.bold());
    println!("  {} {}", "Tasks:".cyan(), config.ntasks.to_string().bold());
    println!("  {} {}", "Nodes:".cyan(), config.nodes.to_string().bold());
    if let Some(ref g) = config.gres {
        println!("  {} {}", "GPU Resources:".cyan(), g.bold());
    }
    println!();
}

/// Confirm execution before launching job
fn confirm_execution() -> Result<bool> {
    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed with this configuration?")
        .default(true)
        .interact()?;
    Ok(confirm)
}

/// Execute srun command with given configuration
fn execute_srun(config: JobConfig) -> Result<()> {
    let mut cmd = Command::new("srun");
    cmd.args(&[
        "--pty",
        "-n", &config.ntasks.to_string(),
        "--ntasks", &config.ntasks.to_string(),
        "--cpus-per-task", &config.cpus.to_string(),
        "-p", &config.partition,
        "--time", &config.time,
        "--mem", &config.mem,
        "--account", &config.account,
    ]);

    if let Some(ref g) = config.gres {
        cmd.args(&["--gres", g]);
    }

    cmd.arg("bash");

    // Show command being executed
    let cmd_str = format!("srun --pty -n {} --ntasks {} --cpus-per-task {} -p {} --time {} --mem {} --account {}{}bash",
        config.ntasks, config.ntasks, config.cpus, config.partition, config.time, config.mem, config.account,
        if let Some(ref g) = config.gres { format!(" --gres {} ", g) } else { " ".to_string() }
    );
    println!("{} {}", "Command:".bright_black(), cmd_str.bright_black());
    println!("{}", "─".repeat(60).bright_blue());
    println!();

    // Execute - this replaces the current process
    let err = cmd.exec();

    // If we reach here, exec failed
    anyhow::bail!("Failed to execute srun: {}", err)
}

/// Standard CLI mode flow
fn run_cli_mode(slurm: &SlurmInterface, mut args: DevrunArgs) -> Result<()> {
    // Handle account selection
    if args.account.is_none() {
        args.account = Some(select_account_cli(slurm)?);
    }

    // Handle GPU resources - default to gpu:1 for gpu and fat partitions
    if args.gres.is_none() && (args.partition == "gpu" || args.partition == "fat") {
        args.gres = Some("gpu:1".to_string());
    }

    // Convert Args to JobConfig
    let config = JobConfig {
        cpus: args.cpus,
        mem: args.mem,
        time: args.time,
        partition: args.partition,
        account: args.account.unwrap(),
        ntasks: args.ntasks,
        nodes: args.nodes,
        gres: args.gres,
    };

    // Display configuration
    display_config(&config);

    // Execute (no confirmation in CLI mode)
    execute_srun(config)
}
