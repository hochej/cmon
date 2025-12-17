//! Interface to Slurm commands using JSON output
//!
//! This module provides a high-level interface to Slurm's `sinfo` and `squeue`
//! commands using their JSON output format. It handles command execution,
//! JSON parsing, and error handling.

use crate::models::{
    ClusterStatus, JobHistoryInfo, JobInfo, NodeInfo, PersonalSummary, SacctResponse,
    SchedulerStats, SinfoResponse, SlurmResponse, SqueueResponse, SshareEntry, SshareResponse,
};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How the Slurm binary path was resolved
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolution {
    /// Explicitly configured via config file or environment variable
    Configured,
    /// Auto-detected via PATH (found sinfo in user's PATH)
    AutoDetected,
    /// Fell back to default /usr/bin and sinfo was found there
    Fallback,
    /// Fell back to default /usr/bin but sinfo was NOT found (likely misconfigured)
    FallbackUnverified,
}

/// Result of finding the Slurm binary path
#[derive(Debug, Clone)]
pub struct SlurmPathResult {
    pub path: PathBuf,
    #[allow(dead_code)] // Kept for potential future use and debugging
    pub resolution: PathResolution,
}

impl SlurmPathResult {
    /// Returns true if using the fallback path (should warn user)
    #[allow(dead_code)] // Kept for potential future use
    #[must_use]
    pub fn is_fallback(&self) -> bool {
        self.resolution == PathResolution::Fallback
    }
}

/// Find the directory containing Slurm binaries.
///
/// Resolution order:
/// 1. Explicit path provided (from config) - validated to be an existing directory.
///    If the path doesn't exist or isn't a directory, a warning is printed and
///    resolution continues to step 2.
/// 2. Auto-detect via PATH using the `which` crate to find `sinfo`, then extract
///    the parent directory.
/// 3. Fallback to `/usr/bin` if PATH detection fails.
///
/// # Arguments
/// * `config_path` - Optional explicit path from configuration
///
/// # Returns
/// `SlurmPathResult` containing the resolved path and how it was resolved
/// (Configured, AutoDetected, or Fallback).
pub fn find_slurm_bin_path(config_path: Option<&Path>) -> SlurmPathResult {
    // 1. Config path (highest priority) - validate it exists
    if let Some(path) = config_path {
        if path.is_dir() {
            return SlurmPathResult {
                path: path.to_path_buf(),
                resolution: PathResolution::Configured,
            };
        } else {
            // Configured path doesn't exist or isn't a directory - warn and continue
            eprintln!(
                "Warning: Configured slurm_bin_path '{}' is not a valid directory, trying auto-detection",
                path.display()
            );
        }
    }

    // 2. Auto-detect via PATH
    if let Ok(sinfo_path) = which::which("sinfo")
        && let Some(parent) = sinfo_path.parent()
    {
        return SlurmPathResult {
            path: parent.to_path_buf(),
            resolution: PathResolution::AutoDetected,
        };
    }

    // 3. Fallback to /usr/bin - validate sinfo exists there
    let fallback_path = PathBuf::from("/usr/bin");
    let sinfo_at_fallback = fallback_path.join("sinfo");
    if sinfo_at_fallback.exists() {
        SlurmPathResult {
            path: fallback_path,
            resolution: PathResolution::Fallback,
        }
    } else {
        // Warn user that Slurm was not found
        eprintln!(
            "Warning: Slurm binaries not found in PATH or {}. Commands may fail.",
            fallback_path.display()
        );
        SlurmPathResult {
            path: fallback_path,
            resolution: PathResolution::FallbackUnverified,
        }
    }
}

/// Slurm version information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlurmVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SlurmVersion {
    /// Minimum version required for JSON output support (21.08)
    pub const MIN_JSON_VERSION: SlurmVersion = SlurmVersion {
        major: 21,
        minor: 8,
        patch: 0,
    };

    /// Check if this version supports JSON output
    #[must_use]
    pub fn supports_json(&self) -> bool {
        *self >= Self::MIN_JSON_VERSION
    }
}

impl std::fmt::Display for SlurmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:02}.{}", self.major, self.minor, self.patch)
    }
}

