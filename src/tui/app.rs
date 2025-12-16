//! Application state and core logic for the TUI
//!
//! This module contains the main App struct and all associated state management.
//! The architecture follows a TEA-inspired pattern with mutable state and method-based updates.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::formatting::format_duration_hms;
use crate::models::{
    FairshareNode, FlatFairshareRow, JobInfo, JobState, NodeInfo, SchedulerStats, SshareEntry,
    TuiConfig,
};
use crate::tui::event::{DataEvent, EventResult, InputEvent, KeyAction};

/// Confirmation action types
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    CancelJob {
        job_id: u64,
        job_name: String,
    },
    CancelJobArray {
        base_job_id: u64,
        job_name: String,
        task_count: u32,
    },
}

impl ConfirmAction {
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            ConfirmAction::CancelJob { job_id, job_name } => {
                format!("Cancel job {} ({})?", job_id, job_name)
            }
            ConfirmAction::CancelJobArray {
                base_job_id,
                job_name,
                task_count,
            } => {
                format!(
                    "Cancel job array {} ({}) with {} tasks?",
                    base_job_id, job_name, task_count
                )
            }
        }
    }

    #[must_use]
    pub fn job_id(&self) -> u64 {
        match self {
            ConfirmAction::CancelJob { job_id, .. } => *job_id,
            ConfirmAction::CancelJobArray { base_job_id, .. } => *base_job_id,
        }
    }
}

/// Sort menu state
#[derive(Debug, Default)]
pub struct SortMenuState {
    pub selected: usize,
    pub columns: Vec<(&'static str, JobSortColumn)>,
}

impl SortMenuState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            columns: vec![
                ("Job ID", JobSortColumn::JobId),
                ("Name", JobSortColumn::Name),
                ("Account", JobSortColumn::Account),
                ("Partition", JobSortColumn::Partition),
                ("State", JobSortColumn::State),
                ("Time", JobSortColumn::Time),
                ("Priority", JobSortColumn::Priority),
                ("GPUs", JobSortColumn::Gpus),
            ],
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected < self.columns.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    #[must_use]
    pub fn selected_column(&self) -> Option<JobSortColumn> {
        self.columns.get(self.selected).map(|(_, col)| *col)
    }
}

/// Filter type for distinguishing quick search vs advanced filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterType {
    #[default]
    QuickSearch, // `/` - filters by name only
    Advanced, // `f` - full filter dialog with field selection
}

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
}

/// Clipboard operation result for visual feedback
#[derive(Debug, Clone)]
pub struct ClipboardFeedback {
    pub message: String,
    pub success: bool,
    pub timestamp: Instant,
}

impl ClipboardFeedback {
    pub fn success(message: String) -> Self {
        Self {
            message,
            success: true,
            timestamp: Instant::now(),
        }
    }

    pub fn failure(message: String) -> Self {
        Self {
            message,
            success: false,
            timestamp: Instant::now(),
        }
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.timestamp.elapsed() < Duration::from_secs(2)
    }
}

/// Time value with explicit unknown handling (0 = unknown in Slurm)
#[derive(Debug, Clone, Copy, Default)]
pub struct SlurmTime(i64);

impl SlurmTime {
    #[must_use]
    pub fn from_epoch(epoch: i64) -> Self {
        Self(epoch)
    }

    #[must_use]
    pub fn is_known(&self) -> bool {
        self.0 > 0 // Slurm uses 0 for "not set"
    }

    #[must_use]
    pub fn as_epoch(&self) -> Option<i64> {
        if self.is_known() { Some(self.0) } else { None }
    }

    #[must_use]
    pub fn as_datetime(&self) -> Option<chrono::DateTime<chrono::Local>> {
        use chrono::TimeZone;
        self.as_epoch()
            .and_then(|e| chrono::Local.timestamp_opt(e, 0).single())
    }
}

/// Job ID supporting both regular jobs and array jobs
///
/// Uses `NonZeroU64` for `base_id` because a zero job ID is never valid in Slurm,
/// making this invariant structural and enabling niche optimization for `Option<JobId>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobId {
    pub base_id: NonZeroU64,
    pub array_task_id: Option<u32>,
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.array_task_id {
            Some(task) => write!(f, "{}_{}", self.base_id.get(), task),
            None => write!(f, "{}", self.base_id.get()),
        }
    }
}

/// Extended job information for TUI display
#[derive(Debug, Clone)]
pub struct TuiJobInfo {
    pub job_id: JobId,
    pub name: String,
    pub user_name: String,
    pub account: String,
    pub partition: String,

    // State information
    pub state: JobState,
    #[allow(dead_code)]
    pub state_raw: String,
    pub state_reason: String,

    // Priority and QOS
    pub priority: u32,
    pub qos: String,

    // Time information
    pub submit_time: SlurmTime,
    pub start_time: SlurmTime,
    pub end_time: SlurmTime,
    pub time_limit_seconds: u32,
    pub elapsed_seconds: u32,

    // Resources
    pub nodes: String,
    pub node_count: u32,
    pub cpus: u32,

    // Job shape
    pub ntasks: u32,
    pub cpus_per_task: u32,
    #[allow(dead_code)]
    pub ntasks_per_node: Option<u32>,
    pub constraint: String,

    // TRES resources
    #[allow(dead_code)]
    pub tres_requested: HashMap<String, f64>,
    #[allow(dead_code)]
    pub tres_allocated: HashMap<String, f64>,

    // Computed GPU info
    pub gpu_count: u32,
    pub gpu_type: Option<String>,
    pub memory_gb: f64,

    // Paths
    pub working_directory: String,
    pub stdout_path: String,
    pub stderr_path: String,

    // Dependencies and array info
    pub dependency: String,
    pub array_job_id: Option<u64>,
    pub array_task_count: Option<u32>,
    pub array_tasks_pending: Option<u32>,
    pub array_tasks_running: Option<u32>,
    pub array_tasks_completed: Option<u32>,
}

impl TuiJobInfo {
    /// Convert from models::JobInfo
    pub fn from_job_info(job: &JobInfo) -> Self {
        // Get the first state string for display
        let state_str = job.state.first().map(|s| s.as_str()).unwrap_or("UNKNOWN");
        let state = JobState::from_state_string(state_str);

        // Get GPU info from allocated resources
        let gpu_info = job.gpu_type_info();

        // Calculate elapsed time from start_time if running
        let elapsed_seconds = if let Some(start) = job.start_time.value() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now.saturating_sub(start) as u32
        } else {
            0
        };

        // Extract array task id from job_id if present (format: "12345_67")
        let array_task_id = None; // Not directly available in current JobInfo

        Self {
            job_id: JobId {
                base_id: NonZeroU64::new(job.job_id)
                    .expect("Slurm job IDs should never be zero"),
                array_task_id,
            },
            name: job.name.clone(),
            user_name: job.user_name.clone(),
            account: job.account.clone(),
            partition: job.partition.clone(),

            state,
            state_raw: state_str.to_string(),
            state_reason: job.state_reason.clone(),

            priority: job.priority.number() as u32,
            qos: job.qos.clone(),

            submit_time: job
                .submit_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            start_time: job
                .start_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            end_time: job
                .end_time
                .value()
                .map(|n| SlurmTime::from_epoch(n as i64))
                .unwrap_or_default(),
            time_limit_seconds: job.time_limit.value().map(|n| (n * 60) as u32).unwrap_or(0),
            elapsed_seconds,

            nodes: job.nodes.clone(),
            node_count: 1, // Simplified - could parse from nodes string
            cpus: job.cpus_per_task.number() as u32 * job.tasks.number().max(1) as u32,

            ntasks: job.tasks.value().map(|n| n as u32).unwrap_or(1),
            cpus_per_task: job.cpus_per_task.value().map(|n| n as u32).unwrap_or(1),
            ntasks_per_node: None,
            constraint: String::new(),

            tres_requested: HashMap::new(),
            tres_allocated: HashMap::new(),

            gpu_count: gpu_info.count,
            gpu_type: if gpu_info.gpu_type.is_empty() {
                None
            } else {
                Some(gpu_info.gpu_type)
            },
            memory_gb: 0.0, // Not directly available

            working_directory: job.current_working_directory.clone(),
            stdout_path: String::new(),
            stderr_path: String::new(),

            dependency: String::new(),
            array_job_id: job.array_job_id.value(),
            array_task_count: None,
            array_tasks_pending: None,
            array_tasks_running: None,
            array_tasks_completed: None,
        }
    }

    #[must_use]
    pub fn is_array_job(&self) -> bool {
        self.job_id.array_task_id.is_some() || self.array_job_id.is_some()
    }

    #[must_use]
    pub fn time_remaining(&self) -> Option<Duration> {
        if self.state == JobState::Running && self.time_limit_seconds > 0 {
            let remaining = self.time_limit_seconds.saturating_sub(self.elapsed_seconds);
            Some(Duration::from_secs(remaining as u64))
        } else {
            None
        }
    }

    #[must_use]
    pub fn elapsed_display(&self) -> String {
        format_duration_hms(self.elapsed_seconds as u64)
    }

    #[must_use]
    pub fn time_limit_display(&self) -> String {
        if self.time_limit_seconds == 0 {
            "-".to_string()
        } else {
            format_duration_hms(self.time_limit_seconds as u64)
        }
    }

    /// Get estimated start time display for pending jobs
    #[must_use]
    pub fn estimated_start_display(&self) -> String {
        if self.state != JobState::Pending {
            return "-".to_string();
        }

        // For pending jobs, start_time contains the estimated start
        if let Some(dt) = self.start_time.as_datetime() {
            let now = chrono::Local::now();
            let duration = dt.signed_duration_since(now);

            if duration.num_seconds() <= 0 {
                "soon".to_string()
            } else if duration.num_hours() < 1 {
                format!("~{}m", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("~{}h", duration.num_hours())
            } else {
                format!("~{}d", duration.num_days())
            }
        } else {
            "N/A".to_string()
        }
    }
}

