//! Application state types for the TUI
//!
//! This module contains all the state management types:
//! - View states (Jobs, Nodes, Partitions, Personal, Problems)
//! - Modal states (Help, Filter, Detail, Sort, Confirm)
//! - Selection and navigation state (ListState)
//! - Data caching with staleness tracking (DataCache)
//! - Feedback state for errors and notifications

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::models::{FlatFairshareRow, NodeInfo, SchedulerStats, SshareEntry, TuiConfig};

use super::filter::job_matches_filter;
use super::types::{DataSlice, PartitionStatus, TuiJobInfo};

// ============================================================================
// Confirmation and Action Types
// ============================================================================

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

// ============================================================================
// Sort Menu State
// ============================================================================

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

// ============================================================================
// Filter Types
// ============================================================================

/// Filter type for distinguishing quick search vs advanced filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterType {
    #[default]
    QuickSearch, // `/` - filters by name only
    Advanced, // `f` - full filter dialog with field selection
}

/// Applied filter that persists when modal closes
#[derive(Debug, Clone, Default)]
pub struct ActiveFilter {
    pub text: String,
    #[allow(dead_code)]
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

// ============================================================================
// Export Types
// ============================================================================

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
}

// ============================================================================
// Clipboard Feedback
// ============================================================================

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

// ============================================================================
// List Navigation State
// ============================================================================

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

// ============================================================================
// Per-View State Types
// ============================================================================

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
    /// Create a new JobsViewState with the specified grouped_by_account setting
    pub fn new(show_grouped_by_account: bool) -> Self {
        Self {
            show_grouped_by_account,
            ..Default::default()
        }
    }

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

// ============================================================================
// View Enum
// ============================================================================

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

// ============================================================================
// Account Context
// ============================================================================

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
// Modal State
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

// ============================================================================
// Data Cache
// ============================================================================

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

// ============================================================================
// Feedback State
// ============================================================================

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

// ============================================================================
// Timing State
// ============================================================================

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
