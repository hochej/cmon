//! Job and node state types and priority constants.
//!
//! This module defines the authoritative JobState enum used throughout the codebase,
//! consolidating what was previously duplicated between different parts of the code.
//! It also provides const arrays defining the priority order for determining the
//! primary state to display when multiple states are present.
//!
//! The `define_state_checkers!` macro reduces boilerplate by generating `is_*()` methods
//! from a declarative definition of state names and their Slurm string representations.

// ============================================================================
// State Checker Macro
// ============================================================================

/// Macro to generate `is_*()` methods for state checking.
///
/// This eliminates the boilerplate of writing nearly identical methods for each state.
/// Each generated method calls `has_state()` with the appropriate state strings.
///
/// # Usage
///
/// ```ignore
/// define_state_checkers! {
///     is_running => ["RUNNING"],
///     is_pending => ["PENDING"],
///     is_draining => ["DRAINING", "DRAIN", "DRNG"],
/// }
/// ```
///
/// This generates:
/// ```ignore
/// #[must_use]
/// pub fn is_running(&self) -> bool {
///     self.has_state(&["RUNNING"])
/// }
/// // ... etc
/// ```
macro_rules! define_state_checkers {
    ($($method:ident => [$($state:literal),+ $(,)?]),* $(,)?) => {
        $(
            #[allow(dead_code)]
            #[must_use]
            pub fn $method(&self) -> bool {
                self.has_state(&[$($state),+])
            }
        )*
    }
}

// Make the macro available to other modules in this crate
pub(crate) use define_state_checkers;

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
///
/// This enum covers all Slurm **base** job states. Job **flags** (like CONFIGURING,
/// REQUEUED, etc.) are handled separately via the `is_*()` methods on `JobInfo`.
/// See: https://slurm.schedmd.com/job_state_codes.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JobState {
    #[default]
    Unknown,
    // Active states
    Running,
    Pending,
    Suspended,
    // Transitional flag (commonly shown instead of base state)
    Completing,
    // Successful completion
    Completed,
    // Termination states
    Cancelled,
    Failed,
    Timeout,
    Preempted,
    NodeFail,
    BootFail,
    Deadline,
    OutOfMemory,
}

impl JobState {
    /// Create a JobState from Slurm's state array (Vec<String>).
    #[allow(dead_code)]
    #[must_use]
    pub fn from_slurm_state(states: &[String]) -> Self {
        states
            .first()
            .map(|s| Self::from_state_string(s))
            .unwrap_or_default()
    }

    /// Create a JobState from a single state string.
    ///
    /// Handles both full names (e.g., "RUNNING") and short codes (e.g., "R").
    /// Also handles state strings with additional info like "CANCELLED by 12345".
    #[must_use]
    pub fn from_state_string(state: &str) -> Self {
        match state.split_whitespace().next() {
            // Active states
            Some("RUNNING") | Some("R") => Self::Running,
            Some("PENDING") | Some("PD") => Self::Pending,
            Some("SUSPENDED") | Some("S") => Self::Suspended,
            // Transitional flag
            Some("COMPLETING") | Some("CG") => Self::Completing,
            // Successful completion
            Some("COMPLETED") | Some("CD") => Self::Completed,
            // Termination states
            Some("CANCELLED") | Some("CA") => Self::Cancelled,
            Some("FAILED") | Some("F") => Self::Failed,
            Some("TIMEOUT") | Some("TO") => Self::Timeout,
            Some("PREEMPTED") | Some("PR") => Self::Preempted,
            Some("NODE_FAIL") | Some("NF") => Self::NodeFail,
            Some("BOOT_FAIL") | Some("BF") => Self::BootFail,
            Some("DEADLINE") | Some("DL") => Self::Deadline,
            Some("OUT_OF_MEMORY") | Some("OOM") => Self::OutOfMemory,
            _ => Self::Unknown,
        }
    }

    /// Return the full Slurm state name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Running => "RUNNING",
            Self::Pending => "PENDING",
            Self::Suspended => "SUSPENDED",
            Self::Completing => "COMPLETING",
            Self::Completed => "COMPLETED",
            Self::Cancelled => "CANCELLED",
            Self::Failed => "FAILED",
            Self::Timeout => "TIMEOUT",
            Self::Preempted => "PREEMPTED",
            Self::NodeFail => "NODE_FAIL",
            Self::BootFail => "BOOT_FAIL",
            Self::Deadline => "DEADLINE",
            Self::OutOfMemory => "OUT_OF_MEMORY",
        }
    }

    /// Return a short display string for the state.
    #[must_use]
    pub fn short_str(&self) -> &'static str {
        match self {
            Self::Unknown => "?",
            Self::Running => "RUN",
            Self::Pending => "PD",
            Self::Suspended => "SUSP",
            Self::Completing => "CG",
            Self::Completed => "CD",
            Self::Cancelled => "CA",
            Self::Failed => "FAIL",
            Self::Timeout => "TO",
            Self::Preempted => "PR",
            Self::NodeFail => "NF",
            Self::BootFail => "BF",
            Self::Deadline => "DL",
            Self::OutOfMemory => "OOM",
        }
    }
}
