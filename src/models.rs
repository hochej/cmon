//! Data models for Slurm JSON responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

// Re-export formatting functions for backwards compatibility
pub use crate::formatting::format_duration_human as format_duration_seconds;
pub use crate::formatting::format_duration_human_minutes as format_duration_minutes;

/// Slurm time value - represents optional/infinite numeric values from Slurm JSON.
///
/// This enum ensures that only valid states are representable:
/// - `NotSet`: The value was not set in Slurm (set=false)
/// - `Infinite`: The value represents infinity (set=true, infinite=true)
/// - `Value(u64)`: A concrete numeric value (set=true, infinite=false)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TimeValue {
    /// Value not set (Slurm JSON: set=false)
    #[default]
    NotSet,
    /// Infinite/unlimited value (Slurm JSON: set=true, infinite=true)
    Infinite,
    /// Concrete numeric value (Slurm JSON: set=true, infinite=false, number=N)
    Value(u64),
}

impl TimeValue {
    /// Returns the numeric value if set and not infinite.
    #[must_use]
    pub fn value(&self) -> Option<u64> {
        match self {
            TimeValue::Value(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the numeric value, or 0 if not set or infinite.
    /// Useful for backwards-compatible field access patterns.
    #[must_use]
    pub fn number(&self) -> u64 {
        match self {
            TimeValue::Value(n) => *n,
            _ => 0,
        }
    }

    /// Returns true if this value is set (either Value or Infinite).
    #[allow(dead_code)]
    #[must_use]
    pub fn is_set(&self) -> bool {
        !matches!(self, TimeValue::NotSet)
    }

    /// Returns true if this value represents infinity.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_infinite(&self) -> bool {
        matches!(self, TimeValue::Infinite)
    }

    /// Convert to a timestamp if this is a concrete value.
    #[allow(dead_code)]
    #[must_use]
    pub fn to_timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            TimeValue::Value(n) => DateTime::from_timestamp(*n as i64, 0),
            _ => None,
        }
    }

    /// Create a TimeValue from explicit set/infinite/number fields.
    /// Used internally for deserialization.
    #[must_use]
    fn from_fields(set: bool, infinite: bool, number: u64) -> Self {
        if !set {
            TimeValue::NotSet
        } else if infinite {
            TimeValue::Infinite
        } else {
            TimeValue::Value(number)
        }
    }
}

/// Internal struct for deserializing Slurm's JSON format
#[derive(Deserialize)]
struct TimeValueRaw {
    #[serde(default)]
    set: bool,
    #[serde(default)]
    infinite: bool,
    #[serde(default)]
    number: u64,
}

impl<'de> Deserialize<'de> for TimeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = TimeValueRaw::deserialize(deserializer)?;
        Ok(TimeValue::from_fields(raw.set, raw.infinite, raw.number))
    }
}

impl Serialize for TimeValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("TimeValue", 3)?;
        match self {
            TimeValue::NotSet => {
                state.serialize_field("set", &false)?;
                state.serialize_field("infinite", &false)?;
                state.serialize_field("number", &0u64)?;
            }
            TimeValue::Infinite => {
                state.serialize_field("set", &true)?;
                state.serialize_field("infinite", &true)?;
                state.serialize_field("number", &0u64)?;
            }
            TimeValue::Value(n) => {
                state.serialize_field("set", &true)?;
                state.serialize_field("infinite", &false)?;
                state.serialize_field("number", n)?;
            }
        }
        state.end()
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
    #[allow(dead_code)]
    #[must_use]
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
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            ReasonInfo::Empty => "",
            ReasonInfo::String(s) => s.as_str(),
            ReasonInfo::Object { description } => description.as_str(),
        }
    }
}

impl NodeInfo {
    #[must_use]
    pub fn name(&self) -> &str {
        self.node_names
            .nodes
            .first()
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    /// Get the partition name (preserves original casing from Slurm)
    #[must_use]
    pub fn partition_name(&self) -> String {
        self.partition
            .name
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }

    // ========================================================================
    // Node State Helpers
    // ========================================================================

    /// Check if node has any of the specified states
    /// This is a generic helper that reduces repetition across is_* methods
    fn has_state(&self, states: &[&str]) -> bool {
        self.node_state
            .state
            .iter()
            .any(|s| states.iter().any(|state| s == *state))
    }

    // Primary node states - all use has_state() for consistency
    #[must_use]
    pub fn is_allocated(&self) -> bool {
        self.has_state(&["ALLOCATED", "ALLOC"])
    }

    #[must_use]
    pub fn is_completing(&self) -> bool {
        self.has_state(&["COMPLETING", "COMP"])
    }

    #[must_use]
    pub fn is_down(&self) -> bool {
        self.has_state(&["DOWN"])
    }

    #[must_use]
    pub fn is_drained(&self) -> bool {
        self.has_state(&["DRAINED"])
    }

    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.has_state(&["DRAINING", "DRAIN", "DRNG"])
    }

    #[must_use]
    pub fn is_fail(&self) -> bool {
        self.has_state(&["FAIL"])
    }

    #[must_use]
    pub fn is_failing(&self) -> bool {
        self.has_state(&["FAILING", "FAILG"])
    }