/// Partition status summary
#[derive(Debug, Clone)]
pub struct PartitionStatus {
    pub name: String,
    pub total_nodes: u32,
    pub available_nodes: u32,
    pub down_nodes: u32,
    pub draining_nodes: u32,
    pub idle_nodes: u32,
    pub allocated_nodes: u32,
    pub mixed_nodes: u32,
    pub total_cpus: u32,
    pub allocated_cpus: u32,
    pub total_memory_gb: f64,
    pub allocated_memory_gb: f64,
    pub total_gpus: u32,
    pub allocated_gpus: u32,
    pub gpu_type: Option<String>,
    pub running_jobs: u32,
    pub pending_jobs: u32,
}

impl PartitionStatus {
    #[must_use]
    pub fn cpu_utilization(&self) -> f64 {
        if self.total_cpus == 0 {
            0.0
        } else {
            self.allocated_cpus as f64 / self.total_cpus as f64 * 100.0
        }
    }

    #[must_use]
    pub fn memory_utilization(&self) -> f64 {
        if self.total_memory_gb < 0.01 {
            0.0
        } else {
            self.allocated_memory_gb / self.total_memory_gb * 100.0
        }
    }

    #[must_use]
    pub fn gpu_utilization(&self) -> f64 {
        if self.total_gpus == 0 {
            0.0
        } else {
            self.allocated_gpus as f64 / self.total_gpus as f64 * 100.0
        }
    }
}

/// Data slice with staleness tracking
///
/// Encapsulates data with timestamp tracking. The `data` field is private to ensure
/// all updates go through `update()`, which properly sets `last_updated`.
/// Use `iter()` and `get()` for read access.
#[derive(Debug)]
pub struct DataSlice<T> {
    data: Vec<T>,
    pub last_updated: Option<Instant>,
    pub stale_threshold: Duration,
}

impl<T> Default for DataSlice<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            last_updated: None,
            stale_threshold: Duration::from_secs(30),
        }
    }
}

impl<T> DataSlice<T> {
    pub fn new(stale_threshold: Duration) -> Self {
        Self {
            data: Vec::new(),
            last_updated: None,
            stale_threshold,
        }
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.last_updated
            .map(|t| t.elapsed() > self.stale_threshold)
            .unwrap_or(true)
    }

    #[must_use]
    pub fn age(&self) -> Option<Duration> {
        self.last_updated.map(|t| t.elapsed())
    }