/// Error type for parsing `SlurmVersion` from a string
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSlurmVersionError {
    input: String,
}

impl std::fmt::Display for ParseSlurmVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Slurm version string: '{}'", self.input)
    }
}

impl std::error::Error for ParseSlurmVersionError {}

impl std::str::FromStr for SlurmVersion {
    type Err = ParseSlurmVersionError;

    /// Parse a Slurm version string into a `SlurmVersion` struct
    ///
    /// Handles formats like:
    /// - "slurm 24.11.0"
    /// - "slurm-24.05.1"
    /// - "24.11.0"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Extract version number part (e.g., "24.11.0" from "slurm 24.11.0")
        let version_part = s
            .trim()
            .split(|c: char| c.is_whitespace() || c == '-')
            .find(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()))
            .ok_or_else(|| ParseSlurmVersionError {
                input: s.to_string(),
            })?;

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 2 {
            return Err(ParseSlurmVersionError {
                input: s.to_string(),
            });
        }

        let major = parts[0].parse().map_err(|_| ParseSlurmVersionError {
            input: s.to_string(),
        })?;
        let minor = parts[1].parse().map_err(|_| ParseSlurmVersionError {
            input: s.to_string(),
        })?;
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);

        Ok(SlurmVersion {
            major,
            minor,
            patch,
        })
    }
}

impl PartialOrd for SlurmVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SlurmVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// Error type for Slurm version detection failures
#[derive(Debug, Clone)]
pub enum SlurmVersionError {
    /// Could not execute sinfo command (binary not found, permission denied, etc.)
    CommandFailed(String),
    /// sinfo command returned non-zero exit code
    NonZeroExit(i32),
    /// Could not parse version string from sinfo output
    ParseFailed(String),
}

impl std::fmt::Display for SlurmVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlurmVersionError::CommandFailed(msg) => {
                write!(f, "Failed to execute sinfo: {}", msg)
            }
            SlurmVersionError::NonZeroExit(code) => {
                write!(f, "sinfo --version exited with code {}", code)
            }
            SlurmVersionError::ParseFailed(output) => {
                write!(
                    f,
                    "Could not parse version from output: '{}'",
                    output.trim()
                )
            }
        }
    }
}

impl std::error::Error for SlurmVersionError {}

/// Detect the installed Slurm version by running `sinfo --version`
///
/// Parses version strings like "slurm 24.11.0" or "slurm-24.05.1"
///
/// # Arguments
/// * `slurm_bin_path` - Path to the directory containing Slurm binaries
///
/// # Returns
/// `Ok(SlurmVersion)` if successfully detected.
///
/// # Errors
/// Returns `SlurmVersionError` with specific failure information:
/// - `CommandFailed`: Binary not found, permission denied, or other execution error
/// - `NonZeroExit`: sinfo command returned a non-zero exit code
/// - `ParseFailed`: Output could not be parsed as a valid version string
pub fn detect_slurm_version(slurm_bin_path: &Path) -> Result<SlurmVersion, SlurmVersionError> {
    let sinfo_path = slurm_bin_path.join("sinfo");
    let output = Command::new(&sinfo_path)
        .arg("--version")
        .output()
        .map_err(|e| SlurmVersionError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(SlurmVersionError::NonZeroExit(
            output.status.code().unwrap_or(-1),
        ));
    }

    let version_str = String::from_utf8_lossy(&output.stdout);
    version_str
        .parse()
        .map_err(|_| SlurmVersionError::ParseFailed(version_str.to_string()))
}

/// Result of checking Slurm JSON support
#[derive(Debug, Clone)]
pub enum JsonSupportResult {
    /// JSON is supported (Slurm 21.08+)
    Supported(SlurmVersion),
    /// Slurm version is too old for JSON support
    UnsupportedVersion(SlurmVersion),
    /// Could not detect Slurm version
    DetectionFailed(SlurmVersionError),
}

impl JsonSupportResult {
    /// Returns true if JSON is supported
    #[must_use]
    pub fn is_supported(&self) -> bool {
        matches!(self, JsonSupportResult::Supported(_))
    }
}