    #[must_use]
    pub fn is_future(&self) -> bool {
        self.has_state(&["FUTURE", "FUTR"])
    }

    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.has_state(&["IDLE"])
    }

    #[must_use]
    pub fn is_maint(&self) -> bool {
        self.has_state(&["MAINT"])
    }

    #[must_use]
    pub fn is_mixed(&self) -> bool {
        self.has_state(&["MIXED", "MIX"])
    }

    #[must_use]
    pub fn is_perfctrs(&self) -> bool {
        self.has_state(&["PERFCTRS", "NPC"])
    }

    #[must_use]
    pub fn is_planned(&self) -> bool {
        self.has_state(&["PLANNED", "PLND"])
    }

    #[must_use]
    pub fn is_power_down(&self) -> bool {
        self.has_state(&["POWER_DOWN", "POW_DN"])
    }

    #[must_use]
    pub fn is_powered_down(&self) -> bool {
        self.has_state(&["POWERED_DOWN"])
    }

    #[must_use]
    pub fn is_powering_down(&self) -> bool {
        self.has_state(&["POWERING_DOWN"])
    }

    #[must_use]
    pub fn is_powering_up(&self) -> bool {
        self.has_state(&["POWERING_UP", "POW_UP"])
    }

    #[must_use]
    pub fn is_reserved(&self) -> bool {
        self.has_state(&["RESERVED", "RESV"])
    }

    #[must_use]
    pub fn is_unknown(&self) -> bool {
        self.has_state(&["UNKNOWN", "UNK"])
    }

    #[must_use]
    pub fn is_reboot_requested(&self) -> bool {
        self.has_state(&["REBOOT_REQUESTED"])
    }

    #[must_use]
    pub fn is_reboot_issued(&self) -> bool {
        self.has_state(&["REBOOT_ISSUED"])
    }

    #[must_use]
    pub fn is_inval(&self) -> bool {
        self.has_state(&["INVAL"])
    }

    #[must_use]
    pub fn is_cloud(&self) -> bool {
        self.has_state(&["CLOUD"])
    }

    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.has_state(&["BLOCKED"])
    }

    /// Get the primary node state for display
    #[must_use]
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
        self.node_state
            .state
            .first()
            .map(|s| s.as_str())
            .unwrap_or("UNKNOWN")
    }

    /// Get node reason description
    #[must_use]
    pub fn reason_description(&self) -> &str {
        self.reason.description()
    }

    #[must_use]
    pub fn memory_total(&self) -> u64 {
        self.memory.minimum
    }

    #[must_use]
    pub fn memory_free(&self) -> u64 {
        self.memory.free.minimum.number()
    }

    #[must_use]
    pub fn memory_utilization(&self) -> f64 {
        if self.memory.minimum == 0 {
            0.0
        } else {
            let free = self.memory.free.minimum.number();
            let used = self.memory.minimum.saturating_sub(free);
            (used as f64 / self.memory.minimum as f64) * 100.0
        }
    }

    /// Parse GPU information from GRES string
    #[must_use]
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

/// Job state enum parsed from Slurm state strings.
///
/// This is the authoritative representation of job states used throughout
/// the codebase. It consolidates what was previously duplicated between
/// `models::JobInfo` (using `Vec<String>`) and `tui::app::JobState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JobState {
    #[default]
    Unknown,
    Pending,
    Running,
    Completing,
    Completed,
    Failed,
    Timeout,
    OutOfMemory,
    Cancelled,
    NodeFail,
    Suspended,
    Preempted,
}

impl JobState {
    /// Create a JobState from Slurm's state array (Vec<String>).
    #[allow(dead_code)]
    #[must_use]
    pub fn from_slurm_state(states: &[String]) -> Self {
        match states.first().and_then(|s| s.split_whitespace().next()) {
            Some("PENDING") => Self::Pending,
            Some("RUNNING") => Self::Running,
            Some("COMPLETING") => Self::Completing,
            Some("COMPLETED") => Self::Completed,
            Some("FAILED") => Self::Failed,
            Some("TIMEOUT") => Self::Timeout,
            Some("OUT_OF_MEMORY") => Self::OutOfMemory,
            Some("CANCELLED") => Self::Cancelled,
            Some("NODE_FAIL") => Self::NodeFail,
            Some("SUSPENDED") => Self::Suspended,
            Some("PREEMPTED") => Self::Preempted,
            _ => Self::Unknown,
        }
    }

    /// Create a JobState from a single state string.
    ///
    /// Handles both full names (e.g., "RUNNING") and short codes (e.g., "R").
    /// Also handles state strings with additional info like "CANCELLED by 12345".
    #[must_use]
    pub fn from_state_string(state: &str) -> Self {
        match state.split_whitespace().next() {
            Some("PENDING") | Some("PD") => Self::Pending,
            Some("RUNNING") | Some("R") => Self::Running,
            Some("COMPLETING") | Some("CG") => Self::Completing,
            Some("COMPLETED") | Some("CD") => Self::Completed,
            Some("FAILED") | Some("F") => Self::Failed,
            Some("TIMEOUT") | Some("TO") => Self::Timeout,
            Some("OUT_OF_MEMORY") | Some("OOM") => Self::OutOfMemory,
            Some("CANCELLED") | Some("CA") => Self::Cancelled,
            Some("NODE_FAIL") | Some("NF") => Self::NodeFail,
            Some("SUSPENDED") | Some("S") => Self::Suspended,
            Some("PREEMPTED") | Some("PR") => Self::Preempted,
            _ => Self::Unknown,
        }
    }

