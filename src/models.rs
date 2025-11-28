//! Data models for Slurm JSON responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Slurm time value structure
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TimeValue {
    pub set: bool,
    pub infinite: bool,
    pub number: u64,
}

impl TimeValue {
    pub fn to_timestamp(&self) -> Option<DateTime<Utc>> {
        if self.set && !self.infinite {
            DateTime::from_timestamp(self.number as i64, 0)
        } else {
            None
        }
    }
}

/// Slurm floating-point value structure (used by sshare for normalized values)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FloatValue {
    #[serde(default)]
    pub set: bool,
    #[serde(default)]
    pub infinite: bool,
    #[serde(default)]
    pub number: f64,
}

impl FloatValue {
    pub fn value(&self) -> Option<f64> {
        if self.set && !self.infinite {
            Some(self.number)
        } else {
            None
        }
    }
}

/// Node information from sinfo
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeInfo {
    #[serde(rename = "nodes")]
    pub node_names: NodeNames,

    #[serde(rename = "node")]
    pub node_state: NodeState,

    #[serde(rename = "partition")]
    pub partition: PartitionInfo,

    pub cpus: CpuInfo,
    pub memory: MemoryInfo,

    #[serde(default)]
    pub gres: GresInfo,

    #[serde(default)]
    pub sockets: MinMaxValue,

    #[serde(default)]
    pub cores: MinMaxValue,

    #[serde(default)]
    pub threads: MinMaxValue,

    #[serde(default)]
    pub features: FeatureInfo,

    #[serde(default)]
    pub reason: ReasonInfo,

    #[serde(default)]
    pub weight: MinMaxValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeNames {
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeState {
    #[serde(default)]
    pub state: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PartitionInfo {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CpuInfo {
    #[serde(default)]
    pub allocated: u32,
    #[serde(default)]
    pub idle: u32,
    #[serde(default)]
    pub total: u32,
    pub load: MinMaxValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryInfo {
    #[serde(default)]
    pub minimum: u64,
    #[serde(default)]
    pub allocated: u64,
    #[serde(default)]
    pub free: MemoryFreeInfo,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MemoryFreeInfo {
    pub minimum: TimeValue,
    #[serde(default)]
    pub maximum: TimeValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GresInfo {
    #[serde(default)]
    pub total: String,
    #[serde(default)]
    pub used: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MinMaxValue {
    #[serde(default)]
    pub minimum: u64,
    #[serde(default)]
    pub maximum: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FeatureInfo {
    #[serde(default)]
    pub total: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ReasonInfo {
    #[default]
    Empty,
    String(String),
    Object {
        #[serde(default)]
        description: String,
    },
}

impl ReasonInfo {
    pub fn description(&self) -> &str {
        match self {
            ReasonInfo::Empty => "",
            ReasonInfo::String(s) => s.as_str(),
            ReasonInfo::Object { description } => description.as_str(),
        }
    }
}

impl NodeInfo {
    pub fn name(&self) -> &str {
        self.node_names.nodes.first().map(|s| s.as_str()).unwrap_or("")
    }

    // Primary node states
    pub fn is_allocated(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "ALLOCATED" || s == "ALLOC")
    }

    pub fn is_completing(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "COMPLETING" || s == "COMP")
    }

    pub fn is_down(&self) -> bool {
        self.node_state.state.contains(&"DOWN".to_string())
    }

    pub fn is_drained(&self) -> bool {
        self.node_state.state.contains(&"DRAINED".to_string())
    }

    pub fn is_draining(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "DRAINING" || s == "DRAIN" || s == "DRNG")
    }

    pub fn is_fail(&self) -> bool {
        self.node_state.state.contains(&"FAIL".to_string())
    }

    pub fn is_failing(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "FAILING" || s == "FAILG")
    }

    pub fn is_future(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "FUTURE" || s == "FUTR")
    }

    pub fn is_idle(&self) -> bool {
        self.node_state.state.contains(&"IDLE".to_string())
    }

    pub fn is_maint(&self) -> bool {
        self.node_state.state.contains(&"MAINT".to_string())
    }

    pub fn is_mixed(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "MIXED" || s == "MIX")
    }

    pub fn is_perfctrs(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "PERFCTRS" || s == "NPC")
    }

    pub fn is_planned(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "PLANNED" || s == "PLND")
    }

    pub fn is_power_down(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "POWER_DOWN" || s == "POW_DN")
    }

    pub fn is_powered_down(&self) -> bool {
        self.node_state.state.contains(&"POWERED_DOWN".to_string())
    }

    pub fn is_powering_down(&self) -> bool {
        self.node_state.state.contains(&"POWERING_DOWN".to_string())
    }

    pub fn is_powering_up(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "POWERING_UP" || s == "POW_UP")
    }

    pub fn is_reserved(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "RESERVED" || s == "RESV")
    }

    pub fn is_unknown(&self) -> bool {
        self.node_state.state.iter()
            .any(|s| s == "UNKNOWN" || s == "UNK")
    }

    pub fn is_reboot_requested(&self) -> bool {
        self.node_state.state.contains(&"REBOOT_REQUESTED".to_string())
    }

    pub fn is_reboot_issued(&self) -> bool {
        self.node_state.state.contains(&"REBOOT_ISSUED".to_string())
    }

    pub fn is_inval(&self) -> bool {
        self.node_state.state.contains(&"INVAL".to_string())
    }

    pub fn is_cloud(&self) -> bool {
        self.node_state.state.contains(&"CLOUD".to_string())
    }

    pub fn is_blocked(&self) -> bool {
        self.node_state.state.contains(&"BLOCKED".to_string())
    }

    /// Get the primary node state for display
    pub fn primary_state(&self) -> &str {
        // Priority order: show most critical state first

        // Critical states
        if self.is_down() {
            return "DOWN";
        }
        if self.is_fail() {
            return "FAIL";
        }
        if self.is_failing() {
            return "FAILING";
        }
        if self.is_inval() {
            return "INVAL";
        }

        // Maintenance/administrative states
        if self.is_drained() {
            return "DRAINED";
        }
        if self.is_draining() {
            return "DRAINING";
        }
        if self.is_maint() {
            return "MAINT";
        }
        if self.is_reserved() {
            return "RESERVED";
        }

        // Reboot states
        if self.is_reboot_issued() {
            return "REBOOT_ISSUED";
        }
        if self.is_reboot_requested() {
            return "REBOOT_REQUESTED";
        }

        // Power states
        if self.is_powered_down() {
            return "POWERED_DOWN";
        }
        if self.is_powering_down() {
            return "POWERING_DOWN";
        }
        if self.is_powering_up() {
            return "POWERING_UP";
        }
        if self.is_power_down() {
            return "POWER_DOWN";
        }

        // Transitional states
        if self.is_completing() {
            return "COMPLETING";
        }
        if self.is_blocked() {
            return "BLOCKED";
        }

        // Operational states
        if self.is_allocated() {
            return "ALLOCATED";
        }
        if self.is_mixed() {
            return "MIXED";
        }
        if self.is_idle() {
            return "IDLE";
        }

        // Special states
        if self.is_perfctrs() {
            return "PERFCTRS";
        }
        if self.is_planned() {
            return "PLANNED";
        }
        if self.is_future() {
            return "FUTURE";
        }
        if self.is_cloud() {
            return "CLOUD";
        }
        if self.is_unknown() {
            return "UNKNOWN";
        }

        // Fallback
        self.node_state.state.first().map(|s| s.as_str()).unwrap_or("UNKNOWN")
    }

    /// Get node reason description
    pub fn reason_description(&self) -> &str {
        self.reason.description()
    }

    pub fn memory_total(&self) -> u64 {
        self.memory.minimum
    }

    pub fn memory_free(&self) -> u64 {
        self.memory.free.minimum.number
    }

    pub fn memory_utilization(&self) -> f64 {
        if self.memory.minimum == 0 {
            0.0
        } else {
            let free = self.memory.free.minimum.number;
            let used = self.memory.minimum.saturating_sub(free);
            (used as f64 / self.memory.minimum as f64) * 100.0
        }
    }

    /// Parse GPU information from GRES string
    pub fn gpu_info(&self) -> GpuInfo {
        let mut gpu_info = GpuInfo {
            total: 0,
            used: 0,
            gpu_type: String::new(),
        };

        // Parse total GPUs from format: "gpu:l40s:4(S:0-1)"
        if self.gres.total.contains("gpu:")
            && let Some(gpu_part) = self.gres.total.split("gpu:").nth(1)
        {
            let parts: Vec<&str> = gpu_part.split(':').collect();
            if parts.len() >= 2 {
                gpu_info.gpu_type = parts[0].to_string();
                if let Ok(count) = parts[1].split('(').next().unwrap_or("0").parse::<u32>() {
                    gpu_info.total = count;
                }
            }
        }

        // Parse used GPUs from format: "gpu:l40s:3(IDX:0-2)"
        if self.gres.used.contains("gpu:")
            && let Some(gpu_part) = self.gres.used.split("gpu:").nth(1)
        {
            let parts: Vec<&str> = gpu_part.split(':').collect();
            if parts.len() >= 2
                && let Ok(count) = parts[1].split('(').next().unwrap_or("0").parse::<u32>()
            {
                gpu_info.used = count;
            }
        }

        gpu_info
    }
}