/// Check if Slurm version supports JSON output
///
/// Returns a `JsonSupportResult` that distinguishes between:
/// - JSON supported (Slurm 21.08+)
/// - Slurm too old for JSON
/// - Version detection failed
///
/// Use `is_supported()` for a simple boolean check.
pub fn check_slurm_json_support(slurm_bin_path: &Path) -> JsonSupportResult {
    match detect_slurm_version(slurm_bin_path) {
        Ok(version) => {
            if version.supports_json() {
                JsonSupportResult::Supported(version)
            } else {
                JsonSupportResult::UnsupportedVersion(version)
            }
        }
        Err(e) => JsonSupportResult::DetectionFailed(e),
    }
}

/// Check Slurm JSON support and print appropriate warnings
///
/// This is a convenience wrapper that prints warnings to stderr and returns
/// a simple boolean.
pub fn check_slurm_json_support_with_warnings(slurm_bin_path: &Path) -> bool {
    match check_slurm_json_support(slurm_bin_path) {
        JsonSupportResult::Supported(_) => true,
        JsonSupportResult::UnsupportedVersion(version) => {
            eprintln!(
                "Warning: Slurm {} detected. JSON output requires Slurm 21.08 or later.",
                version
            );
            eprintln!("Some features may not work correctly.");
            false
        }
        JsonSupportResult::DetectionFailed(e) => {
            eprintln!("Warning: Could not detect Slurm version: {}", e);
            eprintln!("JSON output may not be available.");
            false
        }
    }
}

/// Slurm interface for calling sinfo/squeue commands
///
/// This struct provides methods to query Slurm cluster information through
/// the `sinfo` and `squeue` commands with JSON output format.
#[derive(Debug, Clone)]
pub struct SlurmInterface {
    /// Path to directory containing Slurm binaries (sinfo, squeue, scontrol)
    pub slurm_bin_path: PathBuf,
    /// How the path was resolved (for diagnostics)
    resolution: PathResolution,
}

impl Default for SlurmInterface {
    fn default() -> Self {
        let result = find_slurm_bin_path(None);
        // Note: No warning here - let the caller decide if/how to warn
        Self {
            slurm_bin_path: result.path,
            resolution: result.resolution,
        }
    }
}

impl SlurmInterface {
    /// Create a new SlurmInterface with default settings
    #[allow(dead_code)] // Kept for backward compatibility
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new SlurmInterface using configuration.
    ///
    /// If the config specifies a slurm_bin_path, use it; otherwise auto-detect.
    /// Use `is_fallback_path()` to check if the path resolution fell back to default.
    ///
    /// # Arguments
    /// * `config_path` - Optional path from configuration
    pub fn with_config(config_path: Option<&Path>) -> Self {
        let result = find_slurm_bin_path(config_path);
        // Note: No warning here - let the caller decide if/how to warn
        Self {
            slurm_bin_path: result.path,
            resolution: result.resolution,
        }
    }

    /// Check if the current Slurm path is a fallback (should warn user)
    ///
    /// Returns true if path resolution fell back to the default /usr/bin
    /// because neither a configured path nor auto-detection via PATH succeeded.
    #[must_use]
    pub fn is_fallback_path(&self) -> bool {
        matches!(
            self.resolution,
            PathResolution::Fallback | PathResolution::FallbackUnverified
        )
    }

    /// Check if the fallback path is unverified (sinfo not found at /usr/bin)
    ///
    /// Returns true if Slurm binaries were not found at the fallback path,
    /// indicating that commands will likely fail.
    #[must_use]
    pub fn is_unverified_fallback(&self) -> bool {
        self.resolution == PathResolution::FallbackUnverified
    }