    /// Return the full Slurm state name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Completing => "COMPLETING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Timeout => "TIMEOUT",
            Self::OutOfMemory => "OOM",
            Self::Cancelled => "CANCELLED",
            Self::NodeFail => "NODE_FAIL",
            Self::Suspended => "SUSPENDED",
            Self::Preempted => "PREEMPTED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// Return a short display string for the state.
    #[must_use]
    pub fn short_str(&self) -> &'static str {
        match self {
            Self::Pending => "PD",
            Self::Running => "RUN",
            Self::Completing => "CG",
            Self::Completed => "CD",
            Self::Failed => "FAIL",
            Self::Timeout => "TO",
            Self::OutOfMemory => "OOM",
            Self::Cancelled => "CA",
            Self::NodeFail => "NF",
            Self::Suspended => "SUSP",
            Self::Preempted => "PR",
            Self::Unknown => "?",
        }
    }
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
    /// Check if job state contains any of the given states (without allocating)
    fn has_state(&self, states: &[&str]) -> bool {
        self.state
            .iter()
            .any(|s| states.iter().any(|state| s == *state))
    }

    // Base job states
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.has_state(&["RUNNING"])
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.has_state(&["PENDING"])
    }

    #[must_use]
    pub fn is_suspended(&self) -> bool {
        self.has_state(&["SUSPENDED"])
    }

    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.has_state(&["COMPLETED"])
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.has_state(&["CANCELLED"])
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.has_state(&["FAILED"])
    }

    #[must_use]
    pub fn is_timeout(&self) -> bool {
        self.has_state(&["TIMEOUT"])
    }

    #[must_use]
    pub fn is_node_fail(&self) -> bool {
        self.has_state(&["NODE_FAIL"])
    }

    #[must_use]
    pub fn is_preempted(&self) -> bool {
        self.has_state(&["PREEMPTED"])
    }

    #[must_use]
    pub fn is_boot_fail(&self) -> bool {
        self.has_state(&["BOOT_FAIL"])
    }

    #[must_use]
    pub fn is_deadline(&self) -> bool {
        self.has_state(&["DEADLINE"])
    }

    #[must_use]
    pub fn is_out_of_memory(&self) -> bool {
        self.has_state(&["OUT_OF_MEMORY"])
    }

    // Job state flags
    #[must_use]
    pub fn is_completing(&self) -> bool {
        self.has_state(&["COMPLETING"])
    }

    #[must_use]
    pub fn is_configuring(&self) -> bool {
        self.has_state(&["CONFIGURING"])
    }

    #[must_use]
    pub fn is_requeued(&self) -> bool {
        self.has_state(&["REQUEUED", "REQUEUE_FED", "REQUEUE_HOLD"])
    }

    #[must_use]
    pub fn is_resizing(&self) -> bool {
        self.has_state(&["RESIZING"])
    }

    #[must_use]
    pub fn is_signaling(&self) -> bool {
        self.has_state(&["SIGNALING"])
    }

    #[must_use]
    pub fn is_stage_out(&self) -> bool {
        self.has_state(&["STAGE_OUT"])
    }

    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.has_state(&["STOPPED"])
    }

    /// Get the primary job state for display
    #[must_use]
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
    #[must_use]
    pub fn is_array_job(&self) -> bool {
        self.array_job_id.number() != 0
    }

    /// Parse allocated resources from TRES string
    #[must_use]
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
    #[must_use]
    pub fn allocated_gpus(&self) -> u32 {
        self.allocated_resources()
            .iter()
            .filter(|(k, _)| k.contains("gres/gpu"))
            .find_map(|(_, v)| v.parse::<u32>().ok())
            .unwrap_or(0)
    }

    /// Parse GPU type information
    #[must_use]
    pub fn gpu_type_info(&self) -> JobGpuInfo {
        // Look for specific GPU type allocations like "gres/gpu:l40s"
        if let Some(info) = self
            .allocated_resources()
            .iter()
            .filter(|(k, _)| k.contains("gres/gpu:"))
            .find_map(|(k, v)| {
                let gpu_type = k.split("gres/gpu:").nth(1)?;
                let count = v.parse::<u32>().ok()?;
                Some(JobGpuInfo {
                    count,
                    gpu_type: gpu_type.to_uppercase(),
                    display: format!("{}x{}", count, gpu_type.to_uppercase()),
                })
            })
        {
            return info;
        }

        // Fallback to generic GPU count
        let gpu_count = self.allocated_gpus();
        if gpu_count > 0 {
            return JobGpuInfo {
                count: gpu_count,
                gpu_type: String::new(),
                display: gpu_count.to_string(),
            };
        }

        JobGpuInfo {
            count: 0,
            gpu_type: String::new(),
            display: "-".to_string(),
        }
    }

    /// Calculate remaining time in minutes
    #[must_use]
    pub fn remaining_time_minutes(&self) -> Option<i64> {
        let time_limit = self.time_limit.value()?;
        let start_time = self.start_time.value()?;

        let now = Utc::now().timestamp();
        let elapsed = now - start_time as i64;
        let elapsed_minutes = elapsed / 60;

        let time_limit_minutes = time_limit as i64;
        let remaining = time_limit_minutes - elapsed_minutes;

        Some(remaining.max(0))
    }

    /// Format remaining time for display
    #[must_use]
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
    #[must_use]
    pub fn primary_state(&self) -> &str {
        self.state
            .current
            .first()
            .map(|s| s.as_str())
            .unwrap_or("UNKNOWN")
    }

    /// Check if job completed successfully
    #[must_use]
    pub fn is_completed(&self) -> bool {
        self.state.current.iter().any(|s| s == "COMPLETED")
    }

    /// Check if job failed
    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.state.current.iter().any(|s| s == "FAILED")
    }

    /// Check if job timed out
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        self.state.current.iter().any(|s| s == "TIMEOUT")
    }

    /// Check if job was cancelled
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state
            .current
            .iter()
            .any(|s| s == "CANCELLED" || s.starts_with("CANCELLED"))
    }

    /// Check if job ran out of memory
    #[must_use]
    pub fn is_out_of_memory(&self) -> bool {
        self.state.current.iter().any(|s| s == "OUT_OF_MEMORY")
    }

    /// Check if job is running
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state.current.iter().any(|s| s == "RUNNING")
    }

    /// Check if job is pending
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.state.current.iter().any(|s| s == "PENDING")
    }

    /// Get elapsed time in human-readable format
    #[must_use]
    pub fn elapsed_display(&self) -> String {
        format_duration_seconds(self.time.elapsed)
    }

    /// Get time limit in human-readable format
    #[must_use]
    pub fn time_limit_display(&self) -> String {
        match &self.time.limit {
            TimeValue::Value(n) => format_duration_minutes(*n),
            TimeValue::Infinite => "UNLIMITED".to_string(),
            TimeValue::NotSet => "-".to_string(),
        }
    }

    /// Calculate CPU efficiency percentage
    #[must_use]
    pub fn cpu_efficiency(&self) -> Option<f64> {
        let elapsed = self.time.elapsed;
        let cpus = self.required.cpus;

        if elapsed == 0 || cpus == 0 {
            return None;
        }

        // Total CPU time available (wall time * CPUs)
        let total_cpu_time = elapsed as f64 * cpus as f64;

        // Actual CPU time used (user + system)
        let used_cpu_time =
            self.time.total.seconds as f64 + (self.time.total.microseconds as f64 / 1_000_000.0);

        if total_cpu_time > 0.0 {
            Some((used_cpu_time / total_cpu_time) * 100.0)
        } else {
            None
        }
    }

    /// Get maximum memory used from steps
    #[must_use]
    pub fn max_memory_used(&self) -> u64 {
        self.steps
            .iter()
            .filter_map(|step| step.statistics.as_ref())
            .filter_map(|stats| stats.memory.as_ref())
            .map(|mem| mem.max.task.bytes)
            .max()
            .unwrap_or(0)
    }

    /// Get requested memory in bytes
    #[must_use]
    pub fn requested_memory(&self) -> u64 {
        // Memory per node takes precedence
        if let Some(mem) = self.required.memory_per_node.value() {
            if mem > 0 {
                // Memory is in MB
                return mem * 1024 * 1024;
            }
        }

        // Fall back to memory per CPU * CPUs
        if let Some(mem) = self.required.memory_per_cpu.value() {
            if mem > 0 {
                return mem * self.required.cpus as u64 * 1024 * 1024;
            }
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
    #[must_use]
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
    #[must_use]
    pub fn allocated_gpus(&self) -> u32 {
        for tres in &self.tres.allocated {
            if tres.tres_type == "gres" && tres.name.starts_with("gpu") {
                return tres.count as u32;
            }
        }
        0
    }

    /// Get GPU type
    #[must_use]
    pub fn gpu_type(&self) -> Option<String> {
        for tres in &self.tres.allocated {
            if tres.tres_type == "gres"
                && tres.name.contains(':')
                && let Some(gpu_type) = tres.name.split(':').nth(1)
            {
                return Some(gpu_type.to_uppercase());
            }
        }
        None
    }

    /// Get exit code as string
    #[allow(dead_code)]
    #[must_use]
    pub fn exit_code_display(&self) -> String {
        if let Some(code) = self.exit_code.return_code.value() {
            format!("{}", code)
        } else if !self.exit_code.signal.name.is_empty() {
            format!("SIG{}", self.exit_code.signal.name)
        } else {
            "-".to_string()
        }
    }

    /// Get submit time as formatted string
    #[must_use]
    pub fn submit_time_display(&self) -> String {
        if self.time.submission > 0
            && let Some(dt) = DateTime::from_timestamp(self.time.submission as i64, 0)
        {
            return dt.format("%Y-%m-%d %H:%M:%S").to_string();
        }
        "-".to_string()
    }

    /// Get start time as formatted string
    #[must_use]
    pub fn start_time_display(&self) -> String {
        if self.time.start > 0
            && let Some(dt) = DateTime::from_timestamp(self.time.start as i64, 0)
        {
            return dt.format("%Y-%m-%d %H:%M:%S").to_string();
        }
        "-".to_string()
    }

    /// Get end time as formatted string
    #[must_use]
    pub fn end_time_display(&self) -> String {
        if self.time.end > 0
            && let Some(dt) = DateTime::from_timestamp(self.time.end as i64, 0)
        {
            return dt.format("%Y-%m-%d %H:%M:%S").to_string();
        }
        "-".to_string()
    }

    /// Get wait time (time between submission and start)
    #[must_use]
    pub fn wait_time(&self) -> Option<u64> {
        if self.time.submission > 0
            && self.time.start > 0
            && self.time.start >= self.time.submission
        {
            Some(self.time.start - self.time.submission)
        } else {
            None
        }
    }

    /// Get wait time display
    #[must_use]
    pub fn wait_time_display(&self) -> String {
        self.wait_time()
            .map(format_duration_seconds)
            .unwrap_or_else(|| "-".to_string())
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
    #[must_use]
    pub fn shares_fraction(&self) -> f64 {
        if self.shares_normalized.set {
            self.shares_normalized.number
        } else {
            0.0
        }
    }

    /// Get fairshare factor (0.0 to 1.0, higher is better)
    #[must_use]
    pub fn fairshare_factor(&self) -> f64 {
        if self.fairshare.factor.set {
            self.fairshare.factor.number
        } else {
            0.0
        }
    }

    /// Check if this is a user entry (has a parent that's an account)
    #[must_use]
    pub fn is_user(&self) -> bool {
        !self.parent.is_empty() && self.parent != "root"
    }

    /// Get CPU hours from TRES run_seconds
    #[must_use]
    pub fn cpu_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "cpu" {
                if let Some(val) = item.value.value() {
                    return val as f64 / 3600.0;
                }
            }
        }
        0.0
    }

    /// Get GPU hours from TRES run_seconds
    #[must_use]
    pub fn gpu_hours(&self) -> f64 {
        self.tres
            .run_seconds
            .iter()
            .filter(|item| item.name.starts_with("gres/gpu"))
            .filter_map(|item| item.value.value())
            .map(|val| val as f64 / 3600.0)
            .sum()
    }

    /// Get memory GB-hours from TRES run_seconds
    #[allow(dead_code)]
    #[must_use]
    pub fn mem_gb_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "mem" {
                if let Some(val) = item.value.value() {
                    // Memory is in MB-seconds, convert to GB-hours
                    return val as f64 / (1024.0 * 3600.0);
                }
            }
        }
        0.0
    }
}