#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub total: u32,
    pub used: u32,
    pub gpu_type: String,
}

/// Job information from squeue
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobInfo {
    pub job_id: u64,

    #[serde(default)]
    pub array_job_id: TimeValue,

    pub name: String,
    pub user_name: String,

    #[serde(default)]
    pub group_name: String,

    pub account: String,
    pub partition: String,

    #[serde(rename = "job_state")]
    pub state: Vec<String>,

    #[serde(default)]
    pub nodes: String,

    #[serde(default)]
    pub tres_alloc_str: String,

    #[serde(default)]
    pub cpus_per_task: TimeValue,

    #[serde(default)]
    pub tasks: TimeValue,

    #[serde(default)]
    pub start_time: TimeValue,

    #[serde(default)]
    pub end_time: TimeValue,

    #[serde(default)]
    pub time_limit: TimeValue,

    #[serde(default)]
    pub qos: String,

    #[serde(default)]
    pub flags: Vec<String>,

    #[serde(default)]
    pub batch_host: String,

    #[serde(default)]
    pub state_reason: String,

    #[serde(default)]
    pub priority: TimeValue,

    #[serde(default)]
    pub submit_time: TimeValue,

    #[serde(default)]
    pub current_working_directory: String,
}

impl JobInfo {
    // Base job states
    pub fn is_running(&self) -> bool {
        self.state.contains(&"RUNNING".to_string())
    }

    pub fn is_pending(&self) -> bool {
        self.state.contains(&"PENDING".to_string())
    }

    pub fn is_suspended(&self) -> bool {
        self.state.contains(&"SUSPENDED".to_string())
    }

    pub fn is_completed(&self) -> bool {
        self.state.contains(&"COMPLETED".to_string())
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.contains(&"CANCELLED".to_string())
    }

    pub fn is_failed(&self) -> bool {
        self.state.contains(&"FAILED".to_string())
    }

    pub fn is_timeout(&self) -> bool {
        self.state.contains(&"TIMEOUT".to_string())
    }

    pub fn is_node_fail(&self) -> bool {
        self.state.contains(&"NODE_FAIL".to_string())
    }

    pub fn is_preempted(&self) -> bool {
        self.state.contains(&"PREEMPTED".to_string())
    }

    pub fn is_boot_fail(&self) -> bool {
        self.state.contains(&"BOOT_FAIL".to_string())
    }

    pub fn is_deadline(&self) -> bool {
        self.state.contains(&"DEADLINE".to_string())
    }

    pub fn is_out_of_memory(&self) -> bool {
        self.state.contains(&"OUT_OF_MEMORY".to_string())
    }

    // Job state flags
    pub fn is_completing(&self) -> bool {
        self.state.contains(&"COMPLETING".to_string())
    }

    pub fn is_configuring(&self) -> bool {
        self.state.contains(&"CONFIGURING".to_string())
    }

    pub fn is_requeued(&self) -> bool {
        self.state.contains(&"REQUEUED".to_string())
            || self.state.contains(&"REQUEUE_FED".to_string())
            || self.state.contains(&"REQUEUE_HOLD".to_string())
    }

    pub fn is_resizing(&self) -> bool {
        self.state.contains(&"RESIZING".to_string())
    }

    pub fn is_signaling(&self) -> bool {
        self.state.contains(&"SIGNALING".to_string())
    }

    pub fn is_stage_out(&self) -> bool {
        self.state.contains(&"STAGE_OUT".to_string())
    }

    pub fn is_stopped(&self) -> bool {
        self.state.contains(&"STOPPED".to_string())
    }

    /// Get the primary job state for display
    pub fn primary_state(&self) -> &str {
        // Check flags first (they take precedence in display)
        if self.is_completing() {
            return "COMPLETING";
        }
        if self.is_configuring() {
            return "CONFIGURING";
        }
        if self.is_stage_out() {
            return "STAGE_OUT";
        }
        if self.is_signaling() {
            return "SIGNALING";
        }
        if self.is_resizing() {
            return "RESIZING";
        }
        if self.is_stopped() {
            return "STOPPED";
        }
        if self.is_requeued() {
            return "REQUEUED";
        }

        // Then check base states
        if self.is_running() {
            return "RUNNING";
        }
        if self.is_pending() {
            return "PENDING";
        }
        if self.is_suspended() {
            return "SUSPENDED";
        }
        if self.is_completed() {
            return "COMPLETED";
        }
        if self.is_cancelled() {
            return "CANCELLED";
        }
        if self.is_failed() {
            return "FAILED";
        }
        if self.is_timeout() {
            return "TIMEOUT";
        }
        if self.is_node_fail() {
            return "NODE_FAIL";
        }
        if self.is_preempted() {
            return "PREEMPTED";
        }
        if self.is_boot_fail() {
            return "BOOT_FAIL";
        }
        if self.is_deadline() {
            return "DEADLINE";
        }
        if self.is_out_of_memory() {
            return "OUT_OF_MEMORY";
        }

        // Fallback to first state in array
        self.state.first().map(|s| s.as_str()).unwrap_or("UNKNOWN")
    }

    pub fn is_array_job(&self) -> bool {
        self.array_job_id.number != 0
    }

