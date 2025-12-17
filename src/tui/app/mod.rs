//! Application state and core logic for the TUI
//!
//! This module contains the main App struct and all associated state management.
//! The architecture follows a TEA-inspired pattern with mutable state and method-based updates.

// Submodules
mod export;
mod filter;
mod state;
mod types;

// Re-export public types
pub use export::export_items;
pub use state::{
    AccountContext, ClipboardFeedback, ConfirmAction, DataCache, ExportFormat, FeedbackState,
    FilterType, JobSortColumn, JobsViewState, ListState, ModalState, NodesViewMode, NodesViewState,
    PartitionsViewState, PersonalPanel, PersonalViewState, ProblemsPanel, ProblemsViewState,
    SortMenuState, TimingState, View,
};
pub use types::{PartitionStatus, TuiJobInfo};

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use tokio::sync::mpsc;

use crate::models::{FairshareNode, JobState, NodeInfo, TuiConfig};
use crate::tui::event::{DataEvent, EventResult, InputEvent, KeyAction};
use crate::utils::find_partition_key;

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

            jobs_view: JobsViewState::new(config.display.show_grouped_by_account),
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
                // Open detail view for selected job (works from Jobs or Personal view)
                if self.focused_job().is_some() {
                    self.modal = ModalState::Detail;
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
                // Allow initiating cancel from detail view (works from Jobs or Personal view)
                if let Some(job) = self.focused_job() {
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
                success,
                message,
            }).await;
        });
    }

    /// Copy focused job ID to clipboard (works from Jobs or Personal view)
    fn yank_selected_job_id(&mut self) {
        if let Some(job) = self.focused_job() {
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
            DataEvent::JobCancelResult { success, message } => {
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
            if let Some(actual_name) = find_partition_key(partition_map.keys(), config_name).cloned()
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

    /// Get the currently focused job across any view where a job can be selected
    ///
    /// This is the unified accessor that consolidates:
    /// - `selected_job()` (Jobs view)
    /// - `personal_running_job()` (Personal view, Running panel)
    /// - `personal_pending_job()` (Personal view, Pending panel)
    ///
    /// Use this method when you need the selected job regardless of which view is active.
    #[must_use]
    pub fn focused_job(&self) -> Option<&TuiJobInfo> {
        match self.current_view {
            View::Jobs => self.selected_job(),
            View::Personal => self.personal_running_job().or_else(|| self.personal_pending_job()),
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
                    // All terminal states count as "completed" for array progress
                    JobState::Completed
                    | JobState::Failed
                    | JobState::Cancelled
                    | JobState::Timeout
                    | JobState::OutOfMemory
                    | JobState::NodeFail
                    | JobState::BootFail
                    | JobState::Deadline
                    | JobState::Preempted => completed += 1,
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
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
        };
        let filename = format!("cmon_jobs_{}.{}", timestamp, extension);
        let content = export_items(&jobs, format);
        self.write_export_file(&filename, &content, jobs.len(), "jobs");
    }

    /// Export nodes to file (JSON or CSV)
    fn export_nodes(&mut self, format: ExportFormat) {
        let nodes = self.data.nodes.as_slice();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
        };
        let filename = format!("cmon_nodes_{}.{}", timestamp, extension);
        let content = export_items(nodes, format);
        self.write_export_file(&filename, &content, nodes.len(), "nodes");
    }

    /// Export partitions to file (JSON or CSV)
    fn export_partitions(&mut self, format: ExportFormat) {
        let partitions = self.compute_partition_stats();
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let extension = match format {
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
        };
        let filename = format!("cmon_partitions_{}.{}", timestamp, extension);
        let content = export_items(&partitions, format);
        self.write_export_file(&filename, &content, partitions.len(), "partitions");
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

