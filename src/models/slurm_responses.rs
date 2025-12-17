//! Slurm API response wrappers.
//!
//! This module contains the wrapper types for deserializing responses
//! from Slurm commands (squeue, sinfo, sacct, sshare).

use serde::{Deserialize, Serialize};

use super::job::{JobHistoryInfo, JobInfo};
use super::node::NodeInfo;

/// Trait for Slurm command responses that have an errors field.
///
/// This trait enables generic handling of Slurm command responses, allowing
/// the `execute_slurm_command` helper in `slurm.rs` to check for errors
/// in a type-safe way across all response types.
pub trait SlurmResponse {
    /// Returns a reference to the errors reported by the Slurm command.
    fn errors(&self) -> &[String];
}

/// Slurm API response wrapper for sinfo
#[derive(Debug, Deserialize, Serialize)]
pub struct SinfoResponse {
    #[serde(default)]
    pub sinfo: Vec<NodeInfo>,

    #[serde(default)]
    pub errors: Vec<String>,
}

impl SlurmResponse for SinfoResponse {
    fn errors(&self) -> &[String] {
        &self.errors
    }
}

/// Slurm API response wrapper for squeue
#[derive(Debug, Deserialize, Serialize)]
pub struct SqueueResponse {
    #[serde(default)]
    pub jobs: Vec<JobInfo>,

    #[serde(default)]
    pub errors: Vec<String>,
}

impl SlurmResponse for SqueueResponse {
    fn errors(&self) -> &[String] {
        &self.errors
    }
}

/// Overall cluster status
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    pub nodes: Vec<NodeInfo>,
    pub jobs: Vec<JobInfo>,
}

impl ClusterStatus {
    #[must_use]
    pub fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn idle_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_idle()).count()
    }

    #[must_use]
    pub fn down_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_down()).count()
    }

    #[must_use]
    pub fn mixed_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_mixed()).count()
    }

    #[must_use]
    pub fn total_cpus(&self) -> u32 {
        self.nodes.iter().map(|n| n.cpus.total).sum()
    }

    #[must_use]
    pub fn allocated_cpus(&self) -> u32 {
        self.nodes.iter().map(|n| n.cpus.allocated).sum()
    }

    #[must_use]
    pub fn cpu_utilization(&self) -> f64 {
        let total = self.total_cpus();
        if total == 0 {
            0.0
        } else {
            (self.allocated_cpus() as f64 / total as f64) * 100.0
        }
    }
}

/// Slurm API response wrapper for sacct
#[derive(Debug, Deserialize, Serialize)]
pub struct SacctResponse {
    #[serde(default)]
    pub jobs: Vec<JobHistoryInfo>,

    #[serde(default)]
    pub errors: Vec<String>,

    #[serde(default)]
    pub warnings: Vec<SacctWarning>,
}

impl SlurmResponse for SacctResponse {
    fn errors(&self) -> &[String] {
        &self.errors
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SacctWarning {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source: String,
}

/// Personal usage summary
#[derive(Debug, Clone, Default)]
pub struct PersonalSummary {
    pub username: String,
    pub running_jobs: u32,
    pub pending_jobs: u32,
    pub completed_24h: u32,
    pub failed_24h: u32,
    pub timeout_24h: u32,
    pub cancelled_24h: u32,
    pub total_cpu_hours_24h: f64,
    pub total_gpu_hours_24h: f64,
    pub avg_cpu_efficiency: Option<f64>,
    pub avg_memory_efficiency: Option<f64>,
    pub avg_wait_time_seconds: Option<u64>,
    pub current_jobs: Vec<JobInfo>,
    pub recent_jobs: Vec<JobHistoryInfo>,
}