    /// Execute a Slurm command and parse the JSON response.
    ///
    /// This is a generic helper that handles the common pattern of:
    /// 1. Executing a pre-built Command
    /// 2. Checking for successful execution
    /// 3. Parsing the JSON output
    /// 4. Checking for errors in the Slurm response
    ///
    /// # Arguments
    /// * `cmd` - A pre-built Command (with all arguments already added)
    /// * `error_context` - A human-readable description of the command (e.g., "sinfo", "squeue")
    ///
    /// # Type Parameters
    /// * `T` - The response type to deserialize into. Must implement `DeserializeOwned` and `SlurmResponse`.
    fn execute_slurm_command<T>(&self, mut cmd: Command, error_context: &str) -> Result<T>
    where
        T: DeserializeOwned + SlurmResponse,
    {
        let output = cmd
            .output()
            .with_context(|| format!("Failed to execute {} command", error_context))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{} command failed: {}", error_context, stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: T = serde_json::from_str(&stdout)
            .with_context(|| format!("Failed to parse {} JSON output", error_context))?;

        if !response.errors().is_empty() {
            anyhow::bail!("{} errors: {}", error_context, response.errors().join("; "));
        }

        Ok(response)
    }

    /// Get node information from sinfo command
    ///
    /// # Arguments
    /// * `partition` - Optional partition name to filter by
    /// * `nodelist` - Optional node list expression (e.g., "node[001-010]")
    /// * `states` - Optional list of states to filter by (e.g., ["IDLE", "MIXED"])
    /// * `all_partitions` - If true, includes hidden partitions
    ///
    /// # Returns
    /// Vector of `NodeInfo` structs, filtered to remove nodes with empty names
    pub fn get_nodes(
        &self,
        partition: Option<&str>,
        nodelist: Option<&str>,
        states: Option<&[String]>,
        all_partitions: bool,
    ) -> Result<Vec<NodeInfo>> {
        let sinfo_path = self.slurm_bin_path.join("sinfo");
        let mut cmd = Command::new(&sinfo_path);
        cmd.arg("-N").arg("--json");

        if all_partitions {
            cmd.arg("--all");
        }

        if let Some(partition) = partition {
            cmd.arg("-p").arg(partition);
        }

        if let Some(nodelist) = nodelist {
            cmd.arg("-n").arg(nodelist);
        }

        if let Some(states) = states {
            cmd.arg("--states").arg(states.join(","));
        }

        let response: SinfoResponse = self.execute_slurm_command(cmd, "sinfo")?;

        Ok(response
            .sinfo
            .into_iter()
            .filter(|node| !node.name().is_empty())
            .collect())
    }

    /// Get job information from squeue command
    ///
    /// # Arguments
    /// * `users` - Optional list of usernames to filter by
    /// * `accounts` - Optional list of account names to filter by
    /// * `partitions` - Optional list of partition names to filter by
    /// * `states` - Optional list of job states to filter by (e.g., ["RUNNING", "PENDING"])
    /// * `job_ids` - Optional list of job IDs to filter by
    ///
    /// # Returns
    /// Vector of `JobInfo` structs, filtered to remove jobs with ID 0
    pub fn get_jobs(
        &self,
        users: Option<&[String]>,
        accounts: Option<&[String]>,
        partitions: Option<&[String]>,
        states: Option<&[String]>,
        job_ids: Option<&[u64]>,
    ) -> Result<Vec<JobInfo>> {
        let squeue_path = self.slurm_bin_path.join("squeue");
        let mut cmd = Command::new(&squeue_path);
        cmd.arg("--json");

        if let Some(states) = states {
            cmd.arg("-t").arg(states.join(","));
        }

        if let Some(users) = users {
            cmd.arg("-u").arg(users.join(","));
        }

        if let Some(accounts) = accounts {
            for account in accounts {
                cmd.arg("-A").arg(account);
            }
        }

        if let Some(partitions) = partitions {
            cmd.arg("-p").arg(partitions.join(","));
        }

        if let Some(job_ids) = job_ids {
            let ids: Vec<String> = job_ids.iter().map(|id| id.to_string()).collect();
            cmd.arg("-j").arg(ids.join(","));
        }

        let response: SqueueResponse = self.execute_slurm_command(cmd, "squeue")?;

        Ok(response
            .jobs
            .into_iter()
            .filter(|job| job.job_id != 0)
            .collect())
    }

