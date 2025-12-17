//! Job and node state types and priority constants.
//!
//! This module defines the authoritative JobState enum used throughout the codebase,
//! consolidating what was previously duplicated between different parts of the code.
//! It also provides const arrays defining the priority order for determining the
//! primary state to display when multiple states are present.

// ============================================================================
// Job State Priority
// ============================================================================

/// Priority order for job states when determining the primary state to display.
///
/// Each entry is `(display_name, &[variants])` where variants are the Slurm state
/// strings that map to this display name. Earlier entries have higher priority.
///
/// Based on official Slurm documentation: if a recognized flag is present,
/// it will be reported instead of the base state. Flags are checked first,
/// then base states in order of operational relevance.
///
/// Reference: https://slurm.schedmd.com/job_state_codes.html
pub const JOB_STATE_PRIORITY: &[(&str, &[&str])] = &[
    // ==========================================================================
    // Job Flags (take precedence over base states)
    // ==========================================================================
    // Failure/error flags first (most critical)
    ("LAUNCH_FAILED", &["LAUNCH_FAILED"]),
    ("RECONFIG_FAIL", &["RECONFIG_FAIL"]),
    // Transitional flags
    ("COMPLETING", &["COMPLETING"]),
    ("CONFIGURING", &["CONFIGURING"]),
    ("POWER_UP_NODE", &["POWER_UP_NODE"]),
    ("STAGE_OUT", &["STAGE_OUT"]),
    // Requeue-related flags (each displayed separately for clarity)
    ("REQUEUED", &["REQUEUED"]),
    ("REQUEUE_FED", &["REQUEUE_FED"]),
    ("REQUEUE_HOLD", &["REQUEUE_HOLD"]),
    ("SPECIAL_EXIT", &["SPECIAL_EXIT"]),
    // Hold flags
    ("RESV_DEL_HOLD", &["RESV_DEL_HOLD"]),
    // Operational flags
    ("EXPEDITING", &["EXPEDITING"]),
    ("RESIZING", &["RESIZING"]),
    ("SIGNALING", &["SIGNALING"]),
    ("STOPPED", &["STOPPED"]),
    ("UPDATE_DB", &["UPDATE_DB"]),
    // Federation flag
    ("REVOKED", &["REVOKED"]),
    // ==========================================================================
    // Base States
    // ==========================================================================
    // Active states first
    ("RUNNING", &["RUNNING"]),
    ("PENDING", &["PENDING"]),
    ("SUSPENDED", &["SUSPENDED"]),
    // Successful completion
    ("COMPLETED", &["COMPLETED"]),
    // Termination states (ordered by severity/commonality)
    ("CANCELLED", &["CANCELLED"]),
    ("FAILED", &["FAILED"]),
    ("TIMEOUT", &["TIMEOUT"]),
    ("PREEMPTED", &["PREEMPTED"]),
    ("NODE_FAIL", &["NODE_FAIL"]),
    ("BOOT_FAIL", &["BOOT_FAIL"]),
    ("DEADLINE", &["DEADLINE"]),
    ("OUT_OF_MEMORY", &["OUT_OF_MEMORY"]),
];

// ============================================================================
// Node State Priority
// ============================================================================

/// Priority order for node states when determining the primary state to display.
///
/// Each entry is `(display_name, &[variants])` where variants are the Slurm state
/// strings that map to this display name. Earlier entries have higher priority.
///
/// The ordering prioritizes:
/// 1. Critical states (DOWN, FAIL) - problems requiring attention
/// 2. Administrative states (DRAINED, MAINT) - intentional unavailability
/// 3. Transitional states (POWERING_UP, COMPLETING) - temporary conditions
/// 4. Operational states (ALLOCATED, MIXED, IDLE) - normal operation
/// 5. Special states (PLANNED, FUTURE) - informational
pub const NODE_STATE_PRIORITY: &[(&str, &[&str])] = &[
    // Critical states
    ("DOWN", &["DOWN"]),
    ("FAIL", &["FAIL"]),
    ("FAILING", &["FAILING", "FAILG"]),
    ("INVAL", &["INVAL"]),
    // Maintenance/administrative states
    ("DRAINED", &["DRAINED"]),
    ("DRAINING", &["DRAINING", "DRAIN", "DRNG"]),
    ("MAINT", &["MAINT"]),
    ("RESERVED", &["RESERVED", "RESV"]),
    // Reboot states
    ("REBOOT_ISSUED", &["REBOOT_ISSUED"]),
    ("REBOOT_REQUESTED", &["REBOOT_REQUESTED"]),
    // Power states
    ("POWERED_DOWN", &["POWERED_DOWN"]),
    ("POWERING_DOWN", &["POWERING_DOWN"]),
    ("POWERING_UP", &["POWERING_UP", "POW_UP"]),
    ("POWER_DOWN", &["POWER_DOWN", "POW_DN"]),
    // Transitional states
    ("COMPLETING", &["COMPLETING", "COMP"]),
    ("BLOCKED", &["BLOCKED"]),
    // Operational states
    ("ALLOCATED", &["ALLOCATED", "ALLOC"]),
    ("MIXED", &["MIXED", "MIX"]),
    ("IDLE", &["IDLE"]),
    // Special states
    ("PERFCTRS", &["PERFCTRS", "NPC"]),
    ("PLANNED", &["PLANNED", "PLND"]),
    ("FUTURE", &["FUTURE", "FUTR"]),
    ("CLOUD", &["CLOUD"]),
    ("UNKNOWN", &["UNKNOWN", "UNK"]),
];

// ============================================================================
// Job State Enum
// ============================================================================

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