/// Fairshare tree node for hierarchical display
#[derive(Debug, Clone)]
pub struct FairshareNode {
    pub name: String,
    #[allow(dead_code)]
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

    fn build_node(
        entry: &SshareEntry,
        all_entries: &[SshareEntry],
        depth: usize,
        current_user: &str,
    ) -> Self {
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
                let child_node =
                    Self::build_node(child_entry, all_entries, depth + 1, current_user);
                node.children.push(child_node);
            }
        }

        node
    }

    /// Flatten the tree for display with proper indentation
    #[must_use]
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
    #[must_use]
    pub fn display_name(&self) -> String {
        let indent = "  ".repeat(self.depth);
        let prefix = if self.has_children { "+" } else { "-" };
        format!("{}{} {}", indent, prefix, self.name)
    }
}

// ============================================================================
// Scheduler stats (sdiag) models (Phase 4)
// ============================================================================

/// Main scheduler cycle statistics (microseconds)
#[derive(Debug, Clone, Default)]
pub struct CycleStats {
    pub last_us: Option<u64>,
    pub mean_us: Option<u64>,
    pub max_us: Option<u64>,
}

/// Backfill scheduler statistics
#[derive(Debug, Clone, Default)]
pub struct BackfillStats {
    pub last_cycle_us: Option<u64>,
    pub queue_length: Option<u64>,
    pub last_depth: Option<u64>,
    pub total_jobs_since_start: Option<u64>,
}