    pub fn update(&mut self, data: Vec<T>) {
        self.data = data;
        self.last_updated = Some(Instant::now());
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns an iterator over the data
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Returns a reference to the element at the given index, if it exists
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    /// Returns the data as a slice for efficient read-only access
    ///
    /// Use this when you need indexed access to multiple elements.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

/// List state with selection and scroll tracking
#[derive(Debug, Clone, Default)]
pub struct ListState {
    pub selected: usize,
    pub scroll_offset: usize,
    pub visible_count: usize,
}

impl ListState {
    pub fn clamp(&mut self, list_len: usize) {
        if list_len == 0 {
            self.selected = 0;
            self.scroll_offset = 0;
        } else {
            self.selected = self.selected.min(list_len - 1);
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            } else if self.visible_count > 0
                && self.selected >= self.scroll_offset + self.visible_count
            {
                self.scroll_offset = self.selected.saturating_sub(self.visible_count - 1);
            }
        }
    }

    pub fn move_up(&mut self, list_len: usize) {
        if self.selected > 0 {
            self.selected -= 1;
            self.clamp(list_len);
        }
    }

    pub fn move_down(&mut self, list_len: usize) {
        if list_len > 0 && self.selected < list_len - 1 {
            self.selected += 1;
            self.clamp(list_len);
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn move_to_bottom(&mut self, list_len: usize) {
        if list_len > 0 {
            self.selected = list_len - 1;
            if self.visible_count > 0 {
                self.scroll_offset = list_len.saturating_sub(self.visible_count);
            }
        }
    }

    pub fn page_up(&mut self, list_len: usize) {
        let jump = self.visible_count.max(1) / 2;
        self.selected = self.selected.saturating_sub(jump);
        self.clamp(list_len);
    }

    pub fn page_down(&mut self, list_len: usize) {
        let jump = self.visible_count.max(1) / 2;
        self.selected = self.selected.saturating_add(jump);
        self.clamp(list_len);
    }
}

/// Sort column for jobs view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JobSortColumn {
    #[default]
    JobId,
    Name,
    Account,
    Partition,
    State,
    Time,
    Priority,
    Gpus,
}

/// Jobs view state
#[derive(Debug, Default)]
pub struct JobsViewState {
    pub list_state: ListState,
    pub sort_column: JobSortColumn,
    pub sort_ascending: bool,
    pub show_grouped_by_account: bool,
    pub collapsed_arrays: HashSet<u64>,
    filtered_cache: Option<Vec<usize>>,
    cache_invalidated: bool,
}

impl JobsViewState {
    pub fn invalidate_cache(&mut self) {
        self.cache_invalidated = true;
        self.filtered_cache = None;
    }

    /// Toggle collapse state for an array job
    #[allow(dead_code)]
    pub fn toggle_array_collapse(&mut self, base_job_id: u64) {
        if self.collapsed_arrays.contains(&base_job_id) {
            self.collapsed_arrays.remove(&base_job_id);
        } else {
            self.collapsed_arrays.insert(base_job_id);
        }
        self.invalidate_cache();
    }

    /// Check if an array job is collapsed
    #[must_use]
    pub fn is_array_collapsed(&self, base_job_id: u64) -> bool {
        self.collapsed_arrays.contains(&base_job_id)
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_filtered_indices(
        &mut self,
        jobs: &[TuiJobInfo],
        filter: &Option<String>,
    ) -> Vec<usize> {
        // Simple filter by name for now
        jobs.iter()
            .enumerate()
            .filter(|(_, job)| {
                if let Some(f) = filter {
                    job.name.to_lowercase().contains(&f.to_lowercase())
                        || job.user_name.to_lowercase().contains(&f.to_lowercase())
                        || job.account.to_lowercase().contains(&f.to_lowercase())
                } else {
                    true
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Get filtered and sorted job indices
    ///
    /// Filter syntax:
    /// - Plain text: searches across name, user, account, partition, job_id
    /// - Prefixed: `field:value` for specific field filtering
    ///   - name:test, user:john, account:bio, partition:gpu, state:running, qos:normal
    ///   - Multiple filters can be combined with spaces: `user:john state:running`
    ///   - Negation with !: `!partition:gpu` excludes GPU partition
    #[must_use]
    pub fn get_sorted_indices(&self, jobs: &[TuiJobInfo], filter: &Option<String>) -> Vec<usize> {
        let mut indices: Vec<usize> = jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| job_matches_filter(job, filter))
            .map(|(i, _)| i)
            .collect();

        // Sort based on current sort column
        let sort_column = self.sort_column;
        let ascending = self.sort_ascending;

        indices.sort_by(|&a, &b| {
            let job_a = &jobs[a];
            let job_b = &jobs[b];
            let cmp = match sort_column {
                JobSortColumn::JobId => job_a.job_id.base_id.cmp(&job_b.job_id.base_id),
                JobSortColumn::Name => job_a.name.to_lowercase().cmp(&job_b.name.to_lowercase()),
                JobSortColumn::Account => job_a
                    .account
                    .to_lowercase()
                    .cmp(&job_b.account.to_lowercase()),
                JobSortColumn::Partition => job_a
                    .partition
                    .to_lowercase()
                    .cmp(&job_b.partition.to_lowercase()),
                JobSortColumn::State => (job_a.state as u8).cmp(&(job_b.state as u8)),
                JobSortColumn::Time => job_a.elapsed_seconds.cmp(&job_b.elapsed_seconds),
                JobSortColumn::Priority => job_a.priority.cmp(&job_b.priority),
                JobSortColumn::Gpus => job_a.gpu_count.cmp(&job_b.gpu_count),
            };

            if ascending { cmp } else { cmp.reverse() }
        });

        indices
    }
}

/// Nodes view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodesViewMode {
    #[default]
    List,
    Grid,
}

/// Nodes view state
#[derive(Debug, Default)]
pub struct NodesViewState {
    pub list_state: ListState,
    pub view_mode: NodesViewMode,
    #[allow(dead_code)]
    pub partition_filter: Option<String>,
}

/// Partitions view state
#[derive(Debug, Default)]
pub struct PartitionsViewState {
    pub list_state: ListState,
    #[allow(dead_code)]
    pub show_account_breakdown: bool,
}

/// Personal view state
#[derive(Debug, Default)]
pub struct PersonalViewState {
    pub running_jobs_state: ListState,
    pub pending_jobs_state: ListState,
    pub fairshare_state: ListState,
    pub selected_panel: PersonalPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PersonalPanel {
    #[default]
    Running,
    Pending,
    Fairshare,
    Summary,
}

/// Problems view state
#[derive(Debug, Default)]
pub struct ProblemsViewState {
    pub down_nodes_state: ListState,
    pub draining_nodes_state: ListState,
    pub selected_panel: ProblemsPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProblemsPanel {
    #[default]
    Down,
    Draining,
}

/// Current view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Jobs,
    Nodes,
    Partitions,
    Personal,
    Problems,
}

impl View {
    #[must_use]
    pub fn next(&self) -> Self {
        match self {
            View::Jobs => View::Nodes,
            View::Nodes => View::Partitions,
            View::Partitions => View::Personal,
            View::Personal => View::Problems,
            View::Problems => View::Jobs,
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            View::Jobs => "Jobs",
            View::Nodes => "Nodes",
            View::Partitions => "Partitions",
            View::Personal => "Me",
            View::Problems => "Problems",
        }
    }
}

/// Account context for multi-account users
#[derive(Debug, Default)]
pub struct AccountContext {
    pub user_accounts: Vec<String>,
    pub focused_account: Option<String>,
}

impl AccountContext {
    /// Cycle through accounts: None -> first -> second -> ... -> last -> None
    pub fn cycle_account(&mut self) {
        if self.user_accounts.is_empty() {
            return;
        }

        let current_idx = self
            .focused_account
            .as_ref()
            .and_then(|acc| self.user_accounts.iter().position(|a| a == acc));

        self.focused_account = match current_idx {
            None => self.user_accounts.first().cloned(),
            Some(i) => self.user_accounts.get(i + 1).cloned(),
        };
    }

    #[must_use]
    pub fn display(&self) -> String {
        self.focused_account
            .clone()
            .unwrap_or_else(|| "all".to_string())
    }
}

// ============================================================================
// Grouped State Types (Refactored Architecture)
// ============================================================================

/// Modal overlay state - only one modal can be active at a time.
///
/// This enum replaces the scattered modal-related fields (mode, show_help,
/// confirm_action, sort_menu, filter editing state) with a unified type that
/// makes impossible states unrepresentable.
///
/// NOTE: Filter's edit_buffer is EPHEMERAL - it's the draft being typed.
/// The actual applied filter lives in DataCache.active_filter
#[derive(Debug, Default)]
pub enum ModalState {
    #[default]
    None,
    Help,
    /// Filter editing mode - edit_buffer is temporary draft
    Filter {
        edit_buffer: String,
        cursor: usize,
        filter_type: FilterType,
    },
    Detail,
    Sort {
        menu: SortMenuState,
    },
    Confirm {
        action: ConfirmAction,
    },
}

impl ModalState {
    /// Check if any modal is currently active
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, ModalState::None)
    }

    /// Check if the modal is blocking (requires explicit dismissal)
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            ModalState::Confirm { .. } | ModalState::Detail | ModalState::Sort { .. }
        )
    }

    /// Check if currently in filter editing mode
    #[must_use]
    pub fn is_editing_filter(&self) -> bool {
        matches!(self, ModalState::Filter { .. })
    }

    /// Get the confirm action if in confirm mode
    #[must_use]
    pub fn confirm_action(&self) -> Option<&ConfirmAction> {
        match self {
            ModalState::Confirm { action } => Some(action),
            _ => None,
        }
    }

    /// Get the sort menu if in sort mode
    #[must_use]
    pub fn sort_menu(&self) -> Option<&SortMenuState> {
        match self {
            ModalState::Sort { menu } => Some(menu),
            _ => None,
        }
    }

    /// Get mutable reference to sort menu
    #[must_use]
    pub fn sort_menu_mut(&mut self) -> Option<&mut SortMenuState> {
        match self {
            ModalState::Sort { menu } => Some(menu),
            _ => None,
        }
    }
}

/// Applied filter that persists when modal closes
#[derive(Debug, Clone, Default)]
pub struct ActiveFilter {
    pub text: String,
    pub filter_type: FilterType,
}

impl ActiveFilter {
    /// Get filter text as Option for filtering logic
    #[must_use]
    pub fn as_option(&self) -> Option<String> {
        if self.text.is_empty() {
            None
        } else {
            Some(self.text.clone())
        }
    }
}

/// Grouped data cache with staleness tracking
#[derive(Debug)]
pub struct DataCache {
    pub jobs: DataSlice<TuiJobInfo>,
    pub nodes: DataSlice<NodeInfo>,
    pub partitions: DataSlice<PartitionStatus>,
    pub fairshare: DataSlice<SshareEntry>,
    pub fairshare_tree: Vec<FlatFairshareRow>,
    pub scheduler_stats: Option<SchedulerStats>,

    /// Persistent filter state (survives modal close)
    pub active_filter: Option<ActiveFilter>,
}

impl DataCache {
    /// Create a new DataCache with configured stale thresholds
    pub fn new(config: &TuiConfig) -> Self {
        Self {
            jobs: DataSlice::new(Duration::from_secs(config.refresh.jobs_interval * 3)),
            nodes: DataSlice::new(Duration::from_secs(config.refresh.nodes_interval * 3)),
            partitions: DataSlice::new(Duration::from_secs(config.refresh.nodes_interval * 3)),
            fairshare: DataSlice::new(Duration::from_secs(config.refresh.fairshare_interval * 3)),
            fairshare_tree: Vec::new(),
            scheduler_stats: None,
            active_filter: None,
        }
    }

    /// Get the active filter text as Option for filtering logic
    #[must_use]
    pub fn get_filter(&self) -> Option<String> {
        self.active_filter.as_ref().and_then(|f| f.as_option())
    }

    /// Set the active filter
    pub fn set_filter(&mut self, text: String, filter_type: FilterType) {
        if text.is_empty() {
            self.active_filter = None;
        } else {
            self.active_filter = Some(ActiveFilter { text, filter_type });
        }
    }

    /// Clear the active filter
    pub fn clear_filter(&mut self) {
        self.active_filter = None;
    }
}

/// Unified feedback state for errors, warnings, and transient messages
#[derive(Debug)]
pub struct FeedbackState {
    last_error: Option<(String, Instant)>,
    error_display_duration: Duration,
    pub config_warnings: Vec<String>,
    clipboard_feedback: Option<ClipboardFeedback>,
}

impl FeedbackState {
    /// Create a new FeedbackState with config warnings
    pub fn new(config_warnings: Vec<String>) -> Self {
        Self {
            last_error: None,
            error_display_duration: Duration::from_secs(5),
            config_warnings,
            clipboard_feedback: None,
        }
    }

    /// Set an error message to display
    pub fn set_error(&mut self, msg: String) {
        self.last_error = Some((msg, Instant::now()));
    }

    /// Check if error should still be displayed
    #[must_use]
    pub fn should_show_error(&self) -> bool {
        self.last_error
            .as_ref()
            .map(|(_, t)| t.elapsed() < self.error_display_duration)
            .unwrap_or(false)
    }

    /// Get the current error message if it should be shown
    #[must_use]
    pub fn current_error(&self) -> Option<&str> {
        if self.should_show_error() {
            self.last_error.as_ref().map(|(msg, _)| msg.as_str())
        } else {
            None
        }
    }

    /// Set clipboard operation feedback
    pub fn set_clipboard_feedback(&mut self, feedback: ClipboardFeedback) {
        self.clipboard_feedback = Some(feedback);
    }

    /// Get current clipboard feedback if visible
    #[must_use]
    pub fn current_clipboard_feedback(&self) -> Option<&ClipboardFeedback> {
        self.clipboard_feedback.as_ref().filter(|f| f.is_visible())
    }

    /// Clear clipboard feedback
    pub fn clear_clipboard_feedback(&mut self) {
        self.clipboard_feedback = None;
    }
}

/// Grouped timing state for activity tracking
#[derive(Debug)]
pub struct TimingState {
    pub last_input: Instant,
    pub last_refresh: Option<Instant>,
}

impl Default for TimingState {
    fn default() -> Self {
        Self {
            last_input: Instant::now(),
            last_refresh: None,
        }
    }
}

/// Main application state
///
/// This struct has been refactored to group related fields:
/// - `modal`: Unified modal state (replaces mode, show_help, confirm_action, sort_menu, filter editing)
/// - `data`: Grouped data cache (replaces jobs, nodes, partitions, fairshare, etc.)
/// - `feedback`: Unified feedback state (replaces last_error, clipboard_feedback, config_warnings)
/// - `timing`: Grouped timing state (replaces last_input, last_refresh)
pub struct App {
    // Lifecycle
    pub running: bool,

    // View State
    pub current_view: View,
    pub previous_view: View,

    // Modal State (UNIFIED - replaces mode, show_help, confirm_action, sort_menu, filter editing)
    pub modal: ModalState,

    // Data (GROUPED - replaces jobs, nodes, partitions, fairshare, etc.)
    pub data: DataCache,

    // User Context
    pub username: String,
    pub show_all_jobs: bool,
    pub account_context: AccountContext,

    // Per-View States (unchanged - already well-designed)
    pub jobs_view: JobsViewState,
    pub nodes_view: NodesViewState,
    pub partitions_view: PartitionsViewState,
    pub personal_view: PersonalViewState,
    pub problems_view: ProblemsViewState,

    // Feedback (GROUPED - replaces last_error, clipboard_feedback, config_warnings)
    pub feedback: FeedbackState,

    // Timing (GROUPED - replaces last_input, last_refresh)
    pub timing: TimingState,

    // Configuration
    pub config: TuiConfig,
    pub slurm_bin_path: std::path::PathBuf,

    // Communication
    pub data_tx: mpsc::Sender<DataEvent>,
}

impl App {
    /// Create a new App instance with the required data channel sender.
    ///
    /// The `data_tx` channel is required for async operations like job cancellation.
    /// This ensures the type system prevents runtime errors from missing channels.
    pub fn new(data_tx: mpsc::Sender<DataEvent>) -> Self {
        let username = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let (config, config_warnings) = TuiConfig::load();

        Self {
            running: true,
            current_view: View::Jobs,
            previous_view: View::Jobs,

            // Unified modal state
            modal: ModalState::None,

            // Grouped data cache
            data: DataCache::new(&config),

            username,
            show_all_jobs: config.display.show_all_jobs,
            account_context: AccountContext::default(),

            jobs_view: JobsViewState {
                show_grouped_by_account: config.display.show_grouped_by_account,
                ..Default::default()
            },
            nodes_view: NodesViewState::default(),
            partitions_view: PartitionsViewState::default(),
            personal_view: PersonalViewState::default(),
            problems_view: ProblemsViewState::default(),

            // Grouped feedback state
            feedback: FeedbackState::new(config_warnings),

            // Grouped timing state
            timing: TimingState::default(),

            config,

            // Default empty - should be set via with_slurm_path()
            slurm_bin_path: std::path::PathBuf::new(),

            data_tx,
        }
    }

    /// Create App with a specific Slurm binary path
    pub fn with_slurm_path(mut self, path: std::path::PathBuf) -> Self {
        self.slurm_bin_path = path;
        self
    }

    /// Handle an input event
    pub fn handle_input(&mut self, event: InputEvent) -> EventResult {
        self.timing.last_input = Instant::now();

        match event {
            InputEvent::Key(key_event) => {
                let in_filter = self.modal.is_editing_filter();
                let action = KeyAction::from_key_event(key_event, in_filter);
                self.handle_action(action)
            }
            InputEvent::Resize(_, _) => EventResult::Continue,
            InputEvent::Mouse(mouse_event) => {
                let action = KeyAction::from_mouse_event(mouse_event);
                self.handle_action(action)
            }
        }
    }

    /// Handle a key action
    fn handle_action(&mut self, action: KeyAction) -> EventResult {
        // Help overlay takes priority
        if matches!(self.modal, ModalState::Help) {
            match action {
                KeyAction::Escape | KeyAction::ShowHelp | KeyAction::Quit => {
                    self.modal = ModalState::None;
                    return EventResult::Continue;
                }
                _ => return EventResult::Unchanged,
            }
        }

        // Modal modes take priority over normal navigation
        match &self.modal {
            ModalState::Filter { .. } => return self.handle_filter_action(action),
            ModalState::Confirm { .. } => return self.handle_confirm_action(action),
            ModalState::Sort { .. } => return self.handle_sort_action(action),
            ModalState::Detail => return self.handle_detail_action(action),
            _ => {}
        }

        // Handle navigation actions first (common pattern: call method, return Continue)
        if let Some(result) = self.handle_navigation(&action) {
            return result;
        }

        // Handle view switching actions
        if let Some(result) = self.handle_view_switch(&action) {
            return result;
        }

        match action {
            KeyAction::Quit => {
                self.running = false;
                EventResult::Quit
            }

            // Actions
            KeyAction::Select => {
                // Open detail view for selected item, or toggle array collapse
                if self.current_view == View::Jobs {
                    if self.selected_job().is_some() {
                        // Always show job detail view
                        self.modal = ModalState::Detail;
                    }
                } else if self.current_view == View::Personal {
                    // Allow job detail view from Personal view
                    if self.personal_running_job().is_some()
                        || self.personal_pending_job().is_some()
                    {
                        self.modal = ModalState::Detail;
                    }
                }
                EventResult::Continue
            }
            KeyAction::Cancel => {
                // Initiate job cancel confirmation
                if self.current_view == View::Jobs
                    && let Some(job) = self.selected_job()
                {
                    let confirm_action = if job.is_array_job() {
                        ConfirmAction::CancelJobArray {
                            base_job_id: job.job_id.base_id.get(),
                            job_name: job.name.clone(),
                            task_count: job.array_task_count.unwrap_or(1),
                        }
                    } else {
                        ConfirmAction::CancelJob {
                            job_id: job.job_id.base_id.get(),
                            job_name: job.name.clone(),
                        }
                    };
                    self.modal = ModalState::Confirm { action: confirm_action };
                }
                EventResult::Continue
            }
            KeyAction::ToggleAllJobs => {
                self.show_all_jobs = !self.show_all_jobs;
                self.jobs_view.invalidate_cache();
                EventResult::Continue
            }
            KeyAction::ToggleGroupByAccount => {
                if self.current_view == View::Jobs {
                    self.jobs_view.show_grouped_by_account =
                        !self.jobs_view.show_grouped_by_account;
                    self.jobs_view.invalidate_cache();
                }
                EventResult::Continue
            }
            KeyAction::QuickSearch => {
                // Open filter modal with current active filter as initial edit buffer
                let initial_text = self.data.get_filter().unwrap_or_default();
                self.modal = ModalState::Filter {
                    edit_buffer: initial_text.clone(),
                    cursor: initial_text.len(),
                    filter_type: FilterType::QuickSearch,
                };
                EventResult::Continue
            }
            KeyAction::OpenFilter => {
                // Open filter modal with current active filter as initial edit buffer
                let initial_text = self.data.get_filter().unwrap_or_default();
                self.modal = ModalState::Filter {
                    edit_buffer: initial_text.clone(),
                    cursor: initial_text.len(),
                    filter_type: FilterType::Advanced,
                };
                EventResult::Continue
            }
            KeyAction::OpenSort => {
                if self.current_view == View::Jobs {
                    self.modal = ModalState::Sort { menu: SortMenuState::new() };
                }
                EventResult::Continue
            }
            KeyAction::YankJobId => {
                self.yank_selected_job_id();
                EventResult::Continue
            }
            KeyAction::ShowHelp => {
                self.modal = ModalState::Help;
                EventResult::Continue
            }
            KeyAction::CycleAccount => {
                self.account_context.cycle_account();
                EventResult::Continue
            }
            KeyAction::ToggleViewMode => {
                if self.current_view == View::Nodes {
                    self.nodes_view.view_mode = match self.nodes_view.view_mode {
                        NodesViewMode::List => NodesViewMode::Grid,
                        NodesViewMode::Grid => NodesViewMode::List,
                    };
                }
                EventResult::Continue
            }
            KeyAction::ExportData => {
                // Export current view data to JSON file
                self.export_current_view(ExportFormat::Json);
                EventResult::Continue
            }
            KeyAction::ExportDataCsv => {
                // Export current view data to CSV file
                self.export_current_view(ExportFormat::Csv);
                EventResult::Continue
            }
            KeyAction::Escape => {
                // Clear active filter when pressing escape
                if self.data.active_filter.is_some() {
                    self.data.clear_filter();
                    self.jobs_view.invalidate_cache();
                }
                EventResult::Continue
            }

            // Unhandled - force refresh triggers data refetch
            KeyAction::Refresh => EventResult::Continue,

            // Mouse click (scroll handled by handle_navigation)
            KeyAction::MouseClick { row, column: _ } => {
                self.handle_mouse_click(row);
                EventResult::Continue
            }

            _ => EventResult::Unchanged,
        }
    }

    /// Handle navigation actions (returns Some if action was handled)
    fn handle_navigation(&mut self, action: &KeyAction) -> Option<EventResult> {
        match action {
            KeyAction::MoveUp | KeyAction::MouseScrollUp => {
                self.navigate_up();
                Some(EventResult::Continue)
            }
            KeyAction::MoveDown | KeyAction::MouseScrollDown => {
                self.navigate_down();
                Some(EventResult::Continue)
            }
            KeyAction::MoveToTop => {
                self.navigate_to_top();
                Some(EventResult::Continue)
            }
            KeyAction::MoveToBottom => {
                self.navigate_to_bottom();
                Some(EventResult::Continue)
            }
            KeyAction::PageUp => {
                self.page_up();
                Some(EventResult::Continue)
            }
            KeyAction::PageDown => {
                self.page_down();
                Some(EventResult::Continue)
            }
            _ => None,
        }
    }

    /// Handle view switching actions (returns Some if action was handled)
    fn handle_view_switch(&mut self, action: &KeyAction) -> Option<EventResult> {
        match action {
            KeyAction::SwitchToJobs => {
                self.switch_view(View::Jobs);
                Some(EventResult::Continue)
            }
            KeyAction::SwitchToNodes => {
                self.switch_view(View::Nodes);
                Some(EventResult::Continue)
            }
            KeyAction::SwitchToPartitions => {
                self.switch_view(View::Partitions);
                Some(EventResult::Continue)
            }
            KeyAction::SwitchToPersonal => {
                self.switch_view(View::Personal);
                Some(EventResult::Continue)
            }
            KeyAction::SwitchToProblems => {
                self.switch_view(View::Problems);
                Some(EventResult::Continue)
            }
            KeyAction::NextView => {
                if self.view_has_panels() {
                    self.cycle_panel();
                } else {
                    self.switch_view(self.current_view.next());
                }
                Some(EventResult::Continue)
            }
            _ => None,
        }
    }

    /// Handle mouse click to select row in list views
    fn handle_mouse_click(&mut self, row: u16) {
        // Skip if in a modal mode
        if self.modal.is_active() {
            return;
        }

        // Calculate content area (accounting for header, tabs, and borders)
        // Typical layout: row 0 = title, row 1 = tabs, row 2-3 = info bar/header
        // Content starts around row 4 (after table header)
        const CONTENT_START: u16 = 5;

        if row < CONTENT_START {
            return;
        }

        let clicked_index = (row - CONTENT_START) as usize;
        let len = self.current_list_len();

        if len == 0 {
            return;
        }

        match self.current_view {
            View::Jobs => {
                let target = self.jobs_view.list_state.scroll_offset + clicked_index;
                if target < len {
                    self.jobs_view.list_state.selected = target;
                }
            }
            View::Nodes => {
                let target = self.nodes_view.list_state.scroll_offset + clicked_index;
                if target < len {
                    self.nodes_view.list_state.selected = target;
                }
            }
            View::Partitions => {
                let target = self.partitions_view.list_state.scroll_offset + clicked_index;
                if target < len {
                    self.partitions_view.list_state.selected = target;
                }
            }
            _ => {}
        }
    }

    /// Handle actions in confirm dialog mode
    fn handle_confirm_action(&mut self, action: KeyAction) -> EventResult {
        match action {
            KeyAction::Escape => {
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::Select => {
                // Execute the confirmed action
                if let ModalState::Confirm { action } = std::mem::take(&mut self.modal) {
                    self.execute_cancel_job(action.job_id());
                }
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::FilterChar('y') | KeyAction::FilterChar('Y') => {
                // 'y' for yes in confirm dialog
                if let ModalState::Confirm { action } = std::mem::take(&mut self.modal) {
                    self.execute_cancel_job(action.job_id());
                }
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::FilterChar('n') | KeyAction::FilterChar('N') => {
                // 'n' for no in confirm dialog
                self.modal = ModalState::None;
                EventResult::Continue
            }
            _ => EventResult::Unchanged,
        }
    }

    /// Handle actions in sort menu mode
    fn handle_sort_action(&mut self, action: KeyAction) -> EventResult {
        match action {
            KeyAction::Escape => {
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::MoveUp => {
                if let Some(menu) = self.modal.sort_menu_mut() {
                    menu.move_up();
                }
                EventResult::Continue
            }
            KeyAction::MoveDown => {
                if let Some(menu) = self.modal.sort_menu_mut() {
                    menu.move_down();
                }
                EventResult::Continue
            }
            KeyAction::Select => {
                if let Some(menu) = self.modal.sort_menu() {
                    if let Some(column) = menu.selected_column() {
                        // Toggle direction if same column, otherwise set ascending
                        if self.jobs_view.sort_column == column {
                            self.jobs_view.sort_ascending = !self.jobs_view.sort_ascending;
                        } else {
                            self.jobs_view.sort_column = column;
                            self.jobs_view.sort_ascending = true;
                        }
                        self.jobs_view.invalidate_cache();
                    }
                }
                self.modal = ModalState::None;
                EventResult::Continue
            }
            _ => EventResult::Unchanged,
        }
    }

    /// Handle actions in detail view mode
    fn handle_detail_action(&mut self, action: KeyAction) -> EventResult {
        match action {
            KeyAction::Escape | KeyAction::Select => {
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::Cancel => {
                // Allow initiating cancel from detail view
                if let Some(job) = self.selected_job() {
                    let confirm_action = if job.is_array_job() {
                        ConfirmAction::CancelJobArray {
                            base_job_id: job.job_id.base_id.get(),
                            job_name: job.name.clone(),
                            task_count: job.array_task_count.unwrap_or(1),
                        }
                    } else {
                        ConfirmAction::CancelJob {
                            job_id: job.job_id.base_id.get(),
                            job_name: job.name.clone(),
                        }
                    };
                    self.modal = ModalState::Confirm { action: confirm_action };
                }
                EventResult::Continue
            }
            KeyAction::YankJobId => {
                self.yank_selected_job_id();
                EventResult::Continue
            }
            _ => EventResult::Unchanged,
        }
    }

    /// Execute scancel for a job asynchronously to avoid UI freeze
    ///
    /// Spawns a background task that runs the scancel command and sends
    /// the result back through the data channel. This ensures the TUI
    /// remains responsive even if the Slurm scheduler is slow.
    fn execute_cancel_job(&mut self, job_id: u64) {
        let data_tx = self.data_tx.clone();

        // Show immediate feedback that cancellation is in progress
        self.feedback.set_clipboard_feedback(ClipboardFeedback::success(format!(
            "Cancelling job {}...",
            job_id
        )));

        let scancel_path = self.slurm_bin_path.join("scancel");

        // Spawn async task to run scancel in background
        tokio::spawn(async move {
            // Run blocking command in a separate thread pool
            let result = tokio::task::spawn_blocking(move || {
                use std::process::{Command, Stdio};

                let output = Command::new(&scancel_path)
                    .arg(job_id.to_string())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output();

                match output {
                    Ok(out) => {
                        if out.status.success() {
                            (true, format!("Cancelled job {}", job_id))
                        } else {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            (false, format!("Failed to cancel job {}: {}", job_id, stderr.trim()))
                        }
                    }
                    Err(e) => (false, format!("Failed to execute scancel: {}", e)),
                }
            })
            .await;

            // Send result back through data channel
            let (success, message) = match result {
                Ok((success, msg)) => (success, msg),
                Err(e) => (false, format!("Task error: {}", e)),
            };

            let _ = data_tx.send(DataEvent::JobCancelResult {
                job_id,
                success,
                message,
            }).await;
        });
    }

    /// Copy selected job ID to clipboard
    fn yank_selected_job_id(&mut self) {
        if let Some(job) = self.selected_job() {
            let job_id_str = job.job_id.to_string();

            // Try using xclip, xsel, or pbcopy depending on platform
            let result = self.copy_to_clipboard(&job_id_str);

            self.feedback.set_clipboard_feedback(if result {
                ClipboardFeedback::success(format!("Copied: {}", job_id_str))
            } else {
                ClipboardFeedback::failure("Failed to copy (no clipboard)".to_string())
            });
        }
    }

    /// Attempt to copy text to system clipboard
    fn copy_to_clipboard(&self, text: &str) -> bool {
        // Try multiple clipboard tools in order of preference
        let clipboard_commands = [
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
            ("pbcopy", vec![]),  // macOS
            ("wl-copy", vec![]), // Wayland
        ];

        for (cmd, args) in clipboard_commands {
            if let Ok(mut child) = std::process::Command::new(cmd)
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                && let Some(mut stdin) = child.stdin.take()
            {
                use std::io::Write;
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if let Ok(status) = child.wait()
                        && status.success()
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn handle_filter_action(&mut self, action: KeyAction) -> EventResult {
        match action {
            KeyAction::Escape => {
                // Cancel filter editing - discard edit buffer, keep previous filter
                self.modal = ModalState::None;
                EventResult::Continue
            }
            KeyAction::Select => {
                // Apply filter - move edit buffer to active filter
                if let ModalState::Filter { edit_buffer, filter_type, .. } = &self.modal {
                    self.data.set_filter(edit_buffer.clone(), *filter_type);
                }
                self.modal = ModalState::None;
                self.jobs_view.invalidate_cache();
                EventResult::Continue
            }
            KeyAction::FilterClear => {
                // Clear the edit buffer
                if let ModalState::Filter { edit_buffer, cursor, .. } = &mut self.modal {
                    edit_buffer.clear();
                    *cursor = 0;
                }
                EventResult::Continue
            }
            KeyAction::FilterBackspace => {
                // Backspace in edit buffer
                if let ModalState::Filter { edit_buffer, cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                        edit_buffer.remove(*cursor);
                    }
                }
                self.jobs_view.invalidate_cache();
                EventResult::Continue
            }
            KeyAction::FilterChar(c) => {
                // Insert character in edit buffer
                if let ModalState::Filter { edit_buffer, cursor, .. } = &mut self.modal {
                    edit_buffer.insert(*cursor, c);
                    *cursor += 1;
                }
                self.jobs_view.invalidate_cache();
                EventResult::Continue
            }
            _ => EventResult::Unchanged,
        }
    }

    /// Handle a data event
    pub fn handle_data(&mut self, event: DataEvent) -> EventResult {
        match event {
            DataEvent::JobsUpdated(jobs) => {
                self.data.jobs.update(jobs);
                self.jobs_view.list_state.clamp(self.data.jobs.len());
                self.timing.last_refresh = Some(Instant::now());

                // Extract unique accounts
                let accounts: HashSet<_> =
                    self.data.jobs.iter().map(|j| j.account.clone()).collect();
                self.account_context.user_accounts = accounts.into_iter().collect();
                self.account_context.user_accounts.sort();

                EventResult::Continue
            }
            DataEvent::NodesUpdated(nodes) => {
                self.data.nodes.update(nodes);
                self.nodes_view.list_state.clamp(self.data.nodes.len());
                EventResult::Continue
            }
            DataEvent::PartitionsUpdated(partitions) => {
                self.data.partitions.update(partitions);
                self.partitions_view.list_state.clamp(self.data.partitions.len());
                EventResult::Continue
            }
            DataEvent::FairshareUpdated(entries) => {
                self.data.fairshare.update(entries);
                // Build the flattened tree for display
                let entries: Vec<_> = self.data.fairshare.iter().cloned().collect();
                let tree_roots = FairshareNode::build_tree(&entries, &self.username);
                self.data.fairshare_tree = tree_roots.iter().flat_map(|node| node.flatten()).collect();
                EventResult::Continue
            }
            DataEvent::SchedulerStatsUpdated(stats) => {
                self.data.scheduler_stats = Some(stats);
                EventResult::Continue
            }
            DataEvent::FetchError { source, error } => {
                self.feedback.set_error(format!("{}: {}", source, error));
                EventResult::Continue
            }
            DataEvent::AnimationTick => {
                // Only redraw if we need animation (e.g., spinner visible)
                if self.data.jobs.last_updated.is_none() {
                    EventResult::Continue
                } else {
                    EventResult::Unchanged
                }
            }
            DataEvent::ForceRefresh => EventResult::Continue,
            DataEvent::Shutdown => {
                self.running = false;
                EventResult::Quit
            }
            DataEvent::JobCancelResult {
                job_id: _,
                success,
                message,
            } => {
                if success {
                    self.feedback.set_clipboard_feedback(ClipboardFeedback::success(message));
                } else {
                    self.feedback.set_error(message);
                    // Clear the "Cancelling..." feedback on failure
                    self.feedback.clear_clipboard_feedback();
                }
                EventResult::Continue
            }
        }
    }

    fn switch_view(&mut self, view: View) {
        self.previous_view = self.current_view;
        self.current_view = view;
    }

    /// Helper to apply a navigation operation to the currently active list.
    /// The closure receives a mutable reference to the active ListState and the list length.
    /// Returns early (no-op) for views/panels that have no navigable list (e.g., Summary panel).
    fn with_current_list<F>(&mut self, f: F)
    where
        F: FnOnce(&mut ListState, usize),
    {
        let len = self.current_list_len();
        match self.current_view {
            View::Jobs => f(&mut self.jobs_view.list_state, len),
            View::Nodes => f(&mut self.nodes_view.list_state, len),
            View::Partitions => f(&mut self.partitions_view.list_state, len),
            View::Personal => match self.personal_view.selected_panel {
                PersonalPanel::Running => f(&mut self.personal_view.running_jobs_state, len),
                PersonalPanel::Pending => f(&mut self.personal_view.pending_jobs_state, len),
                PersonalPanel::Fairshare => f(&mut self.personal_view.fairshare_state, len),
                PersonalPanel::Summary => {} // Summary has no list
            },
            View::Problems => match self.problems_view.selected_panel {
                ProblemsPanel::Down => f(&mut self.problems_view.down_nodes_state, len),
                ProblemsPanel::Draining => f(&mut self.problems_view.draining_nodes_state, len),
            },
        }
    }

    fn navigate_up(&mut self) {
        self.with_current_list(|state, len| state.move_up(len));
    }

    fn navigate_down(&mut self) {
        self.with_current_list(|state, len| state.move_down(len));
    }

    fn navigate_to_top(&mut self) {
        self.with_current_list(|state, _len| state.move_to_top());
    }

    fn navigate_to_bottom(&mut self) {
        self.with_current_list(|state, len| state.move_to_bottom(len));
    }

    fn page_up(&mut self) {
        self.with_current_list(|state, len| state.page_up(len));
    }

    fn page_down(&mut self) {
        self.with_current_list(|state, len| state.page_down(len));
    }

    fn current_list_len(&self) -> usize {
        match self.current_view {
            View::Jobs => self.data.jobs.len(),
            View::Nodes => self.data.nodes.len(),
            View::Partitions => self.data.partitions.len(),
            View::Personal => match self.personal_view.selected_panel {
                PersonalPanel::Running => self.my_running_jobs().len(),
                PersonalPanel::Pending => self.my_pending_jobs().len(),
                PersonalPanel::Fairshare => self.data.fairshare_tree.len(),
                PersonalPanel::Summary => 0,
            },
            View::Problems => match self.problems_view.selected_panel {
                ProblemsPanel::Down => self.down_nodes().len(),
                ProblemsPanel::Draining => self.draining_nodes().len(),
            },
        }
    }

    /// Cycle between panels in views that have multiple panels
    fn cycle_panel(&mut self) {
        match self.current_view {
            View::Personal => {
                // Determine if fairshare panel should be included (only if data available)
                let has_fairshare = !self.data.fairshare_tree.is_empty();
                self.personal_view.selected_panel = match self.personal_view.selected_panel {
                    PersonalPanel::Summary => {
                        if has_fairshare {
                            PersonalPanel::Fairshare
                        } else {
                            PersonalPanel::Running
                        }
                    }
                    PersonalPanel::Fairshare => PersonalPanel::Running,
                    PersonalPanel::Running => PersonalPanel::Pending,
                    PersonalPanel::Pending => PersonalPanel::Summary,
                };
            }
            View::Problems => {
                self.problems_view.selected_panel = match self.problems_view.selected_panel {
                    ProblemsPanel::Down => ProblemsPanel::Draining,
                    ProblemsPanel::Draining => ProblemsPanel::Down,
                };
            }
            _ => {} // Other views don't have panels
        }
    }

    /// Check if current view has panels to cycle
    fn view_has_panels(&self) -> bool {
        matches!(self.current_view, View::Personal | View::Problems)
    }

    /// Get the currently selected job (if in Jobs view)
    #[must_use]
    pub fn selected_job(&self) -> Option<&TuiJobInfo> {
        if self.current_view == View::Jobs {
            self.data.jobs.get(self.jobs_view.list_state.selected)
        } else {
            None
        }
    }

    /// Check if error should still be displayed
    #[must_use]
    pub fn should_show_error(&self) -> bool {
        self.feedback.should_show_error()
    }

    /// Get the current error message if it should be shown
    #[must_use]
    pub fn current_error(&self) -> Option<&str> {
        self.feedback.current_error()
    }

    /// Compute partition statistics from nodes data
    ///
    /// Groups nodes by their actual Slurm partition name (portable across clusters).
    /// Display order is configurable via config; defaults to alphabetical.
    #[must_use]
    pub fn compute_partition_stats(&self) -> Vec<PartitionStatus> {
        let mut partition_map: HashMap<String, Vec<&NodeInfo>> = HashMap::new();

        // Group nodes by partition name from Slurm (preserves original case)
        for node in self.data.nodes.iter() {
            partition_map
                .entry(node.partition_name())
                .or_default()
                .push(node);
        }

        // Use configured partition order, or empty (alphabetical) as default
        let partition_order = &self.config.display.partition_order;
        let mut stats: Vec<PartitionStatus> = Vec::new();

        // First add configured partitions in order (case-insensitive match)
        for config_name in partition_order {
            // Find the actual partition key that matches case-insensitively
            if let Some(actual_name) = partition_map
                .keys()
                .find(|k| k.eq_ignore_ascii_case(config_name))
                .cloned()
            {
                if let Some(nodes) = partition_map.remove(&actual_name) {
                    stats.push(compute_partition_from_nodes(&actual_name, &nodes));
                }
            }
        }

        // Then add any remaining partitions alphabetically (case-insensitive sort)
        let mut remaining: Vec<_> = partition_map.into_iter().collect();
        remaining.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (name, nodes) in remaining {
            stats.push(compute_partition_from_nodes(&name, &nodes));
        }

        stats
    }

    /// Get user's running jobs
    #[must_use]
    pub fn my_running_jobs(&self) -> Vec<&TuiJobInfo> {
        self.data.jobs
            .iter()
            .filter(|j| {
                j.state == JobState::Running && (self.show_all_jobs || j.user_name == self.username)
            })
            .collect()
    }

    /// Get user's pending jobs
    #[must_use]
    pub fn my_pending_jobs(&self) -> Vec<&TuiJobInfo> {
        self.data.jobs
            .iter()
            .filter(|j| {
                j.state == JobState::Pending && (self.show_all_jobs || j.user_name == self.username)
            })
            .collect()
    }

    /// Get down nodes
    #[must_use]
    pub fn down_nodes(&self) -> Vec<&NodeInfo> {
        self.data.nodes
            .iter()
            .filter(|n| n.is_down() || n.is_fail())
            .collect()
    }

    /// Get draining nodes
    #[must_use]
    pub fn draining_nodes(&self) -> Vec<&NodeInfo> {
        self.data.nodes
            .iter()
            .filter(|n| n.is_draining() || n.is_drained())
            .collect()
    }

    /// Get total running job count
    #[must_use]
    pub fn running_job_count(&self) -> usize {
        self.data.jobs
            .iter()
            .filter(|j| j.state == JobState::Running)
            .count()
    }

    /// Get total pending job count
    #[must_use]
    pub fn pending_job_count(&self) -> usize {
        self.data.jobs
            .iter()
            .filter(|j| j.state == JobState::Pending)
            .count()
    }

    /// Get currently selected node (if in Nodes view)
    #[must_use]
    pub fn selected_node(&self) -> Option<&NodeInfo> {
        if self.current_view == View::Nodes {
            self.data.nodes.get(self.nodes_view.list_state.selected)
        } else {
            None
        }
    }

    /// Get selected running job from Personal view (if focused on Running panel)
    #[must_use]
    pub fn personal_running_job(&self) -> Option<&TuiJobInfo> {
        if self.current_view == View::Personal
            && self.personal_view.selected_panel == PersonalPanel::Running
        {
            let running_jobs = self.my_running_jobs();
            let idx = self.personal_view.running_jobs_state.selected;
            running_jobs.get(idx).copied()
        } else {
            None
        }
    }

    /// Get selected pending job from Personal view (if focused on Pending panel)
    #[must_use]
    pub fn personal_pending_job(&self) -> Option<&TuiJobInfo> {
        if self.current_view == View::Personal
            && self.personal_view.selected_panel == PersonalPanel::Pending
        {
            let pending_jobs = self.my_pending_jobs();
            let idx = self.personal_view.pending_jobs_state.selected;
            pending_jobs.get(idx).copied()
        } else {
            None
        }
    }

    /// Get the job to show in detail view (works from Jobs or Personal view)
    #[must_use]
    pub fn detail_job(&self) -> Option<&TuiJobInfo> {
        match self.current_view {
            View::Jobs => self.selected_job(),
            View::Personal => match self.personal_view.selected_panel {
                PersonalPanel::Running => {
                    let jobs = self.my_running_jobs();
                    let idx = self.personal_view.running_jobs_state.selected;
                    jobs.get(idx).copied()
                }
                PersonalPanel::Pending => {
                    let jobs = self.my_pending_jobs();
                    let idx = self.personal_view.pending_jobs_state.selected;
                    jobs.get(idx).copied()
                }
                PersonalPanel::Fairshare | PersonalPanel::Summary => None,
            },
            _ => None,
        }
    }

    /// Get current clipboard feedback if visible
    #[must_use]
    pub fn current_clipboard_feedback(&self) -> Option<&ClipboardFeedback> {
        self.feedback.current_clipboard_feedback()
    }

    /// Get array job summary (for collapsed display)
    /// Returns (running_count, pending_count, completed_count, max_elapsed) for an array job
    pub fn array_job_summary(&self, base_job_id: u64) -> (usize, usize, usize, u32) {
        let mut running = 0;
        let mut pending = 0;
        let mut completed = 0;
        let mut max_elapsed = 0u32;

        for job in self.data.jobs.iter() {
            if job.job_id.base_id.get() == base_job_id {
                match job.state {
                    JobState::Running => running += 1,
                    JobState::Pending => pending += 1,
                    JobState::Completed
                    | JobState::Failed
                    | JobState::Cancelled
                    | JobState::Timeout
                    | JobState::OutOfMemory
                    | JobState::NodeFail => completed += 1,
                    _ => {}
                }
                max_elapsed = max_elapsed.max(job.elapsed_seconds);
            }
        }

        (running, pending, completed, max_elapsed)
    }

    /// Check if a job ID represents a visible job (considering array collapse)
    #[must_use]
    pub fn is_job_visible(&self, job: &TuiJobInfo) -> bool {
        if !job.is_array_job() {
            return true;
        }

        // For array jobs, only show if:
        // 1. The array is not collapsed (show all tasks), OR
        // 2. This is the first task of a collapsed array

        if !self.jobs_view.is_array_collapsed(job.job_id.base_id.get()) {
            return true;
        }

        // For collapsed arrays, only show the first task as a summary
        // We consider it the "first" if no other task with the same base_id and lower task_id exists
        job.job_id.array_task_id.is_none_or(|task_id| {
            !self.data.jobs.iter().any(|other| {
                other.job_id.base_id == job.job_id.base_id
                    && other
                        .job_id
                        .array_task_id
                        .is_some_and(|other_id| other_id < task_id)
            })
        })
    }

    /// Get sorted and filtered jobs for display
    #[must_use]
    pub fn get_display_jobs(&self) -> Vec<&TuiJobInfo> {
        let filter = self.data.get_filter();
        let jobs = self.data.jobs.as_slice();
        let indices = self.jobs_view.get_sorted_indices(jobs, &filter);
        indices.iter().map(|&i| &jobs[i]).collect()
    }

    /// Check if in a modal dialog
    #[must_use]
    pub fn is_modal_active(&self) -> bool {
        self.modal.is_blocking()
    }

    /// Export current view data to a file
    pub fn export_current_view(&mut self, format: ExportFormat) {
        match self.current_view {
            View::Jobs => self.export_jobs(format),
            View::Nodes => self.export_nodes(format),
            View::Partitions => self.export_partitions(format),
            _ => {
                self.feedback.set_clipboard_feedback(ClipboardFeedback::failure(
                    "Export not supported for this view".to_string(),
                ));
            }
        }
    }

    /// Export jobs to file (JSON or CSV)
    fn export_jobs(&mut self, format: ExportFormat) {
        let jobs = self.get_display_jobs();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

        match format {
            ExportFormat::Json => {
                let export_data: Vec<serde_json::Value> = jobs
                    .iter()
                    .map(|job| {
                        serde_json::json!({
                            "job_id": job.job_id.to_string(),
                            "name": job.name,
                            "user": job.user_name,
                            "account": job.account,
                            "partition": job.partition,
                            "state": job.state.as_str(),
                            "state_reason": job.state_reason,
                            "priority": job.priority,
                            "qos": job.qos,
                            "cpus": job.cpus,
                            "gpus": job.gpu_count,
                            "gpu_type": job.gpu_type,
                            "nodes": job.nodes,
                            "elapsed_seconds": job.elapsed_seconds,
                            "time_limit_seconds": job.time_limit_seconds,
                            "working_directory": job.working_directory,
                        })
                    })
                    .collect();

                let filename = format!("cmon_jobs_{}.json", timestamp);
                match serde_json::to_string_pretty(&export_data) {
                    Ok(json_str) => {
                        self.write_export_file(&filename, &json_str, jobs.len(), "jobs")
                    }
                    Err(e) => {
                        self.feedback.set_clipboard_feedback(ClipboardFeedback::failure(format!(
                            "Failed to serialize jobs: {}",
                            e
                        )));
                    }
                }
            }
            ExportFormat::Csv => {
                let mut csv = String::new();
                // CSV header
                csv.push_str("job_id,name,user,account,partition,state,state_reason,priority,qos,cpus,gpus,gpu_type,nodes,elapsed_seconds,time_limit_seconds,time_remaining_seconds,working_directory\n");
                // CSV rows
                for job in &jobs {
                    let time_remaining = job.time_remaining().map(|d| d.as_secs()).unwrap_or(0);
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                        job.job_id,
                        escape_csv(&job.name),
                        escape_csv(&job.user_name),
                        escape_csv(&job.account),
                        escape_csv(&job.partition),
                        job.state.as_str(),
                        escape_csv(&job.state_reason),
                        job.priority,
                        escape_csv(&job.qos),
                        job.cpus,
                        job.gpu_count,
                        job.gpu_type.as_deref().unwrap_or(""),
                        escape_csv(&job.nodes),
                        job.elapsed_seconds,
                        job.time_limit_seconds,
                        time_remaining,
                        escape_csv(&job.working_directory),
                    ));
                }

                let filename = format!("cmon_jobs_{}.csv", timestamp);
                self.write_export_file(&filename, &csv, jobs.len(), "jobs");
            }
        }
    }

    /// Export nodes to file (JSON or CSV)
    fn export_nodes(&mut self, format: ExportFormat) {
        let nodes = self.data.nodes.as_slice();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

        match format {
            ExportFormat::Json => {
                let export_data: Vec<serde_json::Value> = nodes
                    .iter()
                    .map(|node| {
                        let gpu_info = node.gpu_info();
                        serde_json::json!({
                            "name": node.node_names.nodes.first().unwrap_or(&"".to_string()),
                            "partition": node.partition.name,
                            "state": node.primary_state(),
                            "cpus_allocated": node.cpus.allocated,
                            "cpus_total": node.cpus.total,
                            "memory_allocated_mb": node.memory.allocated,
                            "memory_total_mb": node.memory.minimum,
                            "gpus_used": gpu_info.used,
                            "gpus_total": gpu_info.total,
                            "gpu_type": gpu_info.gpu_type,
                        })
                    })
                    .collect();

                let filename = format!("cmon_nodes_{}.json", timestamp);
                match serde_json::to_string_pretty(&export_data) {
                    Ok(json_str) => {
                        self.write_export_file(&filename, &json_str, nodes.len(), "nodes")
                    }
                    Err(e) => {
                        self.feedback.set_clipboard_feedback(ClipboardFeedback::failure(format!(
                            "Failed to serialize nodes: {}",
                            e
                        )));
                    }
                }
            }
            ExportFormat::Csv => {
                let mut csv = String::new();
                // CSV header
                csv.push_str("name,partition,state,cpus_allocated,cpus_total,memory_allocated_mb,memory_total_mb,gpus_used,gpus_total,gpu_type\n");
                // CSV rows
                for node in nodes {
                    let gpu_info = node.gpu_info();
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{}\n",
                        node.name(),
                        node.partition.name.as_deref().unwrap_or(""),
                        node.primary_state(),
                        node.cpus.allocated,
                        node.cpus.total,
                        node.memory.allocated,
                        node.memory.minimum,
                        gpu_info.used,
                        gpu_info.total,
                        gpu_info.gpu_type,
                    ));
                }

                let filename = format!("cmon_nodes_{}.csv", timestamp);
                self.write_export_file(&filename, &csv, nodes.len(), "nodes");
            }
        }
    }

    /// Export partitions to file (JSON or CSV)
    fn export_partitions(&mut self, format: ExportFormat) {
        let partitions = self.compute_partition_stats();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");

        match format {
            ExportFormat::Json => {
                let export_data: Vec<serde_json::Value> = partitions
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "name": p.name,
                            "total_nodes": p.total_nodes,
                            "available_nodes": p.available_nodes,
                            "down_nodes": p.down_nodes,
                            "total_cpus": p.total_cpus,
                            "allocated_cpus": p.allocated_cpus,
                            "cpu_utilization": p.cpu_utilization(),
                            "total_gpus": p.total_gpus,
                            "allocated_gpus": p.allocated_gpus,
                            "gpu_utilization": p.gpu_utilization(),
                            "gpu_type": p.gpu_type,
                        })
                    })
                    .collect();

                let filename = format!("cmon_partitions_{}.json", timestamp);
                match serde_json::to_string_pretty(&export_data) {
                    Ok(json_str) => {
                        self.write_export_file(&filename, &json_str, partitions.len(), "partitions")
                    }
                    Err(e) => {
                        self.feedback.set_clipboard_feedback(ClipboardFeedback::failure(format!(
                            "Failed to serialize partitions: {}",
                            e
                        )));
                    }
                }
            }
            ExportFormat::Csv => {
                let mut csv = String::new();
                // CSV header
                csv.push_str("name,total_nodes,available_nodes,down_nodes,total_cpus,allocated_cpus,cpu_utilization,total_gpus,allocated_gpus,gpu_utilization,gpu_type\n");
                // CSV rows
                for p in &partitions {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{:.1},{},{},{:.1},{}\n",
                        p.name,
                        p.total_nodes,
                        p.available_nodes,
                        p.down_nodes,
                        p.total_cpus,
                        p.allocated_cpus,
                        p.cpu_utilization(),
                        p.total_gpus,
                        p.allocated_gpus,
                        p.gpu_utilization(),
                        p.gpu_type.as_deref().unwrap_or(""),
                    ));
                }

                let filename = format!("cmon_partitions_{}.csv", timestamp);
                self.write_export_file(&filename, &csv, partitions.len(), "partitions");
            }
        }
    }

    /// Helper to write export file and set feedback
    fn write_export_file(&mut self, filename: &str, content: &str, count: usize, item_type: &str) {
        match std::fs::write(filename, content) {
            Ok(_) => {
                self.feedback.set_clipboard_feedback(ClipboardFeedback::success(format!(
                    "Exported {} {} to {}",
                    count, item_type, filename
                )));
            }
            Err(e) => {
                self.feedback.set_clipboard_feedback(ClipboardFeedback::failure(format!(
                    "Failed to write {}: {}",
                    filename, e
                )));
            }
        }
    }
}

