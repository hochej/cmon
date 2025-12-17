//! Event types for the TUI
//!
//! This module implements a dual-channel event architecture:
//! - InputEvent: Priority channel for user input (never dropped)
//! - DataEvent: Data channel for updates (may be dropped under load)

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

/// Input events from the terminal (priority channel - never dropped)
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// Mouse input (optional feature)
    Mouse(MouseEvent),
    /// Terminal resize
    #[allow(dead_code)]
    Resize(u16, u16),
}

/// Data source identifiers for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    Jobs,
    Nodes,
    Fairshare,
    SchedulerStats,
}

impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSource::Jobs => write!(f, "jobs"),
            DataSource::Nodes => write!(f, "nodes"),
            DataSource::Fairshare => write!(f, "fairshare"),
            DataSource::SchedulerStats => write!(f, "scheduler"),
        }
    }
}

/// Data and control events (data channel - may be dropped under load)
#[derive(Debug)]
pub enum DataEvent {
    /// Animation tick for spinners/clocks (200ms, only if visible)
    AnimationTick,

    /// Jobs data updated
    JobsUpdated(Vec<crate::tui::app::TuiJobInfo>),

    /// Nodes data updated
    NodesUpdated(Vec<crate::models::NodeInfo>),

    /// Fairshare data updated (Phase 4)
    FairshareUpdated(Vec<crate::models::SshareEntry>),

    /// Scheduler stats updated (Phase 4)
    SchedulerStatsUpdated(crate::models::SchedulerStats),

    /// Fetch error from a data source
    FetchError { source: DataSource, error: String },

    /// Job cancellation completed (success or failure)
    JobCancelResult { success: bool, message: String },
}

/// Result of processing an event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Continue running, UI needs redraw
    Continue,
    /// Continue running, no UI change needed
    Unchanged,
    /// Quit the application
    Quit,
}

/// Key action mappings for the TUI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    // Navigation
    MoveUp,
    MoveDown,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,

    // View switching
    SwitchToJobs,
    SwitchToNodes,
    SwitchToPartitions,
    SwitchToPersonal,
    SwitchToProblems,
    NextView,

    // Actions
    Select,
    Cancel,
    Refresh,
    ToggleAllJobs,
    OpenFilter,
    QuickSearch,
    OpenSort,
    YankJobId,
    CycleAccount,
    ToggleGroupByAccount,
    ToggleViewMode,
    ExportData,    // 'e' - Export to JSON (default)
    ExportDataCsv, // 'E' - Export to CSV

    // UI
    ShowHelp,
    Escape,
    Quit,

    // Filter mode specific
    FilterClear,
    FilterBackspace,
    FilterChar(char),

    // Mouse actions
    MouseClick { row: u16, column: u16 },
    MouseScrollUp,
    MouseScrollDown,

    // Unknown/unhandled
    Unknown,
}

impl KeyAction {
    /// Map a mouse event to an action
    pub fn from_mouse_event(event: MouseEvent) -> Self {
        use crossterm::event::{MouseButton, MouseEventKind};

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => KeyAction::MouseClick {
                row: event.row,
                column: event.column,
            },
            MouseEventKind::ScrollUp => KeyAction::MouseScrollUp,
            MouseEventKind::ScrollDown => KeyAction::MouseScrollDown,
            _ => KeyAction::Unknown,
        }
    }

    /// Map a key event to an action based on current mode
    pub fn from_key_event(event: KeyEvent, in_filter_mode: bool) -> Self {
        let KeyEvent {
            code, modifiers, ..
        } = event;

        // Filter mode has different mappings
        if in_filter_mode {
            return match code {
                KeyCode::Esc => KeyAction::Escape,
                KeyCode::Enter => KeyAction::Select,
                KeyCode::Backspace => KeyAction::FilterBackspace,
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    KeyAction::FilterClear
                }
                KeyCode::Char(c) => KeyAction::FilterChar(c),
                _ => KeyAction::Unknown,
            };
        }

        // Normal mode mappings
        match code {
            // Quit
            KeyCode::Char('q') => KeyAction::Quit,

            // Ctrl+ combinations must come before bare character matches
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => KeyAction::PageDown,
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => KeyAction::PageUp,
            KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::ToggleGroupByAccount
            }

            // Navigation
            KeyCode::Char('j') | KeyCode::Down => KeyAction::MoveDown,
            KeyCode::Char('k') | KeyCode::Up => KeyAction::MoveUp,
            KeyCode::Char('g') | KeyCode::Home => KeyAction::MoveToTop,
            KeyCode::Char('G') | KeyCode::End => KeyAction::MoveToBottom,
            KeyCode::PageDown => KeyAction::PageDown,
            KeyCode::PageUp => KeyAction::PageUp,

            // View switching
            KeyCode::Char('1') => KeyAction::SwitchToJobs,
            KeyCode::Char('2') => KeyAction::SwitchToNodes,
            KeyCode::Char('3') => KeyAction::SwitchToPartitions,
            KeyCode::Char('4') => KeyAction::SwitchToPersonal,
            KeyCode::Char('5') => KeyAction::SwitchToProblems,
            KeyCode::Tab => KeyAction::NextView,

            // Actions
            KeyCode::Enter => KeyAction::Select,
            KeyCode::Char('c') => KeyAction::Cancel,
            KeyCode::Char('r') => KeyAction::Refresh,
            KeyCode::Char('a') => KeyAction::ToggleAllJobs,
            KeyCode::Char('f') => KeyAction::OpenFilter,
            KeyCode::Char('/') => KeyAction::QuickSearch,
            KeyCode::Char('s') => KeyAction::OpenSort,
            KeyCode::Char('y') => KeyAction::YankJobId,
            KeyCode::Char('A') => KeyAction::CycleAccount,
            KeyCode::Char('v') => KeyAction::ToggleViewMode,
            KeyCode::Char('e') => KeyAction::ExportData,
            KeyCode::Char('E') => KeyAction::ExportDataCsv,

            // Help
            KeyCode::Char('?') | KeyCode::F(1) => KeyAction::ShowHelp,
            KeyCode::Esc => KeyAction::Escape,

            _ => KeyAction::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_action_quit() {
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(KeyAction::from_key_event(event, false), KeyAction::Quit);
    }

    #[test]
    fn test_key_action_navigation() {
        let event = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(KeyAction::from_key_event(event, false), KeyAction::MoveDown);

        let event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(KeyAction::from_key_event(event, false), KeyAction::MoveUp);
    }

    #[test]
    fn test_filter_mode_ctrl_u() {
        // In filter mode, Ctrl+U clears input
        let event = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(
            KeyAction::from_key_event(event, true),
            KeyAction::FilterClear
        );

        // In normal mode, Ctrl+U is page up
        assert_eq!(KeyAction::from_key_event(event, false), KeyAction::PageUp);
    }
}
