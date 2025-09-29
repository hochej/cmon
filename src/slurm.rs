//! Interface to Slurm commands using JSON output
//!
//! This module provides a high-level interface to Slurm's `sinfo` and `squeue`
//! commands using their JSON output format. It handles command execution,
//! JSON parsing, and error handling.

use crate::models::{ClusterStatus, JobInfo, NodeInfo, SinfoResponse, SqueueResponse};
use anyhow::{Context, Result};
use std::process::Command;
use std::time::Duration;

/// Slurm interface for calling sinfo/squeue commands
///
/// This struct provides methods to query Slurm cluster information through
/// the `sinfo` and `squeue` commands with JSON output format.
#[derive(Debug, Clone)]
pub struct SlurmInterface {
    /// Path to directory containing Slurm binaries (sinfo, squeue, scontrol)
    pub slurm_bin_path: String,
    /// Command timeout (not currently enforced)
    #[allow(dead_code)]
    pub timeout: Duration,
}

impl Default for SlurmInterface {
    fn default() -> Self {
        Self {
            slurm_bin_path: "/usr/bin".to_string(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl SlurmInterface {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn with_bin_path(mut self, path: String) -> Self {
        self.slurm_bin_path = path;
        self
    }

    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
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

    /// Expand a hostlist expression to individual hostnames
    #[allow(dead_code)]
    pub fn expand_hostlist(&self, hostlist: &str) -> Result<Vec<String>> {
        let hostlist = hostlist.trim();

        if hostlist.is_empty() {
            return Ok(vec![]);
        }

        // If no special characters, return as-is
        if !hostlist.contains('[') && !hostlist.contains(',') {
            return Ok(vec![hostlist.to_string()]);
        }

        let cmd = Command::new(format!("{}/scontrol", self.slurm_bin_path))
            .arg("show")
            .arg("hostnames")
            .arg(hostlist)
            .output()
            .context("Failed to execute scontrol command")?;

        if !cmd.status.success() {
            // If expansion fails, return original as single item
            return Ok(vec![hostlist.to_string()]);
        }

        let output = String::from_utf8_lossy(&cmd.stdout);
        Ok(output
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
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