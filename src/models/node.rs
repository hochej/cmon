//! Node information types for Slurm cluster nodes.
//!
//! This module contains the data structures for representing node information
//! from Slurm's sinfo command, including state, resources, and GPU information.

use serde::{Deserialize, Serialize};

use super::time::TimeValue;

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

/// GPU information parsed from node GRES
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub total: u32,
    pub used: u32,
    pub gpu_type: String,
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