/// Escape a string for CSV (handle commas, quotes, newlines)
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Check if a job matches the filter string
///
/// Supports:
/// - Plain text: matches against name, user, account, partition, job_id
/// - Field prefix: `field:value` for specific field matching
///   - name:, user:, account:, partition:, state:, qos:, gpu:, node:
/// - Negation: `!field:value` to exclude matches
/// - Multiple terms: separated by spaces, all must match (AND logic)
fn job_matches_filter(job: &TuiJobInfo, filter: &Option<String>) -> bool {
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
fn job_matches_field(job: &TuiJobInfo, field: &str, value: &str) -> bool {
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
fn job_matches_gpu(job: &TuiJobInfo, value: &str) -> bool {
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
fn job_matches_any_field(job: &TuiJobInfo, term: &str) -> bool {
    let value = term.to_lowercase();
    job.name.to_lowercase().contains(&value)
        || job.user_name.to_lowercase().contains(&value)
        || job.account.to_lowercase().contains(&value)
        || job.partition.to_lowercase().contains(&value)
        || job.job_id.to_string().contains(&value)
}

/// Helper to compute partition stats from a list of nodes
fn compute_partition_from_nodes(name: &str, nodes: &[&NodeInfo]) -> PartitionStatus {
    let total_nodes = nodes.len() as u32;

    // Node state counts
    let down_nodes = nodes.iter().filter(|n| n.is_down() || n.is_fail()).count() as u32;
    let draining_nodes = nodes
        .iter()
        .filter(|n| n.is_draining() || n.is_drained())
        .count() as u32;
    let idle_nodes = nodes
        .iter()
        .filter(|n| n.is_idle() && !n.is_draining() && !n.is_down())
        .count() as u32;
    let allocated_nodes = nodes
        .iter()
        .filter(|n| n.is_allocated() && !n.is_draining())
        .count() as u32;
    let mixed_nodes = nodes
        .iter()
        .filter(|n| n.is_mixed() && !n.is_draining())
        .count() as u32;
    let available_nodes = total_nodes - down_nodes;

    // CPU stats
    let total_cpus: u32 = nodes.iter().map(|n| n.cpus.total).sum();
    let allocated_cpus: u32 = nodes.iter().map(|n| n.cpus.allocated).sum();

    // Memory stats (convert from MB to GB)
    let total_memory_gb: f64 = nodes
        .iter()
        .map(|n| n.memory_total() as f64 / 1024.0) // MB to GB
        .sum();
    let allocated_memory_gb: f64 = nodes
        .iter()
        .map(|n| {
            let total = n.memory_total() as f64;
            let free = n.memory_free() as f64;
            (total - free) / 1024.0 // Used memory in GB
        })
        .sum();

    // GPU stats
    let mut total_gpus: u32 = 0;
    let mut allocated_gpus: u32 = 0;
    let mut gpu_type: Option<String> = None;

    for node in nodes {
        let gpu_info = node.gpu_info();
        total_gpus += gpu_info.total;
        allocated_gpus += gpu_info.used;
        if gpu_type.is_none() && !gpu_info.gpu_type.is_empty() {
            gpu_type = Some(gpu_info.gpu_type.to_uppercase());
        }
    }

    PartitionStatus {
        name: name.to_string(),
        total_nodes,
        available_nodes,
        down_nodes,
        draining_nodes,
        idle_nodes,
        allocated_nodes,
        mixed_nodes,
        total_cpus,
        allocated_cpus,
        total_memory_gb,
        allocated_memory_gb,
        total_gpus,
        allocated_gpus,
        gpu_type,
        running_jobs: 0, // Would need job data to compute
        pending_jobs: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slurm_time() {
        let known = SlurmTime::from_epoch(1700000000);
        assert!(known.is_known());
        assert_eq!(known.as_epoch(), Some(1700000000));

        let unknown = SlurmTime::from_epoch(0);
        assert!(!unknown.is_known());
        assert_eq!(unknown.as_epoch(), None);
    }

    #[test]
    fn test_job_state_parsing() {
        assert_eq!(JobState::from_state_string("RUNNING"), JobState::Running);
        assert_eq!(JobState::from_state_string("PENDING"), JobState::Pending);
        assert_eq!(
            JobState::from_state_string("CANCELLED by 12345"),
            JobState::Cancelled
        );
    }

    #[test]
    fn test_list_state_navigation() {
        let mut state = ListState::default();
        state.visible_count = 10;

        state.move_down(5);
        assert_eq!(state.selected, 1);

        state.move_to_bottom(5);
        assert_eq!(state.selected, 4);

        state.move_to_top();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_view_cycling() {
        assert_eq!(View::Jobs.next(), View::Nodes);
        assert_eq!(View::Nodes.next(), View::Partitions);
        assert_eq!(View::Problems.next(), View::Jobs);
    }

    #[test]
    fn test_account_cycling() {
        let mut ctx = AccountContext {
            user_accounts: vec!["admin".to_string(), "bio".to_string()],
            focused_account: None,
        };

        ctx.cycle_account();
        assert_eq!(ctx.focused_account, Some("admin".to_string()));

        ctx.cycle_account();
        assert_eq!(ctx.focused_account, Some("bio".to_string()));

        ctx.cycle_account();
        assert_eq!(ctx.focused_account, None);
    }

    #[test]
    fn test_format_duration_hms() {
        assert_eq!(format_duration_hms(3661), "01:01:01");
        assert_eq!(format_duration_hms(86400 + 3661), "1-01:01:01");
    }
}