    /// Parse allocated resources from TRES string
    pub fn allocated_resources(&self) -> HashMap<String, String> {
        let mut resources = HashMap::new();

        for item in self.tres_alloc_str.split(',') {
            if let Some((key, value)) = item.split_once('=') {
                resources.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        resources
    }

    /// Get number of allocated GPUs
    pub fn allocated_gpus(&self) -> u32 {
        let resources = self.allocated_resources();

        for (key, value) in resources.iter() {
            if key.contains("gres/gpu")
                && let Ok(count) = value.parse::<u32>()
            {
                return count;
            }
        }

        0
    }

    /// Parse GPU type information
    pub fn gpu_type_info(&self) -> JobGpuInfo {
        let resources = self.allocated_resources();
        let mut gpu_info = JobGpuInfo {
            count: 0,
            gpu_type: String::new(),
            display: "-".to_string(),
        };

        // Look for specific GPU type allocations like "gres/gpu:l40s"
        for (key, value) in resources.iter() {
            if key.contains("gres/gpu:")
                && let Some(gpu_type) = key.split("gres/gpu:").nth(1)
                && let Ok(count) = value.parse::<u32>()
            {
                gpu_info.count = count;
                gpu_info.gpu_type = gpu_type.to_uppercase();
                gpu_info.display = format!("{}x{}", count, gpu_type.to_uppercase());
                return gpu_info;
            }
        }

        // Fallback to generic GPU count
        let gpu_count = self.allocated_gpus();
        if gpu_count > 0 {
            gpu_info.count = gpu_count;
            gpu_info.display = gpu_count.to_string();
        }

        gpu_info
    }

    /// Calculate remaining time in minutes
    pub fn remaining_time_minutes(&self) -> Option<i64> {
        if !self.time_limit.set || !self.start_time.set {
            return None;
        }

        let now = Utc::now().timestamp();
        let elapsed = now - self.start_time.number as i64;
        let elapsed_minutes = elapsed / 60;

        let time_limit_minutes = self.time_limit.number as i64;
        let remaining = time_limit_minutes - elapsed_minutes;

        Some(remaining.max(0))
    }

    /// Format remaining time for display
    pub fn remaining_time_display(&self) -> String {
        match self.remaining_time_minutes() {
            None => "-".to_string(),
            Some(0) => "0m".to_string(),
            Some(remaining) if remaining < 60 => format!("{}m", remaining),
            Some(remaining) if remaining < 1440 => {
                let hours = remaining / 60;
                let minutes = remaining % 60;
                if minutes == 0 {
                    format!("{}h", hours)
                } else {
                    format!("{}h {}m", hours, minutes)
                }
            }
            Some(remaining) => {
                let days = remaining / 1440;
                let hours = (remaining % 1440) / 60;
                if hours == 0 {
                    format!("{}d", days)
                } else {
                    format!("{}d {}h", days, hours)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct JobGpuInfo {
    pub count: u32,
    pub gpu_type: String,
    pub display: String,
}

/// Slurm API response wrapper for sinfo
#[derive(Debug, Deserialize, Serialize)]
pub struct SinfoResponse {
    #[serde(default)]
    pub sinfo: Vec<NodeInfo>,

    #[serde(default)]
    pub errors: Vec<String>,
}

/// Slurm API response wrapper for squeue
#[derive(Debug, Deserialize, Serialize)]
pub struct SqueueResponse {
    #[serde(default)]
    pub jobs: Vec<JobInfo>,

    #[serde(default)]
    pub errors: Vec<String>,
}

/// Overall cluster status
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    pub nodes: Vec<NodeInfo>,
    pub jobs: Vec<JobInfo>,
}

impl ClusterStatus {
    pub fn total_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn idle_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_idle()).count()
    }

    pub fn down_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_down()).count()
    }

    pub fn mixed_nodes(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_mixed()).count()
    }

    pub fn total_cpus(&self) -> u32 {
        self.nodes.iter().map(|n| n.cpus.total).sum()
    }

    pub fn allocated_cpus(&self) -> u32 {
        self.nodes.iter().map(|n| n.cpus.allocated).sum()
    }

    pub fn cpu_utilization(&self) -> f64 {
        let total = self.total_cpus();
        if total == 0 {
            0.0
        } else {
            (self.allocated_cpus() as f64 / total as f64) * 100.0
        }
    }
}

/// Job history information from sacct
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobHistoryInfo {
    pub job_id: u64,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub user: String,

    #[serde(default)]
    pub account: String,

    #[serde(default)]
    pub partition: String,

    #[serde(default)]
    pub state: JobHistoryState,

    #[serde(default)]
    pub exit_code: ExitCodeInfo,

    #[serde(default)]
    pub derived_exit_code: ExitCodeInfo,

    #[serde(default)]
    pub nodes: String,

    #[serde(default)]
    pub time: JobTimeInfo,

    #[serde(default)]
    pub required: JobRequiredResources,

    #[serde(default)]
    pub tres: JobTresInfo,

    #[serde(default)]
    pub steps: Vec<JobStepInfo>,

    #[serde(default)]
    pub submit_line: String,

    #[serde(default)]
    pub working_directory: String,

    #[serde(default)]
    pub stdout: String,

    #[serde(default)]
    pub stderr: String,

    #[serde(default)]
    pub group: String,

    #[serde(default)]
    pub cluster: String,

    #[serde(default)]
    pub qos: String,

    #[serde(default)]
    pub priority: TimeValue,

    #[serde(default)]
    pub association: JobAssociation,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobHistoryState {
    #[serde(default)]
    pub current: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExitCodeInfo {
    #[serde(default)]
    pub status: Vec<String>,
    #[serde(default)]
    pub return_code: TimeValue,
    #[serde(default)]
    pub signal: SignalInfo,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SignalInfo {
    #[serde(default)]
    pub id: TimeValue,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobTimeInfo {
    #[serde(default)]
    pub elapsed: u64,
    #[serde(default)]
    pub eligible: u64,
    #[serde(default)]
    pub end: u64,
    #[serde(default)]
    pub start: u64,
    #[serde(default)]
    pub submission: u64,
    #[serde(default)]
    pub suspended: u64,
    #[serde(default)]
    pub limit: TimeValue,
    #[serde(default)]
    pub system: TimeSeconds,
    #[serde(default)]
    pub user: TimeSeconds,
    #[serde(default)]
    pub total: TimeSeconds,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TimeSeconds {
    #[serde(default)]
    pub seconds: u64,
    #[serde(default)]
    pub microseconds: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobRequiredResources {
    #[serde(rename = "CPUs")]
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory_per_cpu: TimeValue,
    #[serde(default)]
    pub memory_per_node: TimeValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobTresInfo {
    #[serde(default)]
    pub allocated: Vec<TresItem>,
    #[serde(default)]
    pub requested: Vec<TresItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TresItem {
    #[serde(default, rename = "type")]
    pub tres_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub count: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobStepInfo {
    #[serde(default)]
    pub time: JobStepTimeInfo,
    #[serde(default)]
    pub exit_code: ExitCodeInfo,
    #[serde(default)]
    pub statistics: Option<JobStepStatistics>,
    #[serde(default)]
    pub step: StepIdInfo,
    #[serde(default)]
    pub tasks: TasksInfo,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobStepTimeInfo {
    #[serde(default)]
    pub elapsed: u64,
    #[serde(default)]
    pub start: TimeValue,
    #[serde(default)]
    pub end: TimeValue,
    #[serde(default)]
    pub system: TimeSeconds,
    #[serde(default)]
    pub user: TimeSeconds,
    #[serde(default)]
    pub total: TimeSeconds,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobStepStatistics {
    #[serde(rename = "CPU")]
    #[serde(default)]
    pub cpu: Option<CpuStatistics>,
    #[serde(default)]
    pub memory: Option<MemoryStatistics>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CpuStatistics {
    #[serde(default)]
    pub actual_frequency: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MemoryStatistics {
    #[serde(default)]
    pub max: MemoryMaxInfo,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MemoryMaxInfo {
    #[serde(default)]
    pub task: MemoryTaskInfo,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MemoryTaskInfo {
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StepIdInfo {
    #[serde(default)]
    pub id: StepId,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StepId {
    #[default]
    Unknown,
    Number(u64),
    Name(String),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TasksInfo {
    #[serde(default)]
    pub count: u32,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JobAssociation {
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub cluster: String,
    #[serde(default)]
    pub partition: String,
    #[serde(default)]
    pub user: String,
}

impl JobHistoryInfo {
    /// Get primary state string
    pub fn primary_state(&self) -> &str {
        self.state.current.first().map(|s| s.as_str()).unwrap_or("UNKNOWN")
    }

    /// Check if job completed successfully
    pub fn is_completed(&self) -> bool {
        self.state.current.iter().any(|s| s == "COMPLETED")
    }

    /// Check if job failed
    pub fn is_failed(&self) -> bool {
        self.state.current.iter().any(|s| s == "FAILED")
    }

    /// Check if job timed out
    pub fn is_timeout(&self) -> bool {
        self.state.current.iter().any(|s| s == "TIMEOUT")
    }

    /// Check if job was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.state.current.iter().any(|s| s == "CANCELLED" || s.starts_with("CANCELLED"))
    }

    /// Check if job ran out of memory
    pub fn is_out_of_memory(&self) -> bool {
        self.state.current.iter().any(|s| s == "OUT_OF_MEMORY")
    }

    /// Check if job is running
    pub fn is_running(&self) -> bool {
        self.state.current.iter().any(|s| s == "RUNNING")
    }

    /// Check if job is pending
    pub fn is_pending(&self) -> bool {
        self.state.current.iter().any(|s| s == "PENDING")
    }

    /// Get elapsed time in human-readable format
    pub fn elapsed_display(&self) -> String {
        format_duration_seconds(self.time.elapsed)
    }

    /// Get time limit in human-readable format
    pub fn time_limit_display(&self) -> String {
        if self.time.limit.set && !self.time.limit.infinite {
            format_duration_minutes(self.time.limit.number)
        } else if self.time.limit.infinite {
            "UNLIMITED".to_string()
        } else {
            "-".to_string()
        }
    }

    /// Calculate CPU efficiency percentage
    pub fn cpu_efficiency(&self) -> Option<f64> {
        let elapsed = self.time.elapsed;
        let cpus = self.required.cpus;

        if elapsed == 0 || cpus == 0 {
            return None;
        }

        // Total CPU time available (wall time * CPUs)
        let total_cpu_time = elapsed as f64 * cpus as f64;

        // Actual CPU time used (user + system)
        let used_cpu_time = self.time.total.seconds as f64 +
            (self.time.total.microseconds as f64 / 1_000_000.0);

        if total_cpu_time > 0.0 {
            Some((used_cpu_time / total_cpu_time) * 100.0)
        } else {
            None
        }
    }

    /// Get maximum memory used from steps
    pub fn max_memory_used(&self) -> u64 {
        self.steps.iter()
            .filter_map(|step| step.statistics.as_ref())
            .filter_map(|stats| stats.memory.as_ref())
            .map(|mem| mem.max.task.bytes)
            .max()
            .unwrap_or(0)
    }

    /// Get requested memory in bytes
    pub fn requested_memory(&self) -> u64 {
        // Memory per node takes precedence
        if self.required.memory_per_node.set && self.required.memory_per_node.number > 0 {
            // Memory is in MB
            return self.required.memory_per_node.number * 1024 * 1024;
        }

        // Fall back to memory per CPU * CPUs
        if self.required.memory_per_cpu.set && self.required.memory_per_cpu.number > 0 {
            return self.required.memory_per_cpu.number * self.required.cpus as u64 * 1024 * 1024;
        }

        // Try to get from TRES
        for tres in &self.tres.requested {
            if tres.tres_type == "mem" {
                return tres.count * 1024 * 1024; // Assuming MB
            }
        }

        0
    }

    /// Calculate memory efficiency percentage
    pub fn memory_efficiency(&self) -> Option<f64> {
        let max_used = self.max_memory_used();
        let requested = self.requested_memory();

        if requested > 0 && max_used > 0 {
            Some((max_used as f64 / requested as f64) * 100.0)
        } else {
            None
        }
    }

    /// Get number of allocated GPUs
    pub fn allocated_gpus(&self) -> u32 {
        for tres in &self.tres.allocated {
            if tres.tres_type == "gres" && tres.name.starts_with("gpu") {
                return tres.count as u32;
            }
        }
        0
    }

    /// Get GPU type
    pub fn gpu_type(&self) -> Option<String> {
        for tres in &self.tres.allocated {
            if tres.tres_type == "gres" && tres.name.contains(':') {
                if let Some(gpu_type) = tres.name.split(':').nth(1) {
                    return Some(gpu_type.to_uppercase());
                }
            }
        }
        None
    }

    /// Get exit code as string
    pub fn exit_code_display(&self) -> String {
        if self.exit_code.return_code.set {
            format!("{}", self.exit_code.return_code.number)
        } else if !self.exit_code.signal.name.is_empty() {
            format!("SIG{}", self.exit_code.signal.name)
        } else {
            "-".to_string()
        }
    }

    /// Get submit time as formatted string
    pub fn submit_time_display(&self) -> String {
        if self.time.submission > 0 {
            if let Some(dt) = DateTime::from_timestamp(self.time.submission as i64, 0) {
                return dt.format("%Y-%m-%d %H:%M:%S").to_string();
            }
        }
        "-".to_string()
    }

    /// Get start time as formatted string
    pub fn start_time_display(&self) -> String {
        if self.time.start > 0 {
            if let Some(dt) = DateTime::from_timestamp(self.time.start as i64, 0) {
                return dt.format("%Y-%m-%d %H:%M:%S").to_string();
            }
        }
        "-".to_string()
    }

    /// Get end time as formatted string
    pub fn end_time_display(&self) -> String {
        if self.time.end > 0 {
            if let Some(dt) = DateTime::from_timestamp(self.time.end as i64, 0) {
                return dt.format("%Y-%m-%d %H:%M:%S").to_string();
            }
        }
        "-".to_string()
    }

    /// Get wait time (time between submission and start)
    pub fn wait_time(&self) -> Option<u64> {
        if self.time.submission > 0 && self.time.start > 0 && self.time.start >= self.time.submission {
            Some(self.time.start - self.time.submission)
        } else {
            None
        }
    }

    /// Get wait time display
    pub fn wait_time_display(&self) -> String {
        self.wait_time()
            .map(format_duration_seconds)
            .unwrap_or_else(|| "-".to_string())
    }
}

/// Format duration from seconds to human-readable
pub fn format_duration_seconds(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        if secs > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}m", minutes)
        }
    } else {
        format!("{}s", secs)
    }
}

/// Format duration from minutes to human-readable
pub fn format_duration_minutes(minutes: u64) -> String {
    format_duration_seconds(minutes * 60)
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

// ============================================================================
// Fairshare/sshare models (Phase 4)
// ============================================================================

/// Slurm API response wrapper for sshare
#[derive(Debug, Deserialize, Serialize)]
pub struct SshareResponse {
    #[serde(default)]
    pub shares: SshareWrapper,

    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareWrapper {
    #[serde(default)]
    pub shares: Vec<SshareEntry>,
}

/// Individual fairshare entry from sshare
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SshareEntry {
    #[serde(default)]
    pub id: u32,

    #[serde(default)]
    pub cluster: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub parent: String,

    #[serde(default)]
    pub partition: String,

    /// Normalized shares (floating-point)
    #[serde(default)]
    pub shares_normalized: FloatValue,

    /// Raw shares (can be float in Slurm 24+)
    #[serde(default)]
    pub shares: FloatValue,

    #[serde(default)]
    pub tres: SshareTres,

    /// Raw usage value (integer)
    #[serde(default)]
    pub usage: u64,

    #[serde(default)]
    pub fairshare: SshareFairshare,

    /// Effective usage (floating-point)
    #[serde(default)]
    pub effective_usage: FloatValue,

    /// Normalized usage (floating-point)
    #[serde(default)]
    pub usage_normalized: FloatValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareTres {
    #[serde(default)]
    pub run_seconds: Vec<SshareTresItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareTresItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: TimeValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareFairshare {
    /// Fairshare factor (floating-point value struct)
    #[serde(default)]
    pub factor: FloatValue,
    /// Fairshare level (floating-point value struct)
    #[serde(default)]
    pub level: FloatValue,
}

impl SshareEntry {
    /// Get shares as a fraction (0.0 to 1.0)
    pub fn shares_fraction(&self) -> f64 {
        if self.shares_normalized.set {
            self.shares_normalized.number
        } else {
            0.0
        }
    }

    /// Get fairshare factor (0.0 to 1.0, higher is better)
    pub fn fairshare_factor(&self) -> f64 {
        if self.fairshare.factor.set {
            self.fairshare.factor.number
        } else {
            0.0
        }
    }

    /// Check if this is a user entry (has a parent that's an account)
    pub fn is_user(&self) -> bool {
        !self.parent.is_empty() && self.parent != "root"
    }

    /// Get CPU hours from TRES run_seconds
    pub fn cpu_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "cpu" && item.value.set {
                return item.value.number as f64 / 3600.0;
            }
        }
        0.0
    }

    /// Get GPU hours from TRES run_seconds
    pub fn gpu_hours(&self) -> f64 {
        let mut total = 0.0;
        for item in &self.tres.run_seconds {
            if item.name.starts_with("gres/gpu") && item.value.set {
                total += item.value.number as f64 / 3600.0;
            }
        }
        total
    }

    /// Get memory GB-hours from TRES run_seconds
    pub fn mem_gb_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "mem" && item.value.set {
                // Memory is in MB-seconds, convert to GB-hours
                return item.value.number as f64 / (1024.0 * 3600.0);
            }
        }
        0.0
    }
}

/// Fairshare tree node for hierarchical display
#[derive(Debug, Clone)]
pub struct FairshareNode {
    pub name: String,
    pub parent: String,
    pub depth: usize,
    pub is_user: bool,
    pub is_current_user: bool,
    pub shares_percent: f64,
    pub fairshare_factor: f64,
    pub cpu_hours: f64,
    pub gpu_hours: f64,
    pub children: Vec<FairshareNode>,
}

impl FairshareNode {
    /// Build a tree from flat sshare entries
    pub fn build_tree(entries: &[SshareEntry], current_user: &str) -> Vec<FairshareNode> {
        let mut root_nodes = Vec::new();

        // Find all root-level entries (parent is "root" or empty)
        for entry in entries {
            if entry.parent == "root" || entry.parent.is_empty() {
                let node = Self::build_node(entry, entries, 0, current_user);
                root_nodes.push(node);
            }
        }

        root_nodes
    }

    fn build_node(entry: &SshareEntry, all_entries: &[SshareEntry], depth: usize, current_user: &str) -> Self {
        let mut node = FairshareNode {
            name: entry.name.clone(),
            parent: entry.parent.clone(),
            depth,
            is_user: entry.is_user(),
            is_current_user: entry.name == current_user,
            shares_percent: entry.shares_fraction() * 100.0,
            fairshare_factor: entry.fairshare_factor(),
            cpu_hours: entry.cpu_hours(),
            gpu_hours: entry.gpu_hours(),
            children: Vec::new(),
        };

        // Find children (entries whose parent matches this name)
        for child_entry in all_entries {
            if child_entry.parent == entry.name {
                let child_node = Self::build_node(child_entry, all_entries, depth + 1, current_user);
                node.children.push(child_node);
            }
        }

        node
    }

    /// Flatten the tree for display with proper indentation
    pub fn flatten(&self) -> Vec<FlatFairshareRow> {
        let mut rows = Vec::new();
        self.flatten_recursive(&mut rows);
        rows
    }

    fn flatten_recursive(&self, rows: &mut Vec<FlatFairshareRow>) {
        rows.push(FlatFairshareRow {
            name: self.name.clone(),
            depth: self.depth,
            is_user: self.is_user,
            is_current_user: self.is_current_user,
            shares_percent: self.shares_percent,
            fairshare_factor: self.fairshare_factor,
            cpu_hours: self.cpu_hours,
            gpu_hours: self.gpu_hours,
            has_children: !self.children.is_empty(),
        });

        for child in &self.children {
            child.flatten_recursive(rows);
        }
    }
}

/// Flattened fairshare row for table display
#[derive(Debug, Clone)]
pub struct FlatFairshareRow {
    pub name: String,
    pub depth: usize,
    pub is_user: bool,
    pub is_current_user: bool,
    pub shares_percent: f64,
    pub fairshare_factor: f64,
    pub cpu_hours: f64,
    pub gpu_hours: f64,
    pub has_children: bool,
}

impl FlatFairshareRow {
    /// Get display name with tree indentation
    pub fn display_name(&self) -> String {
        let indent = "  ".repeat(self.depth);
        let prefix = if self.has_children { "+" } else { "-" };
        format!("{}{} {}", indent, prefix, self.name)
    }
}

// ============================================================================
// Scheduler stats (sdiag) models (Phase 4)
// ============================================================================

/// Scheduler statistics from sdiag
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    /// Jobs currently in queue
    pub jobs_pending: Option<u64>,
    pub jobs_running: Option<u64>,

    /// Main scheduler cycle times (microseconds)
    pub last_cycle_us: Option<u64>,
    pub mean_cycle_us: Option<u64>,
    pub max_cycle_us: Option<u64>,

    /// Backfill statistics
    pub backfill_last_cycle_us: Option<u64>,
    pub backfill_queue_length: Option<u64>,
    pub backfill_last_depth: Option<u64>,
    pub backfill_total_jobs_since_start: Option<u64>,

    /// When this was fetched
    pub fetched_at: Option<std::time::Instant>,

    /// Whether sdiag is available (permission may be denied)
    pub available: bool,
}

impl SchedulerStats {
    /// Parse sdiag text output
    pub fn from_sdiag_output(output: &str) -> Self {
        let mut stats = SchedulerStats {
            available: true,
            fetched_at: Some(std::time::Instant::now()),
            ..Default::default()
        };

        for line in output.lines() {
            let line = line.trim();

            // Main scheduler stats
            if line.starts_with("Last cycle:") {
                stats.last_cycle_us = Self::parse_microseconds(line);
            } else if line.starts_with("Mean cycle:") {
                stats.mean_cycle_us = Self::parse_microseconds(line);
            } else if line.starts_with("Max cycle:") {
                stats.max_cycle_us = Self::parse_microseconds(line);
            } else if line.starts_with("Jobs pending:") {
                stats.jobs_pending = Self::parse_number(line);
            } else if line.starts_with("Jobs running:") {
                stats.jobs_running = Self::parse_number(line);
            }
            // Backfill stats
            else if line.contains("Backfill") && line.contains("Last cycle") {
                stats.backfill_last_cycle_us = Self::parse_microseconds(line);
            } else if line.contains("Backfill") && line.contains("queue length") {
                stats.backfill_queue_length = Self::parse_number(line);
            } else if line.contains("Backfill") && line.contains("depth") {
                stats.backfill_last_depth = Self::parse_number(line);
            } else if line.contains("Total backfilled jobs") {
                stats.backfill_total_jobs_since_start = Self::parse_number(line);
            }
        }

        stats
    }

    fn parse_microseconds(line: &str) -> Option<u64> {
        // Parse lines like "Last cycle:   1234 microseconds"
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "microseconds" || part.starts_with("microsec") {
                if i > 0 {
                    return parts[i - 1].parse().ok();
                }
            }
        }
        None
    }

    fn parse_number(line: &str) -> Option<u64> {
        // Parse lines like "Jobs pending:  1234"
        if let Some((_prefix, suffix)) = line.split_once(':') {
            return suffix.trim().split_whitespace().next()?.parse().ok();
        }
        None
    }

    /// Check if scheduler is healthy (mean cycle < 5 seconds)
    pub fn is_healthy(&self) -> Option<bool> {
        self.mean_cycle_us.map(|us| us < 5_000_000)
    }

    /// Format mean cycle for display
    pub fn mean_cycle_display(&self) -> String {
        match self.mean_cycle_us {
            Some(us) if us < 1000 => format!("{}us", us),
            Some(us) if us < 1_000_000 => format!("{:.1}ms", us as f64 / 1000.0),
            Some(us) => format!("{:.1}s", us as f64 / 1_000_000.0),
            None => "N/A".to_string(),
        }
    }
}

// ============================================================================
// Configuration models (Phase 4)
// ============================================================================

/// TUI configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TuiConfig {
    #[serde(default)]
    pub refresh: RefreshConfig,

    #[serde(default)]
    pub display: DisplayConfig,

    #[serde(default)]
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefreshConfig {
    /// Jobs refresh interval in seconds
    #[serde(default = "default_jobs_interval")]
    pub jobs_interval: u64,

    /// Nodes refresh interval in seconds
    #[serde(default = "default_nodes_interval")]
    pub nodes_interval: u64,

    /// Fairshare refresh interval in seconds
    #[serde(default = "default_fairshare_interval")]
    pub fairshare_interval: u64,

    /// Enable idle slowdown
    #[serde(default = "default_true")]
    pub idle_slowdown: bool,

    /// Seconds before considered idle
    #[serde(default = "default_idle_threshold")]
    pub idle_threshold: u64,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DisplayConfig {
    /// Default view on startup
    #[serde(default = "default_view")]
    pub default_view: String,

    /// Show all jobs by default
    #[serde(default)]
    pub show_all_jobs: bool,

    /// Start with grouped-by-account mode
    #[serde(default)]
    pub show_grouped_by_account: bool,

    /// Theme name
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_view: "jobs".to_string(),
            show_all_jobs: false,
            show_grouped_by_account: false,
            theme: "dark".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorConfig {
    /// Require confirmation before cancelling jobs
    #[serde(default = "default_true")]
    pub confirm_cancel: bool,

    /// Enable clipboard support
    #[serde(default = "default_true")]
    pub copy_to_clipboard: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            confirm_cancel: true,
            copy_to_clipboard: true,
        }
    }
}

fn default_jobs_interval() -> u64 { 5 }
fn default_nodes_interval() -> u64 { 10 }
fn default_fairshare_interval() -> u64 { 60 }
fn default_idle_threshold() -> u64 { 30 }
fn default_true() -> bool { true }
fn default_view() -> String { "jobs".to_string() }
fn default_theme() -> String { "dark".to_string() }

impl TuiConfig {
    /// Load configuration from files and environment
    pub fn load() -> Self {
        let mut config = Self::default();

        // Try to load from /etc/cmon/config.toml
        if let Ok(content) = std::fs::read_to_string("/etc/cmon/config.toml") {
            if let Ok(site) = toml::from_str::<TuiConfig>(&content) {
                config.merge(site);
            }
        }

        // Try to load from ~/.config/cmon/config.toml
        if let Some(home) = std::env::var_os("HOME") {
            let user_path = std::path::PathBuf::from(home)
                .join(".config/cmon/config.toml");
            if let Ok(content) = std::fs::read_to_string(&user_path) {
                if let Ok(user) = toml::from_str::<TuiConfig>(&content) {
                    config.merge(user);
                }
            }
        }

        // Apply environment overrides
        config.apply_env_overrides();

        config
    }

    fn merge(&mut self, other: TuiConfig) {
        self.refresh = other.refresh;
        self.display = other.display;
        self.behavior = other.behavior;
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("CMON_REFRESH_JOBS") {
            if let Ok(secs) = val.parse() {
                self.refresh.jobs_interval = secs;
            }
        }
        if let Ok(val) = std::env::var("CMON_REFRESH_NODES") {
            if let Ok(secs) = val.parse() {
                self.refresh.nodes_interval = secs;
            }
        }
        if let Ok(val) = std::env::var("CMON_DEFAULT_VIEW") {
            self.display.default_view = val;
        }
        if let Ok(val) = std::env::var("CMON_THEME") {
            self.display.theme = val;
        }
        if std::env::var("CMON_NO_CLIPBOARD").is_ok() {
            self.behavior.copy_to_clipboard = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a test node with specific states
    fn create_test_node(states: Vec<&str>) -> NodeInfo {
        NodeInfo {
            node_names: NodeNames {
                nodes: vec!["testnode".to_string()],
            },
            node_state: NodeState {
                state: states.iter().map(|s| s.to_string()).collect(),
            },
            partition: PartitionInfo { name: Some("test".to_string()) },
            cpus: CpuInfo {
                allocated: 0,
                idle: 128,
                total: 128,
                load: MinMaxValue { minimum: 0, maximum: 0 },
            },
            memory: MemoryInfo {
                minimum: 1024000,
                allocated: 0,
                free: MemoryFreeInfo {
                    minimum: TimeValue { set: true, infinite: false, number: 1024000 },
                    maximum: TimeValue { set: true, infinite: false, number: 1024000 },
                },
            },
            gres: GresInfo {
                total: String::new(),
                used: String::new(),
            },
            sockets: MinMaxValue { minimum: 2, maximum: 2 },
            cores: MinMaxValue { minimum: 32, maximum: 32 },
            threads: MinMaxValue { minimum: 64, maximum: 64 },
            features: FeatureInfo { total: String::new() },
            reason: ReasonInfo::Empty,
            weight: MinMaxValue { minimum: 1, maximum: 1 },
        }
    }

    // Helper to create a test job with specific states
    fn create_test_job(states: Vec<&str>, reason: &str) -> JobInfo {
        JobInfo {
            job_id: 12345,
            array_job_id: TimeValue::default(),
            name: "test_job".to_string(),
            user_name: "testuser".to_string(),
            group_name: "testgroup".to_string(),
            account: "testacct".to_string(),
            partition: "cpu".to_string(),
            state: states.iter().map(|s| s.to_string()).collect(),
            nodes: String::new(),
            tres_alloc_str: String::new(),
            cpus_per_task: TimeValue { set: true, infinite: false, number: 1 },
            tasks: TimeValue { set: true, infinite: false, number: 8 },
            start_time: TimeValue::default(),
            end_time: TimeValue::default(),
            time_limit: TimeValue::default(),
            qos: String::new(),
            flags: vec![],
            batch_host: String::new(),
            state_reason: reason.to_string(),
            priority: TimeValue::default(),
            submit_time: TimeValue::default(),
            current_working_directory: String::new(),
        }
    }

    #[test]
    fn test_node_state_idle() {
        let node = create_test_node(vec!["IDLE"]);
        assert!(node.is_idle());
        assert!(!node.is_down());
        assert!(!node.is_mixed());
        assert_eq!(node.primary_state(), "IDLE");
    }

    #[test]
    fn test_node_state_down() {
        let node = create_test_node(vec!["DOWN"]);
        assert!(node.is_down());
        assert!(!node.is_idle());
        assert_eq!(node.primary_state(), "DOWN");
    }

    #[test]
    fn test_node_state_mixed() {
        let node = create_test_node(vec!["MIXED"]);
        assert!(node.is_mixed());
        assert!(!node.is_idle());
        assert!(!node.is_down());
        assert_eq!(node.primary_state(), "MIXED");
    }

    #[test]
    fn test_node_state_allocated() {
        let node = create_test_node(vec!["ALLOCATED"]);
        assert!(node.is_allocated());
        assert_eq!(node.primary_state(), "ALLOCATED");
    }

    #[test]
    fn test_node_state_allocated_abbrev() {
        let node = create_test_node(vec!["ALLOC"]);
        assert!(node.is_allocated());
        assert_eq!(node.primary_state(), "ALLOCATED");
    }

    #[test]
    fn test_node_state_draining() {
        let node = create_test_node(vec!["DRAINING"]);
        assert!(node.is_draining());
        assert_eq!(node.primary_state(), "DRAINING");
    }

    #[test]
    fn test_node_state_draining_abbrev() {
        let node = create_test_node(vec!["DRAIN", "DRNG"]);
        assert!(node.is_draining());
        assert_eq!(node.primary_state(), "DRAINING");
    }

    #[test]
    fn test_node_state_drained() {
        let node = create_test_node(vec!["DRAINED"]);
        assert!(node.is_drained());
        assert!(!node.is_draining());
        assert_eq!(node.primary_state(), "DRAINED");
    }

    #[test]
    fn test_node_state_compound_idle_drain() {
        // When a node has both IDLE and DRAIN, it's DRAINING
        let node = create_test_node(vec!["IDLE", "DRAIN"]);
        assert!(node.is_idle());
        assert!(node.is_draining());
        // DRAINING has higher priority than IDLE
        assert_eq!(node.primary_state(), "DRAINING");
    }

    #[test]
    fn test_node_state_priority_down_over_idle() {
        let node = create_test_node(vec!["DOWN", "IDLE"]);
        assert!(node.is_down());
        assert!(node.is_idle());
        // DOWN should take priority
        assert_eq!(node.primary_state(), "DOWN");
    }

    #[test]
    fn test_node_state_maint() {
        let node = create_test_node(vec!["MAINT"]);
        assert!(node.is_maint());
        assert_eq!(node.primary_state(), "MAINT");
    }

    #[test]
    fn test_node_state_reserved() {
        let node = create_test_node(vec!["RESERVED"]);
        assert!(node.is_reserved());
        assert_eq!(node.primary_state(), "RESERVED");
    }

    #[test]
    fn test_node_state_power() {
        let powered_down = create_test_node(vec!["POWERED_DOWN"]);
        assert!(powered_down.is_powered_down());
        assert_eq!(powered_down.primary_state(), "POWERED_DOWN");

        let powering_up = create_test_node(vec!["POWERING_UP"]);
        assert!(powering_up.is_powering_up());
        assert_eq!(powering_up.primary_state(), "POWERING_UP");
    }

    #[test]
    fn test_node_gpu_parsing() {
        let mut node = create_test_node(vec!["IDLE"]);
        node.gres.total = "gpu:l40s:4(S:0-1)".to_string();
        node.gres.used = "gpu:l40s:3(IDX:0-2)".to_string();

        let gpu_info = node.gpu_info();
        assert_eq!(gpu_info.total, 4);
        assert_eq!(gpu_info.used, 3);
        assert_eq!(gpu_info.gpu_type, "l40s");
    }

    #[test]
    fn test_job_state_running() {
        let job = create_test_job(vec!["RUNNING"], "None");
        assert!(job.is_running());
        assert!(!job.is_pending());
        assert_eq!(job.primary_state(), "RUNNING");
    }

    #[test]
    fn test_job_state_pending() {
        let job = create_test_job(vec!["PENDING"], "Resources");
        assert!(job.is_pending());
        assert!(!job.is_running());
        assert_eq!(job.primary_state(), "PENDING");
        assert_eq!(job.state_reason, "Resources");
    }

    #[test]
    fn test_job_state_completing() {
        let job = create_test_job(vec!["RUNNING", "COMPLETING"], "None");
        assert!(job.is_running());
        assert!(job.is_completing());
        // COMPLETING should take priority in display
        assert_eq!(job.primary_state(), "COMPLETING");
    }

    #[test]
    fn test_job_state_failed() {
        let job = create_test_job(vec!["FAILED"], "NonZeroExitCode");
        assert!(job.is_failed());
        assert_eq!(job.primary_state(), "FAILED");
    }

    #[test]
    fn test_job_state_timeout() {
        let job = create_test_job(vec!["TIMEOUT"], "TimeLimit");
        assert!(job.is_timeout());
        assert_eq!(job.primary_state(), "TIMEOUT");
    }

    #[test]
    fn test_job_state_cancelled() {
        let job = create_test_job(vec!["CANCELLED"], "None");
        assert!(job.is_cancelled());
        assert_eq!(job.primary_state(), "CANCELLED");
    }

    #[test]
    fn test_job_state_out_of_memory() {
        let job = create_test_job(vec!["OUT_OF_MEMORY"], "None");
        assert!(job.is_out_of_memory());
        assert_eq!(job.primary_state(), "OUT_OF_MEMORY");
    }

    #[test]
    fn test_job_gpu_allocation() {
        let mut job = create_test_job(vec!["RUNNING"], "None");
        job.tres_alloc_str = "cpu=8,mem=64G,gres/gpu:l40s=2".to_string();

        assert_eq!(job.allocated_gpus(), 2);

        let gpu_info = job.gpu_type_info();
        assert_eq!(gpu_info.count, 2);
        assert_eq!(gpu_info.gpu_type, "L40S");
        assert_eq!(gpu_info.display, "2xL40S");
    }

    #[test]
    fn test_job_remaining_time() {
        let mut job = create_test_job(vec!["RUNNING"], "None");

        // Set start time to 1 hour ago
        let now = chrono::Utc::now().timestamp() as u64;
        job.start_time = TimeValue { set: true, infinite: false, number: now - 3600 };

        // Set time limit to 2 hours (120 minutes)
        job.time_limit = TimeValue { set: true, infinite: false, number: 120 };

        let remaining = job.remaining_time_minutes();
        assert!(remaining.is_some());
        let remaining = remaining.unwrap();
        // Should have ~60 minutes remaining (allowing for test execution time)
        assert!(remaining >= 58 && remaining <= 62);
    }

    #[test]
    fn test_time_value_timestamp() {
        let tv = TimeValue { set: true, infinite: false, number: 1704067200 };
        let ts = tv.to_timestamp();
        assert!(ts.is_some());

        let tv_infinite = TimeValue { set: true, infinite: true, number: 0 };
        assert!(tv_infinite.to_timestamp().is_none());

        let tv_unset = TimeValue { set: false, infinite: false, number: 0 };
        assert!(tv_unset.to_timestamp().is_none());
    }

    #[test]
    fn test_cluster_status_calculations() {
        let nodes = vec![
            create_test_node(vec!["IDLE"]),
            create_test_node(vec!["MIXED"]),
            create_test_node(vec!["DOWN"]),
        ];

        let status = ClusterStatus {
            nodes,
            jobs: vec![],
        };

        assert_eq!(status.total_nodes(), 3);
        assert_eq!(status.idle_nodes(), 1);
        assert_eq!(status.mixed_nodes(), 1);
        assert_eq!(status.down_nodes(), 1);
        assert_eq!(status.total_cpus(), 384); // 3 nodes × 128 CPUs
        assert_eq!(status.allocated_cpus(), 0);
    }

    #[test]
    fn test_sinfo_json_parsing_minimal() {
        let json = r#"{
            "sinfo": [
                {
                    "nodes": {
                        "nodes": ["testnode001"]
                    },
                    "node": {
                        "state": ["IDLE"]
                    },
                    "partition": {
                        "name": "cpu"
                    },
                    "cpus": {
                        "allocated": 0,
                        "idle": 128,
                        "total": 128,
                        "load": {
                            "minimum": 0,
                            "maximum": 0
                        }
                    },
                    "memory": {
                        "minimum": 1536000,
                        "allocated": 0,
                        "free": {
                            "minimum": {
                                "set": true,
                                "infinite": false,
                                "number": 1536000
                            },
                            "maximum": {
                                "set": true,
                                "infinite": false,
                                "number": 1536000
                            }
                        }
                    }
                }
            ],
            "errors": []
        }"#;

        let response: SinfoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.sinfo.len(), 1);
        assert_eq!(response.errors.len(), 0);

        let node = &response.sinfo[0];
        assert_eq!(node.name(), "testnode001");
        assert!(node.is_idle());
        assert_eq!(node.cpus.total, 128);
        assert_eq!(node.memory_total(), 1536000);
    }

    #[test]
    fn test_sinfo_json_parsing_with_gpu() {
        let json = r#"{
            "sinfo": [
                {
                    "nodes": {
                        "nodes": ["gpu001"]
                    },
                    "node": {
                        "state": ["MIXED"]
                    },
                    "partition": {
                        "name": "gpu"
                    },
                    "cpus": {
                        "allocated": 64,
                        "idle": 64,
                        "total": 128,
                        "load": {
                            "minimum": 0,
                            "maximum": 0
                        }
                    },
                    "memory": {
                        "minimum": 1536000,
                        "allocated": 768000,
                        "free": {
                            "minimum": {
                                "set": true,
                                "infinite": false,
                                "number": 768000
                            },
                            "maximum": {
                                "set": true,
                                "infinite": false,
                                "number": 768000
                            }
                        }
                    },
                    "gres": {
                        "total": "gpu:l40s:4(S:0-1)",
                        "used": "gpu:l40s:2(IDX:0-1)"
                    }
                }
            ],
            "errors": []
        }"#;

        let response: SinfoResponse = serde_json::from_str(json).unwrap();
        let node = &response.sinfo[0];

        assert!(node.is_mixed());
        assert_eq!(node.cpus.allocated, 64);

        let gpu_info = node.gpu_info();
        assert_eq!(gpu_info.total, 4);
        assert_eq!(gpu_info.used, 2);
        assert_eq!(gpu_info.gpu_type, "l40s");
    }

    #[test]
    fn test_squeue_json_parsing_minimal() {
        let json = r#"{
            "jobs": [
                {
                    "job_id": 12345,
                    "name": "test_job",
                    "user_name": "testuser",
                    "account": "testacct",
                    "partition": "cpu",
                    "job_state": ["RUNNING"],
                    "nodes": "cpu001"
                }
            ],
            "errors": []
        }"#;

        let response: SqueueResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.jobs.len(), 1);
        assert_eq!(response.errors.len(), 0);

        let job = &response.jobs[0];
        assert_eq!(job.job_id, 12345);
        assert_eq!(job.name, "test_job");
        assert_eq!(job.user_name, "testuser");
        assert!(job.is_running());
        assert_eq!(job.primary_state(), "RUNNING");
    }

    #[test]
    fn test_squeue_json_parsing_with_tres() {
        let json = r#"{
            "jobs": [
                {
                    "job_id": 67890,
                    "name": "gpu_job",
                    "user_name": "testuser",
                    "account": "testacct",
                    "partition": "gpu",
                    "job_state": ["RUNNING"],
                    "nodes": "gpu001",
                    "tres_alloc_str": "cpu=16,mem=128G,node=1,billing=16,gres/gpu:l40s=2",
                    "cpus_per_task": {
                        "set": true,
                        "infinite": false,
                        "number": 2
                    },
                    "tasks": {
                        "set": true,
                        "infinite": false,
                        "number": 8
                    }
                }
            ],
            "errors": []
        }"#;

        let response: SqueueResponse = serde_json::from_str(json).unwrap();
        let job = &response.jobs[0];

        assert_eq!(job.allocated_gpus(), 2);

        let gpu_info = job.gpu_type_info();
        assert_eq!(gpu_info.count, 2);
        assert_eq!(gpu_info.gpu_type, "L40S");
        assert_eq!(gpu_info.display, "2xL40S");

        let resources = job.allocated_resources();
        assert_eq!(resources.get("cpu"), Some(&"16".to_string()));
        assert_eq!(resources.get("mem"), Some(&"128G".to_string()));
        assert_eq!(resources.get("node"), Some(&"1".to_string()));
    }

    #[test]
    fn test_reason_info_deserialization() {
        // String reason (empty string becomes String variant)
        let json_empty = r#""""#;
        let reason: ReasonInfo = serde_json::from_str(json_empty).unwrap();
        assert_eq!(reason.description(), "");

        // String reason
        let json_string = r#""Not responding""#;
        let reason: ReasonInfo = serde_json::from_str(json_string).unwrap();
        assert!(matches!(reason, ReasonInfo::String(_)));
        assert_eq!(reason.description(), "Not responding");

        // Object reason
        let json_object = r#"{"description": "Node down for maintenance"}"#;
        let reason: ReasonInfo = serde_json::from_str(json_object).unwrap();
        assert!(matches!(reason, ReasonInfo::Object { .. }));
        assert_eq!(reason.description(), "Node down for maintenance");
    }
}