    /// Get complete cluster status including nodes and jobs
    pub fn get_cluster_status(
        &self,
        partition: Option<&str>,
        user: Option<&str>,
        nodelist: Option<&str>,
    ) -> Result<ClusterStatus> {
        let nodes = self.get_nodes(partition, nodelist, None, false)?;

        let users = user.map(|u| vec![u.to_string()]);
        let partitions = partition.map(|p| vec![p.to_string()]);

        let jobs = self.get_jobs(users.as_deref(), None, partitions.as_deref(), None, None)?;

        Ok(ClusterStatus { nodes, jobs })
    }

    /// Test if Slurm commands are available
    ///
    /// # Returns
    /// * `Ok(())` if sinfo --version executes successfully
    /// * `Err(String)` with a specific error message describing why the connection failed
    ///
    /// # Errors
    /// Returns an error if:
    /// - The sinfo binary is not found
    /// - Permission is denied to execute the binary
    /// - The command exits with a non-zero status
    /// - Any other I/O error occurs
    pub fn test_connection(&self) -> Result<(), String> {
        let sinfo_path = self.slurm_bin_path.join("sinfo");

        match Command::new(&sinfo_path).arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!(
                        "sinfo command failed with exit code {}: {}",
                        output.status.code().unwrap_or(-1),
                        stderr.trim()
                    ))
                }
            }
            Err(e) => {
                let msg = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("sinfo binary not found at '{}'", sinfo_path.display())
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        format!(
                            "permission denied when trying to execute '{}'",
                            sinfo_path.display()
                        )
                    }
                    _ => format!("failed to execute sinfo: {e}"),
                };
                Err(msg)
            }
        }
    }

    /// Get job history from sacct command
    ///
    /// # Arguments
    /// * `user` - Optional username to filter by (defaults to current user if None)
    /// * `start_time` - Optional start time in YYYY-MM-DD format
    /// * `end_time` - Optional end time in YYYY-MM-DD format
    /// * `states` - Optional list of job states to filter by
    /// * `job_ids` - Optional list of specific job IDs
    /// * `all_users` - If true, show all users' jobs (requires admin privileges)
    ///
    /// # Returns
    /// Vector of `JobHistoryInfo` structs
    pub fn get_job_history(
        &self,
        user: Option<&str>,
        start_time: Option<&str>,
        end_time: Option<&str>,
        states: Option<&[String]>,
        job_ids: Option<&[u64]>,
        all_users: bool,
    ) -> Result<Vec<JobHistoryInfo>> {
        let sacct_path = self.slurm_bin_path.join("sacct");
        let mut cmd = Command::new(&sacct_path);
        cmd.arg("--json");

        if all_users {
            cmd.arg("-a");
        } else if let Some(user) = user {
            cmd.arg("-u").arg(user);
        }

        if let Some(start) = start_time {
            cmd.arg("-S").arg(start);
        }

        if let Some(end) = end_time {
            cmd.arg("-E").arg(end);
        }

        if let Some(states) = states {
            cmd.arg("-s").arg(states.join(","));
        }

        if let Some(job_ids) = job_ids {
            let ids: Vec<String> = job_ids.iter().map(|id| id.to_string()).collect();
            cmd.arg("-j").arg(ids.join(","));
        }

        let response: SacctResponse = self.execute_slurm_command(cmd, "sacct")?;

        // Filter out job steps (keep only main job entries)
        // Job steps have IDs like "12345.0", "12345.batch", etc.
        Ok(response
            .jobs
            .into_iter()
            .filter(|job| job.job_id != 0)
            .collect())
    }

    /// Get detailed information for a specific job
    ///
    /// # Arguments
    /// * `job_id` - The job ID to look up
    ///
    /// # Returns
    /// `JobHistoryInfo` if found
    pub fn get_job_details(&self, job_id: u64) -> Result<JobHistoryInfo> {
        let jobs = self.get_job_history(
            None,
            None,
            None,
            None,
            Some(&[job_id]),
            true, // Need all_users to see other users' jobs
        )?;

        jobs.into_iter()
            .find(|j| j.job_id == job_id)
            .ok_or_else(|| anyhow::anyhow!("Job {} not found", job_id))
    }

    /// Get personal summary for a user
    ///
    /// Combines current queue info with recent job history
    pub fn get_personal_summary(&self, username: &str) -> Result<PersonalSummary> {
        // Get current jobs from squeue
        let users = vec![username.to_string()];
        let current_jobs = self.get_jobs(
            Some(&users),
            None,
            None,
            None, // All states
            None,
        )?;

        // Get recent history (last 24 hours)
        let now = chrono::Utc::now();
        let yesterday = now - chrono::Duration::hours(24);
        let start_time = yesterday.format("%Y-%m-%dT%H:%M:%S").to_string();

        let recent_history =
            self.get_job_history(Some(username), Some(&start_time), None, None, None, false)?;

        // Calculate statistics
        let running_jobs = current_jobs.iter().filter(|j| j.is_running()).count() as u32;
        let pending_jobs = current_jobs.iter().filter(|j| j.is_pending()).count() as u32;

        let completed_24h = recent_history.iter().filter(|j| j.is_completed()).count() as u32;
        let failed_24h = recent_history.iter().filter(|j| j.is_failed()).count() as u32;
        let timeout_24h = recent_history.iter().filter(|j| j.is_timeout()).count() as u32;
        let cancelled_24h = recent_history.iter().filter(|j| j.is_cancelled()).count() as u32;

        // Calculate CPU hours (elapsed time * CPUs for completed jobs)
        let total_cpu_hours_24h: f64 = recent_history
            .iter()
            .filter(|j| !j.is_pending())
            .map(|j| (j.time.elapsed as f64 / 3600.0) * j.required.cpus as f64)
            .sum();

        // Calculate GPU hours
        let total_gpu_hours_24h: f64 = recent_history
            .iter()
            .filter(|j| !j.is_pending())
            .map(|j| (j.time.elapsed as f64 / 3600.0) * j.allocated_gpus() as f64)
            .sum();

        // Average efficiencies
        let cpu_efficiencies: Vec<f64> = recent_history
            .iter()
            .filter_map(|j| j.cpu_efficiency())
            .collect();
        let avg_cpu_efficiency = if !cpu_efficiencies.is_empty() {
            Some(cpu_efficiencies.iter().sum::<f64>() / cpu_efficiencies.len() as f64)
        } else {
            None
        };

        let mem_efficiencies: Vec<f64> = recent_history
            .iter()
            .filter_map(|j| j.memory_efficiency())
            .collect();
        let avg_memory_efficiency = if !mem_efficiencies.is_empty() {
            Some(mem_efficiencies.iter().sum::<f64>() / mem_efficiencies.len() as f64)
        } else {
            None
        };

        // Average wait time
        let wait_times: Vec<u64> = recent_history
            .iter()
            .filter_map(|j| j.wait_time())
            .collect();
        let avg_wait_time_seconds = if !wait_times.is_empty() {
            Some(wait_times.iter().sum::<u64>() / wait_times.len() as u64)
        } else {
            None
        };

        Ok(PersonalSummary {
            username: username.to_string(),
            running_jobs,
            pending_jobs,
            completed_24h,
            failed_24h,
            timeout_24h,
            cancelled_24h,
            total_cpu_hours_24h,
            total_gpu_hours_24h,
            avg_cpu_efficiency,
            avg_memory_efficiency,
            avg_wait_time_seconds,
            current_jobs,
            recent_jobs: recent_history,
        })
    }

    /// Get current username from environment
    pub fn get_current_user() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| {
                eprintln!("Warning: Could not determine username from USER or LOGNAME environment variables");
                "unknown".to_string()
            })
    }

    /// Get fairshare information from sshare command
    ///
    /// # Arguments
    /// * `user` - Optional username to filter by
    /// * `account` - Optional account to filter by
    ///
    /// # Returns
    /// Vector of `SshareEntry` structs
    pub fn get_fairshare(
        &self,
        user: Option<&str>,
        account: Option<&str>,
    ) -> Result<Vec<SshareEntry>> {
        let sshare_path = self.slurm_bin_path.join("sshare");
        let mut cmd = Command::new(&sshare_path);
        cmd.arg("--json");

        // Always include the full tree for context
        cmd.arg("-a"); // All users

        if let Some(user) = user {
            cmd.arg("-u").arg(user);
        }

        if let Some(account) = account {
            cmd.arg("-A").arg(account);
        }

        let response: SshareResponse = self.execute_slurm_command(cmd, "sshare")?;

        Ok(response.shares.shares)
    }

    /// Get scheduler statistics from sdiag command
    ///
    /// Note: sdiag may require admin privileges on some clusters.
    /// This method returns `SchedulerStats::Unavailable` if access is denied.
    ///
    /// # Returns
    /// `SchedulerStats` enum - either Available with stats or Unavailable with reason
    pub fn get_scheduler_stats(&self) -> SchedulerStats {
        let sdiag_path = self.slurm_bin_path.join("sdiag");
        let cmd = Command::new(&sdiag_path).output();

        match cmd {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                SchedulerStats::from_sdiag_output(&stdout)
            }
            Ok(output) => {
                // Command executed but returned non-zero exit code
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code();

                let reason = if stderr.contains("Permission denied")
                    || stderr.contains("Access denied")
                {
                    tracing::debug!(
                        "sdiag permission denied: exit_code={:?}, stderr={}",
                        exit_code,
                        stderr.trim()
                    );
                    "permission denied".to_string()
                } else if stderr.contains("slurm_load_ctl")
                    || stderr.contains("Unable to contact")
                {
                    tracing::debug!(
                        "sdiag cannot contact slurmctld: exit_code={:?}, stderr={}",
                        exit_code,
                        stderr.trim()
                    );
                    "cannot contact slurmctld".to_string()
                } else {
                    tracing::debug!(
                        "sdiag failed: exit_code={:?}, stderr={}",
                        exit_code,
                        stderr.trim()
                    );
                    format!(
                        "command failed (exit code {})",
                        exit_code.map_or("unknown".to_string(), |c| c.to_string())
                    )
                };

                SchedulerStats::unavailable(reason)
            }
            Err(e) => {
                // I/O error - binary not found, permission to execute, etc.
                let reason = if e.kind() == std::io::ErrorKind::NotFound {
                    tracing::debug!("sdiag binary not found at {:?}", sdiag_path);
                    format!("sdiag not found at {}", sdiag_path.display())
                } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                    tracing::debug!(
                        "permission denied executing sdiag at {:?}: {}",
                        sdiag_path,
                        e
                    );
                    "permission denied (cannot execute sdiag)".to_string()
                } else {
                    tracing::debug!("sdiag I/O error: {} (kind: {:?})", e, e.kind());
                    format!("I/O error: {}", e)
                };

                SchedulerStats::unavailable(reason)
            }
        }
    }

    /// Get estimated start time for a pending job using squeue --start
    ///
    /// # Arguments
    /// * `job_id` - The job ID to get start estimate for
    ///
    /// # Returns
    /// Optional estimated start time as Unix timestamp
    #[allow(dead_code)]
    pub fn get_estimated_start(&self, job_id: u64) -> Option<i64> {
        let squeue_path = self.slurm_bin_path.join("squeue");
        let output = match Command::new(&squeue_path)
            .arg("--start")
            .arg("-j")
            .arg(job_id.to_string())
            .arg("--noheader")
            .arg("-o")
            .arg("%S") // Just the start time
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                tracing::debug!(
                    job_id = job_id,
                    error = %e,
                    "Failed to execute squeue --start command"
                );
                return None;
            }
        };

        if !output.status.success() {
            tracing::debug!(
                job_id = job_id,
                exit_code = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "squeue --start command failed"
            );
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();

        if line.is_empty() || line == "N/A" || line == "Unknown" {
            return None;
        }

        // Parse date format like "2025-11-27T16:30:00"
        match chrono::NaiveDateTime::parse_from_str(line, "%Y-%m-%dT%H:%M:%S") {
            Ok(dt) => Some(dt.and_utc().timestamp()),
            Err(e) => {
                tracing::debug!(
                    job_id = job_id,
                    raw_value = line,
                    error = %e,
                    "Failed to parse estimated start time"
                );
                None
            }
        }
    }
}

