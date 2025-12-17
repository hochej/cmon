//! Scheduler statistics from sdiag.
//!
//! This module contains types for parsing and representing Slurm scheduler
//! statistics from the sdiag command.

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