/// Scheduler statistics from sdiag
///
/// This enum ensures that invalid states are unrepresentable:
/// - When available, we have full statistics
/// - When unavailable, we have a reason explaining why
#[derive(Debug, Clone)]
pub enum SchedulerStats {
    /// Scheduler stats successfully retrieved
    Available {
        jobs_pending: Option<u64>,
        jobs_running: Option<u64>,
        cycles: CycleStats,
        backfill: BackfillStats,
        #[allow(dead_code)]
        fetched_at: std::time::Instant,
    },
    /// Scheduler stats unavailable (permission denied, command failed, etc.)
    Unavailable {
        #[allow(dead_code)]
        reason: String,
    },
}

impl SchedulerStats {
    /// Parse sdiag text output
    pub fn from_sdiag_output(output: &str) -> Self {
        let mut cycles = CycleStats::default();
        let mut backfill = BackfillStats::default();
        let mut jobs_pending = None;
        let mut jobs_running = None;

        for line in output.lines() {
            let line = line.trim();

            // Main scheduler stats
            if line.starts_with("Last cycle:") {
                cycles.last_us = Self::parse_microseconds(line);
            } else if line.starts_with("Mean cycle:") {
                cycles.mean_us = Self::parse_microseconds(line);
            } else if line.starts_with("Max cycle:") {
                cycles.max_us = Self::parse_microseconds(line);
            } else if line.starts_with("Jobs pending:") {
                jobs_pending = Self::parse_number(line);
            } else if line.starts_with("Jobs running:") {
                jobs_running = Self::parse_number(line);
            }
            // Backfill stats
            else if line.contains("Backfill") && line.contains("Last cycle") {
                backfill.last_cycle_us = Self::parse_microseconds(line);
            } else if line.contains("Backfill") && line.contains("queue length") {
                backfill.queue_length = Self::parse_number(line);
            } else if line.contains("Backfill") && line.contains("depth") {
                backfill.last_depth = Self::parse_number(line);
            } else if line.contains("Total backfilled jobs") {
                backfill.total_jobs_since_start = Self::parse_number(line);
            }
        }

        SchedulerStats::Available {
            jobs_pending,
            jobs_running,
            cycles,
            backfill,
            fetched_at: std::time::Instant::now(),
        }
    }

    /// Create an unavailable stats instance with a reason
    pub fn unavailable(reason: String) -> Self {
        SchedulerStats::Unavailable { reason }
    }

