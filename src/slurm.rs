//! Interface to Slurm commands using JSON output
//!
//! This module provides a high-level interface to Slurm's `sinfo` and `squeue`
//! commands using their JSON output format. It handles command execution,
//! JSON parsing, and error handling.

use crate::models::{ClusterStatus, JobHistoryInfo, JobInfo, NodeInfo, PersonalSummary, SacctResponse, SinfoResponse, SqueueResponse, SshareEntry, SshareResponse, SchedulerStats};
use anyhow::{Context, Result};
use std::process::Command;

/// Slurm interface for calling sinfo/squeue commands
///
/// This struct provides methods to query Slurm cluster information through
/// the `sinfo` and `squeue` commands with JSON output format.
#[derive(Debug, Clone)]
pub struct SlurmInterface {
    /// Path to directory containing Slurm binaries (sinfo, squeue, scontrol)
    pub slurm_bin_path: String,
}

impl Default for SlurmInterface {
    fn default() -> Self {
        Self {
            slurm_bin_path: "/usr/bin".to_string(),
        }
    }
}

impl SlurmInterface {
    pub fn new() -> Self {
        Self::default()
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
        let mut cmd = Command::new(format!("{}/sinfo", self.slurm_bin_path));
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

        let output = cmd
            .output()
            .context("Failed to execute sinfo command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("sinfo command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: SinfoResponse = serde_json::from_str(&stdout)
            .context("Failed to parse sinfo JSON output")?;

        if !response.errors.is_empty() {
            anyhow::bail!("sinfo errors: {}", response.errors.join("; "));
        }

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
        let mut cmd = Command::new(format!("{}/squeue", self.slurm_bin_path));
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

        let output = cmd
            .output()
            .context("Failed to execute squeue command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("squeue command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: SqueueResponse = serde_json::from_str(&stdout)
            .context("Failed to parse squeue JSON output")?;

        if !response.errors.is_empty() {
            anyhow::bail!("squeue errors: {}", response.errors.join("; "));
        }

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

        let jobs = self.get_jobs(
            users.as_deref(),
            None,
            partitions.as_deref(),
            None,
            None,
        )?;

        Ok(ClusterStatus { nodes, jobs })
    }

    /// Test if Slurm commands are available
    pub fn test_connection(&self) -> bool {
        Command::new(format!("{}/sinfo", self.slurm_bin_path))
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
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
        let mut cmd = Command::new(format!("{}/sacct", self.slurm_bin_path));
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

        let output = cmd
            .output()
            .context("Failed to execute sacct command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("sacct command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: SacctResponse = serde_json::from_str(&stdout)
            .context("Failed to parse sacct JSON output")?;

        if !response.errors.is_empty() {
            anyhow::bail!("sacct errors: {}", response.errors.join("; "));
        }

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
            true,  // Need all_users to see other users' jobs
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
            None,  // All states
            None,
        )?;

        // Get recent history (last 24 hours)
        let now = chrono::Utc::now();
        let yesterday = now - chrono::Duration::hours(24);
        let start_time = yesterday.format("%Y-%m-%dT%H:%M:%S").to_string();

        let recent_history = self.get_job_history(
            Some(username),
            Some(&start_time),
            None,
            None,
            None,
            false,
        )?;

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
            .unwrap_or_else(|_| "unknown".to_string())
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
        let mut cmd = Command::new(format!("{}/sshare", self.slurm_bin_path));
        cmd.arg("--json");

        // Always include the full tree for context
        cmd.arg("-a");  // All users

        if let Some(user) = user {
            cmd.arg("-u").arg(user);
        }

        if let Some(account) = account {
            cmd.arg("-A").arg(account);
        }

        let output = cmd
            .output()
            .context("Failed to execute sshare command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("sshare command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let response: SshareResponse = serde_json::from_str(&stdout)
            .context("Failed to parse sshare JSON output")?;

        if !response.errors.is_empty() {
            anyhow::bail!("sshare errors: {}", response.errors.join("; "));
        }

        Ok(response.shares.shares)
    }

    /// Get scheduler statistics from sdiag command
    ///
    /// Note: sdiag may require admin privileges on some clusters.
    /// This method returns SchedulerStats with available=false if access is denied.
    ///
    /// # Returns
    /// `SchedulerStats` with parsed scheduler statistics
    pub fn get_scheduler_stats(&self) -> SchedulerStats {
        let cmd = Command::new(format!("{}/sdiag", self.slurm_bin_path))
            .output();

        match cmd {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                SchedulerStats::from_sdiag_output(&stdout)
            }
            _ => {
                // sdiag not available or permission denied
                SchedulerStats {
                    available: false,
                    ..Default::default()
                }
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
    pub fn get_estimated_start(&self, job_id: u64) -> Option<i64> {
        let output = Command::new(format!("{}/squeue", self.slurm_bin_path))
            .arg("--start")
            .arg("-j")
            .arg(job_id.to_string())
            .arg("--noheader")
            .arg("-o")
            .arg("%S")  // Just the start time
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();

        if line.is_empty() || line == "N/A" || line == "Unknown" {
            return None;
        }

        // Parse date format like "2025-11-27T16:30:00"
        chrono::NaiveDateTime::parse_from_str(line, "%Y-%m-%dT%H:%M:%S")
            .ok()
            .map(|dt| dt.and_utc().timestamp())
    }
}

/// Shorten node names by removing 'demu4x' prefix
pub fn shorten_node_name(node_name: &str) -> &str {
    node_name.strip_prefix("demu4x").unwrap_or(node_name)
}

/// Shorten a comma-separated list of node names
pub fn shorten_node_list(node_list: &str) -> String {
    if node_list.is_empty() {
        return node_list.to_string();
    }

    node_list
        .split(',')
        .map(|node| shorten_node_name(node.trim()))
        .collect::<Vec<_>>()
        .join(",")
}