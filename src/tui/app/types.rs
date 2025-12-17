//! Core data types for the TUI application
//!
//! This module contains the fundamental data structures used throughout the TUI:
//! - `SlurmTime`: Wrapper for Slurm epoch timestamps with explicit unknown handling
//! - `JobId`: Job identifier supporting both regular and array jobs
//! - `TuiJobInfo`: Extended job information for TUI display
//! - `PartitionStatus`: Aggregated partition statistics
//! - `DataSlice`: Generic container with staleness tracking

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use crate::formatting::format_duration_hms;
use crate::models::{JobInfo, JobState};

/// Time value with explicit unknown handling (0 = unknown in Slurm)
#[derive(Debug, Clone, Copy, Default)]
pub struct SlurmTime(i64);

impl SlurmTime {
    #[must_use]
    pub fn from_epoch(epoch: i64) -> Self {
        Self(epoch)
    }

    #[must_use]
    pub fn is_known(&self) -> bool {
        self.0 > 0 // Slurm uses 0 for "not set"
    }

    #[must_use]
    pub fn as_epoch(&self) -> Option<i64> {
        if self.is_known() { Some(self.0) } else { None }
    }

    #[must_use]
    pub fn as_datetime(&self) -> Option<chrono::DateTime<chrono::Local>> {
        use chrono::TimeZone;
        self.as_epoch()
            .and_then(|e| chrono::Local.timestamp_opt(e, 0).single())
    }
}

/// Job ID supporting both regular jobs and array jobs
///
/// Uses `NonZeroU64` for `base_id` because a zero job ID is never valid in Slurm,
/// making this invariant structural and enabling niche optimization for `Option<JobId>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobId {
    pub base_id: NonZeroU64,
    pub array_task_id: Option<u32>,
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.array_task_id {
            Some(task) => write!(f, "{}_{}", self.base_id.get(), task),
            None => write!(f, "{}", self.base_id.get()),
        }
    }
}

/// Extended job information for TUI display
#[derive(Debug, Clone)]
pub struct TuiJobInfo {
    pub job_id: JobId,
    pub name: String,
    pub user_name: String,
    pub account: String,
    pub partition: String,

    // State information
    pub state: JobState,
    #[allow(dead_code)]
    pub state_raw: String,
    pub state_reason: String,

    // Priority and QOS
    pub priority: u32,
    pub qos: String,

    // Time information
    pub submit_time: SlurmTime,
    pub start_time: SlurmTime,
    pub end_time: SlurmTime,
    pub time_limit_seconds: u32,
    pub elapsed_seconds: u32,

    // Resources
    pub nodes: String,
    pub node_count: u32,
    pub cpus: u32,

    // Job shape
    pub ntasks: u32,
    pub cpus_per_task: u32,
    #[allow(dead_code)]
    pub ntasks_per_node: Option<u32>,
    pub constraint: String,

    // TRES resources
    #[allow(dead_code)]
    pub tres_requested: HashMap<String, f64>,
    #[allow(dead_code)]
    pub tres_allocated: HashMap<String, f64>,

    // Computed GPU info
    pub gpu_count: u32,
    pub gpu_type: Option<String>,
    pub memory_gb: f64,

    // Paths
    pub working_directory: String,
    pub stdout_path: String,
    pub stderr_path: String,

    // Dependencies and array info
    pub dependency: String,
    pub array_job_id: Option<u64>,
    pub array_task_count: Option<u32>,
    pub array_tasks_pending: Option<u32>,
    pub array_tasks_running: Option<u32>,
    pub array_tasks_completed: Option<u32>,
}

impl TuiJobInfo {
    /// Convert from models::JobInfo
    ///
    /// Returns `None` if the job has an invalid (zero) job ID, which should never
    /// occur with valid Slurm data but may happen with malformed JSON or test data.
    pub fn from_job_info(job: &JobInfo) -> Option<Self> {
        // Validate job_id first - zero is invalid in Slurm
        let base_id = match NonZeroU64::new(job.job_id) {
            Some(id) => id,
            None => {
                tracing::warn!(
                    job_name = %job.name,
                    "Skipping job with invalid zero job_id"
                );
                return None;
            }
        };

        // Get the first state string for display
        let state_str = job.state.first().map(|s| s.as_str()).unwrap_or("UNKNOWN");
        let state = JobState::from_state_string(state_str);

        // Get GPU info from allocated resources
        let gpu_info = job.gpu_type_info();

        // Calculate elapsed time from start_time if running
        let elapsed_seconds = if let Some(start) = job.start_time.value() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now.saturating_sub(start) as u32
        } else {
            0
        };

        // Extract array task id from job_id if present (format: "12345_67")
        let array_task_id = None; // Not directly available in current JobInfo