    /// Check if stats are available
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, SchedulerStats::Available { .. })
    }

    fn parse_microseconds(line: &str) -> Option<u64> {
        // Parse lines like "Last cycle:   1234 microseconds"
        let parts: Vec<&str> = line.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            if (*part == "microseconds" || part.starts_with("microsec")) && i > 0 {
                return parts[i - 1].parse().ok();
            }
        }
        None
    }

    fn parse_number(line: &str) -> Option<u64> {
        // Parse lines like "Jobs pending:  1234"
        if let Some((_prefix, suffix)) = line.split_once(':') {
            return suffix.split_whitespace().next()?.parse().ok();
        }
        None
    }

    /// Check if scheduler is healthy (mean cycle < 5 seconds)
    /// Returns None if stats are unavailable or mean cycle is unknown
    #[must_use]
    pub fn is_healthy(&self) -> Option<bool> {
        match self {
            SchedulerStats::Available { cycles, .. } => {
                cycles.mean_us.map(|us| us < 5_000_000)
            }
            SchedulerStats::Unavailable { .. } => None,
        }
    }

    /// Format mean cycle for display
    #[must_use]
    pub fn mean_cycle_display(&self) -> String {
        match self {
            SchedulerStats::Available { cycles, .. } => match cycles.mean_us {
                Some(us) if us < 1000 => format!("{}us", us),
                Some(us) if us < 1_000_000 => format!("{:.1}ms", us as f64 / 1000.0),
                Some(us) => format!("{:.1}s", us as f64 / 1_000_000.0),
                None => "N/A".to_string(),
            },
            SchedulerStats::Unavailable { .. } => "N/A".to_string(),
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
    pub system: SystemConfig,

    #[serde(default)]
    pub refresh: RefreshConfig,

    #[serde(default)]
    pub display: DisplayConfig,

    #[serde(default)]
    pub behavior: BehaviorConfig,
}

/// System configuration for paths and environment
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SystemConfig {
    /// Path to directory containing Slurm binaries (sinfo, squeue, etc.)
    /// If empty or not set, auto-detected via PATH
    #[serde(default)]
    pub slurm_bin_path: Option<std::path::PathBuf>,
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

/// Minimum allowed refresh interval in seconds (prevents tight polling loops)
const MIN_REFRESH_INTERVAL: u64 = 1;

/// Minimum idle threshold in seconds
const MIN_IDLE_THRESHOLD: u64 = 1;

impl RefreshConfig {
    /// Validate refresh configuration values.
    /// Returns a list of warnings for invalid values that were corrected to defaults.
    /// If `strict` is true, returns Err instead of correcting values.
    pub fn validate(&mut self, strict: bool) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        let defaults = Self::default();

        // Validate jobs_interval
        if self.jobs_interval < MIN_REFRESH_INTERVAL {
            let msg = format!(
                "refresh.jobs_interval must be at least {} second(s), got {}",
                MIN_REFRESH_INTERVAL, self.jobs_interval
            );
            if strict {
                return Err(msg);
            }
            warnings.push(format!("{} - using default ({})", msg, defaults.jobs_interval));
            self.jobs_interval = defaults.jobs_interval;
        }

        // Validate nodes_interval
        if self.nodes_interval < MIN_REFRESH_INTERVAL {
            let msg = format!(
                "refresh.nodes_interval must be at least {} second(s), got {}",
                MIN_REFRESH_INTERVAL, self.nodes_interval
            );
            if strict {
                return Err(msg);
            }
            warnings.push(format!("{} - using default ({})", msg, defaults.nodes_interval));
            self.nodes_interval = defaults.nodes_interval;
        }

        // Validate fairshare_interval
        if self.fairshare_interval < MIN_REFRESH_INTERVAL {
            let msg = format!(
                "refresh.fairshare_interval must be at least {} second(s), got {}",
                MIN_REFRESH_INTERVAL, self.fairshare_interval
            );
            if strict {
                return Err(msg);
            }
            warnings.push(format!(
                "{} - using default ({})",
                msg, defaults.fairshare_interval
            ));
            self.fairshare_interval = defaults.fairshare_interval;
        }

        // Validate idle_threshold (only relevant if idle_slowdown is enabled)
        if self.idle_slowdown && self.idle_threshold < MIN_IDLE_THRESHOLD {
            let msg = format!(
                "refresh.idle_threshold must be at least {} second(s), got {}",
                MIN_IDLE_THRESHOLD, self.idle_threshold
            );
            if strict {
                return Err(msg);
            }
            warnings.push(format!("{} - using default ({})", msg, defaults.idle_threshold));
            self.idle_threshold = defaults.idle_threshold;
        }

        Ok(warnings)
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

    /// Partition display order (empty = alphabetical)
    /// Example: ["cpu", "gpu", "fat", "vdi"]
    #[serde(default)]
    pub partition_order: Vec<String>,

    /// Prefix to strip from node names for display (optional)
    /// Example: "demu4x" would turn "demu4xcpu01" into "cpu01"
    #[serde(default)]
    pub node_prefix_strip: String,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_view: "jobs".to_string(),
            show_all_jobs: false,
            show_grouped_by_account: false,
            theme: "dark".to_string(),
            partition_order: Vec::new(),
            node_prefix_strip: String::new(),
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

fn default_jobs_interval() -> u64 {
    5
}
fn default_nodes_interval() -> u64 {
    10
}
fn default_fairshare_interval() -> u64 {
    60
}
fn default_idle_threshold() -> u64 {
    30
}
fn default_true() -> bool {
    true
}
fn default_view() -> String {
    "jobs".to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}

impl TuiConfig {
    /// Get the user config file path, respecting XDG_CONFIG_HOME
    ///
    /// Resolution order:
    /// 1. $XDG_CONFIG_HOME/cmon/config.toml (if XDG_CONFIG_HOME is set)
    /// 2. $HOME/.config/cmon/config.toml (if HOME is set)
    /// 3. dirs::config_dir()/cmon/config.toml (fallback using dirs crate)
    /// 4. None if no config directory can be determined
    #[must_use]
    pub fn user_config_path() -> Option<std::path::PathBuf> {
        // Prefer XDG_CONFIG_HOME if set
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME")
            && !xdg_config.is_empty()
        {
            return Some(std::path::PathBuf::from(xdg_config).join("cmon/config.toml"));
        }

        // Fall back to ~/.config
        if let Some(home) = std::env::var_os("HOME") {
            return Some(std::path::PathBuf::from(home).join(".config/cmon/config.toml"));
        }

        // Last resort: use dirs crate
        dirs::config_dir().map(|dir| dir.join("cmon/config.toml"))
    }

    /// Load configuration from files and environment.
    /// Returns the config and any warnings encountered during loading.
    pub fn load() -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();
        let strict = Self::is_strict_mode();

        // Try to load from /etc/cmon/config.toml
        Self::load_config_file(&mut config, "/etc/cmon/config.toml", &mut warnings);

        // Try to load from user config path (respects XDG_CONFIG_HOME)
        if let Some(user_path) = Self::user_config_path() {
            Self::load_config_file(&mut config, &user_path.to_string_lossy(), &mut warnings);
        }

        // Apply environment overrides
        config.apply_env_overrides();

        // Validate refresh intervals
        match config.refresh.validate(strict) {
            Ok(validation_warnings) => warnings.extend(validation_warnings),
            Err(err) => {
                eprintln!("Error: {}", err);
                eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                std::process::exit(1);
            }
        }

        (config, warnings)
    }

    /// Check if strict config mode is enabled via CMON_STRICT_CONFIG
    fn is_strict_mode() -> bool {
        std::env::var("CMON_STRICT_CONFIG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// Load a config file, collecting warnings on parse errors but not on missing files.
    /// If CMON_STRICT_CONFIG=1 is set, parse errors cause immediate exit.
    fn load_config_file(config: &mut Self, path: &str, warnings: &mut Vec<String>) {
        let strict = Self::is_strict_mode();

        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<TuiConfig>(&content) {
                Ok(parsed) => config.merge(parsed),
                Err(e) => {
                    if strict {
                        eprintln!("Error: Failed to parse config file '{}': {}", path, e);
                        eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                        std::process::exit(1);
                    } else {
                        // Collect warning for display in TUI status bar
                        warnings.push(format!("Config parse error in '{}': {}", path, e));
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File not found is expected and not an error
            }
            Err(e) => {
                // Other errors (permissions, etc.) should be reported
                if strict {
                    eprintln!("Error: Could not read config file '{}': {}", path, e);
                    eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                    std::process::exit(1);
                } else {
                    warnings.push(format!("Could not read config '{}': {}", path, e));
                }
            }
        }
    }

    fn merge(&mut self, other: TuiConfig) {
        // Prefer other's slurm_bin_path if set, otherwise keep current
        self.system.slurm_bin_path = other
            .system
            .slurm_bin_path
            .or(self.system.slurm_bin_path.take());
        self.refresh = other.refresh;
        self.display = other.display;
        self.behavior = other.behavior;
    }

    fn apply_env_overrides(&mut self) {
        let strict = Self::is_strict_mode();

        // System overrides
        if let Ok(val) = std::env::var("CMON_SLURM_PATH")
            && !val.is_empty()
        {
            let path = std::path::PathBuf::from(&val);
            if path.is_dir() {
                self.system.slurm_bin_path = Some(path);
            } else {
                Self::report_env_error(
                    strict,
                    "CMON_SLURM_PATH",
                    &val,
                    "not a valid directory",
                );
            }
        }

        if let Ok(val) = std::env::var("CMON_REFRESH_JOBS") {
            match val.parse::<u64>() {
                Ok(secs) if secs >= MIN_REFRESH_INTERVAL => {
                    self.refresh.jobs_interval = secs;
                }
                Ok(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_JOBS",
                    &val,
                    &format!("must be at least {} second(s)", MIN_REFRESH_INTERVAL),
                ),
                Err(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_JOBS",
                    &val,
                    "expected a positive integer (seconds)",
                ),
            }
        }

        if let Ok(val) = std::env::var("CMON_REFRESH_NODES") {
            match val.parse::<u64>() {
                Ok(secs) if secs >= MIN_REFRESH_INTERVAL => {
                    self.refresh.nodes_interval = secs;
                }
                Ok(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_NODES",
                    &val,
                    &format!("must be at least {} second(s)", MIN_REFRESH_INTERVAL),
                ),
                Err(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_NODES",
                    &val,
                    "expected a positive integer (seconds)",
                ),
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

    /// Report an environment variable error, exiting if strict mode is enabled
    fn report_env_error(strict: bool, var_name: &str, value: &str, reason: &str) {
        if strict {
            eprintln!("Error: Invalid value '{}' for {}: {}", value, var_name, reason);
            eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
            std::process::exit(1);
        } else {
            eprintln!(
                "Warning: Invalid value '{}' for {}, {} - using default",
                value, var_name, reason
            );
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
            partition: PartitionInfo {
                name: Some("test".to_string()),
            },
            cpus: CpuInfo {
                allocated: 0,
                idle: 128,
                total: 128,
                load: MinMaxValue {
                    minimum: 0,
                    maximum: 0,
                },
            },
            memory: MemoryInfo {
                minimum: 1024000,
                allocated: 0,
                free: MemoryFreeInfo {
                    minimum: TimeValue::Value(1024000),
                    maximum: TimeValue::Value(1024000),
                },
            },
            gres: GresInfo {
                total: String::new(),
                used: String::new(),
            },
            sockets: MinMaxValue {
                minimum: 2,
                maximum: 2,
            },
            cores: MinMaxValue {
                minimum: 32,
                maximum: 32,
            },
            threads: MinMaxValue {
                minimum: 64,
                maximum: 64,
            },
            features: FeatureInfo {
                total: String::new(),
            },
            reason: ReasonInfo::Empty,
            weight: MinMaxValue {
                minimum: 1,
                maximum: 1,
            },
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
            cpus_per_task: TimeValue::Value(1),
            tasks: TimeValue::Value(8),
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
        job.start_time = TimeValue::Value(now - 3600);

        // Set time limit to 2 hours (120 minutes)
        job.time_limit = TimeValue::Value(120);

        let remaining = job.remaining_time_minutes();
        assert!(remaining.is_some());
        let remaining = remaining.unwrap();
        // Should have ~60 minutes remaining (allowing for test execution time)
        assert!(remaining >= 58 && remaining <= 62);
    }

    #[test]
    fn test_time_value_timestamp() {
        let tv = TimeValue::Value(1704067200);
        let ts = tv.to_timestamp();
        assert!(ts.is_some());

        let tv_infinite = TimeValue::Infinite;
        assert!(tv_infinite.to_timestamp().is_none());

        let tv_unset = TimeValue::NotSet;
        assert!(tv_unset.to_timestamp().is_none());
    }

    #[test]
    fn test_time_value_enum_methods() {
        // Test Value variant
        let val = TimeValue::Value(42);
        assert!(val.is_set());
        assert!(!val.is_infinite());
        assert_eq!(val.value(), Some(42));
        assert_eq!(val.number(), 42);

        // Test Infinite variant
        let inf = TimeValue::Infinite;
        assert!(inf.is_set());
        assert!(inf.is_infinite());
        assert_eq!(inf.value(), None);
        assert_eq!(inf.number(), 0);

        // Test NotSet variant
        let unset = TimeValue::NotSet;
        assert!(!unset.is_set());
        assert!(!unset.is_infinite());
        assert_eq!(unset.value(), None);
        assert_eq!(unset.number(), 0);

        // Test Default
        assert_eq!(TimeValue::default(), TimeValue::NotSet);
    }

    #[test]
    fn test_time_value_serde_roundtrip() {
        // Test Value variant
        let val = TimeValue::Value(12345);
        let json = serde_json::to_string(&val).unwrap();
        assert!(json.contains("\"set\":true"));
        assert!(json.contains("\"infinite\":false"));
        assert!(json.contains("\"number\":12345"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, val);

        // Test Infinite variant
        let inf = TimeValue::Infinite;
        let json = serde_json::to_string(&inf).unwrap();
        assert!(json.contains("\"set\":true"));
        assert!(json.contains("\"infinite\":true"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, inf);

        // Test NotSet variant
        let unset = TimeValue::NotSet;
        let json = serde_json::to_string(&unset).unwrap();
        assert!(json.contains("\"set\":false"));
        let parsed: TimeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, unset);
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

    // Tests for RefreshConfig validation
    #[test]
    fn test_refresh_config_validate_valid_values() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "No warnings expected for valid config");
    }

    #[test]
    fn test_refresh_config_validate_minimum_values() {
        // Minimum valid values (1 second each)
        let mut config = RefreshConfig {
            jobs_interval: 1,
            nodes_interval: 1,
            fairshare_interval: 1,
            idle_slowdown: true,
            idle_threshold: 1,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "No warnings expected for minimum valid values");
    }

    #[test]
    fn test_refresh_config_validate_zero_jobs_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("jobs_interval"));
        assert!(warnings[0].contains("at least 1"));
        // Value should be corrected to default
        assert_eq!(config.jobs_interval, RefreshConfig::default().jobs_interval);
    }

    #[test]
    fn test_refresh_config_validate_zero_nodes_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 0,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nodes_interval"));
        // Value should be corrected to default
        assert_eq!(config.nodes_interval, RefreshConfig::default().nodes_interval);
    }

    #[test]
    fn test_refresh_config_validate_zero_fairshare_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 0,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("fairshare_interval"));
        // Value should be corrected to default
        assert_eq!(
            config.fairshare_interval,
            RefreshConfig::default().fairshare_interval
        );
    }

    #[test]
    fn test_refresh_config_validate_zero_idle_threshold_with_slowdown_enabled() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("idle_threshold"));
        // Value should be corrected to default
        assert_eq!(
            config.idle_threshold,
            RefreshConfig::default().idle_threshold
        );
    }

    #[test]
    fn test_refresh_config_validate_zero_idle_threshold_with_slowdown_disabled() {
        // If idle_slowdown is disabled, idle_threshold doesn't matter
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: false,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "No warnings when idle_slowdown is disabled"
        );
    }

    #[test]
    fn test_refresh_config_validate_multiple_zero_values() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 0,
            fairshare_interval: 0,
            idle_slowdown: true,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        // Should have 4 warnings: jobs, nodes, fairshare, idle_threshold
        assert_eq!(warnings.len(), 4);
        // All values should be corrected to defaults
        let defaults = RefreshConfig::default();
        assert_eq!(config.jobs_interval, defaults.jobs_interval);
        assert_eq!(config.nodes_interval, defaults.nodes_interval);
        assert_eq!(config.fairshare_interval, defaults.fairshare_interval);
        assert_eq!(config.idle_threshold, defaults.idle_threshold);
    }

    #[test]
    fn test_refresh_config_validate_strict_mode_error() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("jobs_interval"));
        assert!(err.contains("at least 1"));
    }
}