/// Shorten node names by removing a configurable prefix
///
/// If prefix is empty, returns the original name unchanged.
/// This makes the tool portable across different clusters.
pub fn shorten_node_name<'a>(node_name: &'a str, prefix: &str) -> &'a str {
    if prefix.is_empty() {
        node_name
    } else {
        node_name.strip_prefix(prefix).unwrap_or(node_name)
    }
}

/// Shorten a comma-separated list of node names
pub fn shorten_node_list(node_list: &str, prefix: &str) -> String {
    if node_list.is_empty() {
        return node_list.to_string();
    }

    node_list
        .split(',')
        .map(|node| shorten_node_name(node.trim(), prefix))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slurm_version_from_str_standard() {
        let version: SlurmVersion = "slurm 24.11.0".parse().unwrap();
        assert_eq!(version.major, 24);
        assert_eq!(version.minor, 11);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_slurm_version_from_str_with_hyphen() {
        let version: SlurmVersion = "slurm-24.05.1".parse().unwrap();
        assert_eq!(version.major, 24);
        assert_eq!(version.minor, 5);
        assert_eq!(version.patch, 1);
    }

    #[test]
    fn test_slurm_version_from_str_just_numbers() {
        let version: SlurmVersion = "24.11.0".parse().unwrap();
        assert_eq!(version.major, 24);
        assert_eq!(version.minor, 11);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_slurm_version_from_str_two_parts() {
        let version: SlurmVersion = "slurm 21.08".parse().unwrap();
        assert_eq!(version.major, 21);
        assert_eq!(version.minor, 8);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_slurm_version_from_str_with_newline() {
        let version: SlurmVersion = "slurm 24.11.0\n".parse().unwrap();
        assert_eq!(version.major, 24);
        assert_eq!(version.minor, 11);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_slurm_version_from_str_invalid() {
        assert!("not a version".parse::<SlurmVersion>().is_err());
        assert!("".parse::<SlurmVersion>().is_err());
        assert!("slurm".parse::<SlurmVersion>().is_err());
    }

    #[test]
    fn test_slurm_version_supports_json() {
        // Version 21.08 and later should support JSON
        assert!(
            SlurmVersion {
                major: 21,
                minor: 8,
                patch: 0
            }
            .supports_json()
        );
        assert!(
            SlurmVersion {
                major: 24,
                minor: 11,
                patch: 0
            }
            .supports_json()
        );
        assert!(
            SlurmVersion {
                major: 22,
                minor: 0,
                patch: 0
            }
            .supports_json()
        );

        // Versions before 21.08 should not support JSON
        assert!(
            !SlurmVersion {
                major: 21,
                minor: 7,
                patch: 99
            }
            .supports_json()
        );
        assert!(
            !SlurmVersion {
                major: 20,
                minor: 11,
                patch: 0
            }
            .supports_json()
        );
        assert!(
            !SlurmVersion {
                major: 19,
                minor: 5,
                patch: 0
            }
            .supports_json()
        );
    }

    #[test]
    fn test_slurm_version_display() {
        assert_eq!(
            format!(
                "{}",
                SlurmVersion {
                    major: 24,
                    minor: 11,
                    patch: 0
                }
            ),
            "24.11.0"
        );
        assert_eq!(
            format!(
                "{}",
                SlurmVersion {
                    major: 21,
                    minor: 8,
                    patch: 5
                }
            ),
            "21.08.5"
        );
    }

    #[test]
    fn test_slurm_version_ordering() {
        let v20 = SlurmVersion {
            major: 20,
            minor: 0,
            patch: 0,
        };
        let v21_7 = SlurmVersion {
            major: 21,
            minor: 7,
            patch: 0,
        };
        let v21_8 = SlurmVersion {
            major: 21,
            minor: 8,
            patch: 0,
        };
        let v24 = SlurmVersion {
            major: 24,
            minor: 11,
            patch: 0,
        };

        assert!(v20 < v21_7);
        assert!(v21_7 < v21_8);
        assert!(v21_8 < v24);
        assert!(v21_8 == SlurmVersion::MIN_JSON_VERSION);
    }
}