        Some(Self {
            job_id: JobId {
                base_id,
                array_task_id,
            },
            name: job.name.clone(),
            user_name: job.user_name.clone(),
            account: job.account.clone(),
            partition: job.partition.clone(),

            state,
            state_raw: state_str.to_string(),
            state_reason: job.state_reason.clone(),

            priority: job.priority.number() as u32,
            qos: job.qos.clone(),

            submit_time: job
                .submit_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            start_time: job
                .start_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            end_time: job
                .end_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            time_limit_seconds: job.time_limit.value().map(|n| (n * 60) as u32).unwrap_or(0),
            elapsed_seconds,

            nodes: job.nodes.clone(),
            node_count: 1, // Simplified - could parse from nodes string
            cpus: job.cpus_per_task.number() as u32 * job.tasks.number().max(1) as u32,

            ntasks: job.tasks.value().map(|n| n as u32).unwrap_or(1),
            cpus_per_task: job.cpus_per_task.value().map(|n| n as u32).unwrap_or(1),
            ntasks_per_node: None,
            constraint: String::new(),

            tres_requested: HashMap::new(),
            tres_allocated: HashMap::new(),

            gpu_count: gpu_info.count,
            gpu_type: if gpu_info.gpu_type.is_empty() {
                None
            } else {
                Some(gpu_info.gpu_type)
            },
            memory_gb: 0.0, // Not directly available

            working_directory: job.current_working_directory.clone(),
            stdout_path: String::new(),
            stderr_path: String::new(),

            dependency: String::new(),
            array_job_id: job.array_job_id.value(),
            array_task_count: None,
            array_tasks_pending: None,
            array_tasks_running: None,
            array_tasks_completed: None,
        })
    }

    #[must_use]
    pub fn is_array_job(&self) -> bool {
        self.job_id.array_task_id.is_some() || self.array_job_id.is_some()
    }

    #[must_use]
    pub fn time_remaining(&self) -> Option<Duration> {
        if self.state == JobState::Running && self.time_limit_seconds > 0 {
            let remaining = self.time_limit_seconds.saturating_sub(self.elapsed_seconds);
            Some(Duration::from_secs(remaining as u64))
        } else {
            None
        }
    }

    #[must_use]
    pub fn elapsed_display(&self) -> String {
        format_duration_hms(self.elapsed_seconds as u64)
    }

    #[must_use]
    pub fn time_limit_display(&self) -> String {
        if self.time_limit_seconds == 0 {
            "-".to_string()
        } else {
            format_duration_hms(self.time_limit_seconds as u64)
        }
    }

    /// Get estimated start time display for pending jobs
    #[must_use]
    pub fn estimated_start_display(&self) -> String {
        if self.state != JobState::Pending {
            return "-".to_string();
        }

        // For pending jobs, start_time contains the estimated start
        if let Some(dt) = self.start_time.as_datetime() {
            let now = chrono::Local::now();
            let duration = dt.signed_duration_since(now);

            if duration.num_seconds() <= 0 {
                "soon".to_string()
            } else if duration.num_hours() < 1 {
                format!("~{}m", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("~{}h", duration.num_hours())
            } else {
                format!("~{}d", duration.num_days())
            }
        } else {
            "N/A".to_string()
        }
    }
}

/// Partition status summary
#[derive(Debug, Clone)]
pub struct PartitionStatus {
    pub name: String,
    pub total_nodes: u32,
    pub available_nodes: u32,
    pub down_nodes: u32,
    pub draining_nodes: u32,
    pub idle_nodes: u32,
    pub allocated_nodes: u32,
    pub mixed_nodes: u32,
    pub total_cpus: u32,
    pub allocated_cpus: u32,
    pub total_memory_gb: f64,
    pub allocated_memory_gb: f64,
    pub total_gpus: u32,
    pub allocated_gpus: u32,
    pub gpu_type: Option<String>,
    pub running_jobs: u32,
    pub pending_jobs: u32,
}

impl PartitionStatus {
    #[must_use]
    pub fn cpu_utilization(&self) -> f64 {
        if self.total_cpus == 0 {
            0.0
        } else {
            self.allocated_cpus as f64 / self.total_cpus as f64 * 100.0
        }
    }

    #[must_use]
    pub fn memory_utilization(&self) -> f64 {
        if self.total_memory_gb < 0.01 {
            0.0
        } else {
            self.allocated_memory_gb / self.total_memory_gb * 100.0
        }
    }

    #[must_use]
    pub fn gpu_utilization(&self) -> f64 {
        if self.total_gpus == 0 {
            0.0
        } else {
            self.allocated_gpus as f64 / self.total_gpus as f64 * 100.0
        }
    }
}

/// Data slice with staleness tracking
///
/// Encapsulates data with timestamp tracking. The `data` field is private to ensure
/// all updates go through `update()`, which properly sets `last_updated`.
/// Use `iter()` and `get()` for read access.
#[derive(Debug)]
pub struct DataSlice<T> {
    data: Vec<T>,
    pub last_updated: Option<Instant>,
    pub stale_threshold: Duration,
}

impl<T> Default for DataSlice<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            last_updated: None,
            stale_threshold: Duration::from_secs(30),
        }
    }
}

impl<T> DataSlice<T> {
    pub fn new(stale_threshold: Duration) -> Self {
        Self {
            data: Vec::new(),
            last_updated: None,
            stale_threshold,
        }
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.last_updated
            .map(|t| t.elapsed() > self.stale_threshold)
            .unwrap_or(true)
    }

    #[must_use]
    pub fn age(&self) -> Option<Duration> {
        self.last_updated.map(|t| t.elapsed())
    }

    pub fn update(&mut self, data: Vec<T>) {
        self.data = data;
        self.last_updated = Some(Instant::now());
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns an iterator over the data
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Returns a reference to the element at the given index, if it exists
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    /// Returns the data as a slice for efficient read-only access
    ///
    /// Use this when you need indexed access to multiple elements.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slurm_time() {
        let known = SlurmTime::from_epoch(1700000000);
        assert!(known.is_known());
        assert_eq!(known.as_epoch(), Some(1700000000));

        let unknown = SlurmTime::from_epoch(0);
        assert!(!unknown.is_known());
        assert_eq!(unknown.as_epoch(), None);
    }

    #[test]
    fn test_job_state_parsing() {
        assert_eq!(JobState::from_state_string("RUNNING"), JobState::Running);
        assert_eq!(JobState::from_state_string("PENDING"), JobState::Pending);
        assert_eq!(
            JobState::from_state_string("CANCELLED by 12345"),
            JobState::Cancelled
        );
    }
}
