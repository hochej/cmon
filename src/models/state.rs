//! Job state enum and related types.
//!
//! This module defines the authoritative JobState enum used throughout the codebase,
//! consolidating what was previously duplicated between different parts of the code.

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
