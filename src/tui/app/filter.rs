//! Job filtering logic for the TUI
//!
//! This module provides filtering functionality for job lists with support for:
//! - Plain text search across multiple fields
//! - Field-prefixed filters (e.g., `user:john`, `partition:gpu`)
//! - Negation with `!` prefix
//! - Multiple terms with AND logic

use super::types::TuiJobInfo;

/// Check if a job matches the filter string
///
/// Supports:
/// - Plain text: matches against name, user, account, partition, job_id
/// - Field prefix: `field:value` for specific field matching
///   - name:, user:, account:, partition:, state:, qos:, gpu:, node:
/// - Negation: `!field:value` to exclude matches
/// - Multiple terms: separated by spaces, all must match (AND logic)
pub fn job_matches_filter(job: &TuiJobInfo, filter: &Option<String>) -> bool {
    let filter_str = match filter {
        Some(f) if !f.is_empty() => f,
        _ => return true, // No filter = match all
    };

    // Split filter into terms (space-separated)
    let terms: Vec<&str> = filter_str.split_whitespace().collect();

    // All terms must match (AND logic)
    terms.iter().all(|term| {
        let (negated, term) = if let Some(stripped) = term.strip_prefix('!') {
            (true, stripped)
        } else {
            (false, *term)
        };

        let matches = if let Some(colon_pos) = term.find(':') {
            // Field-prefixed filter
            let field = &term[..colon_pos].to_lowercase();
            let value = &term[colon_pos + 1..].to_lowercase();
            job_matches_field(job, field, value)
        } else {
            // Plain text search across multiple fields
            job_matches_any_field(job, term)
        };

        if negated { !matches } else { matches }
    })
}

/// Match a job against a specific field
pub fn job_matches_field(job: &TuiJobInfo, field: &str, value: &str) -> bool {
    match field {
        "name" | "n" => job.name.to_lowercase().contains(value),
        "user" | "u" => job.user_name.to_lowercase().contains(value),
        "account" | "acct" | "a" => job.account.to_lowercase().contains(value),
        "partition" | "part" | "p" => job.partition.to_lowercase().contains(value),
        "state" | "s" => {
            job.state.as_str().to_lowercase().contains(value)
                || job.state.short_str().to_lowercase().contains(value)
        }
        "qos" | "q" => job.qos.to_lowercase().contains(value),
        "gpu" | "gpus" | "g" => job_matches_gpu(job, value),
        "node" | "nodes" => job.nodes.to_lowercase().contains(value),
        "id" | "job" | "jobid" => job.job_id.to_string().contains(value),
        "reason" | "r" => job.state_reason.to_lowercase().contains(value),
        _ => false, // Unknown field prefix
    }
}

/// Match GPU filter (handles count, boolean, or type matching)
pub fn job_matches_gpu(job: &TuiJobInfo, value: &str) -> bool {
    if let Ok(count) = value.parse::<u32>() {
        job.gpu_count == count
    } else {
        match value {
            "yes" | "true" | "any" => job.gpu_count > 0,
            "no" | "false" | "none" => job.gpu_count == 0,
            _ => {
                // Match GPU type
                job.gpu_type
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(value))
                    .unwrap_or(false)
            }
        }
    }
}

/// Match a job against any searchable field (plain text search)
pub fn job_matches_any_field(job: &TuiJobInfo, term: &str) -> bool {
    let value = term.to_lowercase();
    job.name.to_lowercase().contains(&value)
        || job.user_name.to_lowercase().contains(&value)
        || job.account.to_lowercase().contains(&value)
        || job.partition.to_lowercase().contains(&value)
        || job.job_id.to_string().contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::JobState;
    use std::num::NonZeroU64;

    fn make_test_job() -> TuiJobInfo {
        use super::super::types::{JobId, SlurmTime};
        use std::collections::HashMap;

        TuiJobInfo {
            job_id: JobId {
                base_id: NonZeroU64::new(12345).unwrap(),
                array_task_id: None,
            },
            name: "test_job".to_string(),
            user_name: "testuser".to_string(),
            account: "research".to_string(),
            partition: "gpu".to_string(),
            state: JobState::Running,
            state_raw: "RUNNING".to_string(),
            state_reason: "None".to_string(),
            priority: 1000,
            qos: "normal".to_string(),
            submit_time: SlurmTime::default(),
            start_time: SlurmTime::default(),
            end_time: SlurmTime::default(),
            time_limit_seconds: 3600,
            elapsed_seconds: 1800,
            nodes: "node001".to_string(),
            node_count: 1,
            cpus: 4,
            ntasks: 1,
            cpus_per_task: 4,
            ntasks_per_node: None,
            constraint: String::new(),
            tres_requested: HashMap::new(),
            tres_allocated: HashMap::new(),
            gpu_count: 2,
            gpu_type: Some("A100".to_string()),
            memory_gb: 32.0,
            working_directory: "/home/testuser".to_string(),
            stdout_path: String::new(),
            stderr_path: String::new(),
            dependency: String::new(),
            array_job_id: None,
            array_task_count: None,
            array_tasks_pending: None,
            array_tasks_running: None,
            array_tasks_completed: None,
        }
    }

    #[test]
    fn test_no_filter() {
        let job = make_test_job();
        assert!(job_matches_filter(&job, &None));
        assert!(job_matches_filter(&job, &Some(String::new())));
    }

    #[test]
    fn test_plain_text_filter() {
        let job = make_test_job();
        assert!(job_matches_filter(&job, &Some("test".to_string())));
        assert!(job_matches_filter(&job, &Some("research".to_string())));
        assert!(!job_matches_filter(&job, &Some("nonexistent".to_string())));
    }

    #[test]
    fn test_field_filter() {
        let job = make_test_job();
        assert!(job_matches_filter(&job, &Some("user:testuser".to_string())));
        assert!(job_matches_filter(&job, &Some("partition:gpu".to_string())));
        assert!(!job_matches_filter(&job, &Some("partition:cpu".to_string())));
    }

    #[test]
    fn test_negation() {
        let job = make_test_job();
        assert!(!job_matches_filter(&job, &Some("!partition:gpu".to_string())));
        assert!(job_matches_filter(&job, &Some("!partition:cpu".to_string())));
    }

    #[test]
    fn test_gpu_filter() {
        let job = make_test_job();
        assert!(job_matches_filter(&job, &Some("gpu:2".to_string())));
        assert!(job_matches_filter(&job, &Some("gpu:yes".to_string())));
        assert!(job_matches_filter(&job, &Some("gpu:a100".to_string())));
        assert!(!job_matches_filter(&job, &Some("gpu:0".to_string())));
        assert!(!job_matches_filter(&job, &Some("gpu:no".to_string())));
    }

    #[test]
    fn test_combined_filters() {
        let job = make_test_job();
        assert!(job_matches_filter(
            &job,
            &Some("user:testuser partition:gpu".to_string())
        ));
        assert!(!job_matches_filter(
            &job,
            &Some("user:testuser partition:cpu".to_string())
        ));
    }
}
