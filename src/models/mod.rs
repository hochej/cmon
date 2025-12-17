//! Data models for Slurm JSON responses.
//!
//! This module provides all the data structures needed to interact with Slurm,
//! including job and node information, fairshare data, scheduler statistics,
//! and configuration types.

mod config;
mod fairshare;
mod job;
mod node;
mod scheduler;
mod slurm_responses;
mod state;
mod time;

// Re-export formatting functions for backwards compatibility
pub use crate::formatting::format_duration_human as format_duration_seconds;
pub use crate::formatting::format_duration_human_minutes as format_duration_minutes;

// Re-export all public types from submodules
pub use config::{BehaviorConfig, DisplayConfig, RefreshConfig, SystemConfig, TuiConfig};
pub use fairshare::{
    FairshareNode, FlatFairshareRow, SshareEntry, SshareFairshare, SshareResponse, SshareTres,
    SshareTresItem, SshareWrapper,
};
pub use job::{
    CpuStatistics, ExitCodeInfo, JobAssociation, JobGpuInfo, JobHistoryInfo, JobHistoryState,
    JobInfo, JobRequiredResources, JobStepInfo, JobStepStatistics, JobStepTimeInfo, JobTimeInfo,
    JobTresInfo, MemoryMaxInfo, MemoryStatistics, MemoryTaskInfo, SignalInfo, StepId, StepIdInfo,
    TasksInfo, TimeSeconds, TresItem,
};
pub use node::{
    CpuInfo, FeatureInfo, GpuInfo, GresInfo, MemoryFreeInfo, MemoryInfo, MinMaxValue, NodeInfo,
    NodeNames, NodeState, PartitionInfo, ReasonInfo,
};
pub use scheduler::{BackfillStats, CycleStats, SchedulerStats};
pub use slurm_responses::{
    ClusterStatus, PersonalSummary, SacctResponse, SacctWarning, SinfoResponse, SqueueResponse,
};
pub use state::JobState;
pub use time::{FloatValue, TimeValue};

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
        assert_eq!(status.total_cpus(), 384); // 3 nodes x 128 CPUs
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
