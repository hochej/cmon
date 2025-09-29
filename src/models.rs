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
    #[allow(dead_code)]
    pub fn to_timestamp(&self) -> Option<DateTime<Utc>> {
        if self.set && !self.infinite {
            DateTime::from_timestamp(self.number as i64, 0)
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
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn cpu_utilization(&self) -> f64 {
        if self.cpus.total == 0 {
            0.0
        } else {
            (self.cpus.allocated as f64 / self.cpus.total as f64) * 100.0
        }
    }

    #[allow(dead_code)]
    pub fn cpu_load(&self) -> f64 {
        self.cpus.load.minimum as f64 / 100.0
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

    #[allow(dead_code)]
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