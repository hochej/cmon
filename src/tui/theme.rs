//! Theme definitions for the TUI
//!
//! This module provides colorblind-safe themes for both dark and light terminals.
//! The default is "dark" but users can configure "light" via config file or env var.

use ratatui::style::Color;

use crate::models::JobState;

/// Available theme names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Dark,
    Light,
}

impl ThemeName {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => ThemeName::Light,
            _ => ThemeName::Dark,
        }
    }
}

/// Color theme for the TUI
#[derive(Debug, Clone)]
pub struct Theme {
    #[allow(dead_code)]
    pub name: ThemeName,

    // Base colors
    #[allow(dead_code)]
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub border_focused: Color,

    // Job state colors (colorblind-safe)
    pub running: Color,
    pub pending: Color,
    pub completed: Color,
    pub failed: Color,
    pub cancelled: Color,
    pub timeout: Color,

    // Node state colors
    pub idle: Color,
    pub mixed: Color,
    pub draining: Color,

    // UI elements
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub stale_indicator: Color,

    // Progress bars
    pub progress_full: Color,
    #[allow(dead_code)]
    pub progress_empty: Color,
    pub progress_warn: Color,
    pub progress_crit: Color,

    // Account highlighting
    pub account_highlight: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Create a dark theme (default)
    pub fn dark() -> Self {
        Self {
            name: ThemeName::Dark,

            bg: Color::Reset,
            fg: Color::White,
            border: Color::DarkGray,
            border_focused: Color::Cyan,

            // Colorblind-safe palette for dark backgrounds
            // Using blue for positive, orange for warning, distinct brightness levels
            running: Color::Rgb(0, 200, 0),      // Bright green
            pending: Color::Rgb(255, 180, 0),    // Orange (not yellow - better visibility)
            completed: Color::Rgb(80, 160, 255), // Light blue
            failed: Color::Rgb(255, 80, 80),     // Bright red
            cancelled: Color::DarkGray,
            timeout: Color::Magenta,

            // Node state colors (distinct from job colors for clarity)
            idle: Color::Rgb(100, 180, 100),     // Muted green
            mixed: Color::Rgb(255, 200, 100),    // Soft amber
            draining: Color::Rgb(180, 100, 180), // Muted purple

            selected_bg: Color::Rgb(60, 60, 80),
            selected_fg: Color::White,
            header_bg: Color::Rgb(40, 80, 120),
            header_fg: Color::White,
            stale_indicator: Color::Rgb(255, 100, 100),

            progress_full: Color::Rgb(0, 200, 0),
            progress_empty: Color::DarkGray,
            progress_warn: Color::Rgb(255, 180, 0),
            progress_crit: Color::Rgb(255, 80, 80),

            account_highlight: Color::Cyan,
        }
    }

    /// Create a light theme
    /// Uses darker, more saturated colors for visibility on light backgrounds
    pub fn light() -> Self {
        Self {
            name: ThemeName::Light,

            bg: Color::Reset,
            fg: Color::Black,
            border: Color::Rgb(120, 120, 120),
            border_focused: Color::Rgb(0, 100, 180),

            // Colorblind-safe palette for light backgrounds
            // Using darker, more saturated versions that contrast well
            running: Color::Rgb(0, 140, 0),       // Dark green
            pending: Color::Rgb(200, 120, 0),     // Dark orange
            completed: Color::Rgb(0, 80, 180),    // Dark blue
            failed: Color::Rgb(200, 0, 0),        // Dark red
            cancelled: Color::Rgb(100, 100, 100), // Medium gray
            timeout: Color::Rgb(160, 0, 160),     // Dark magenta

            // Node state colors for light theme
            idle: Color::Rgb(60, 120, 60),      // Dark muted green
            mixed: Color::Rgb(180, 140, 60),    // Dark amber
            draining: Color::Rgb(140, 60, 140), // Dark muted purple

            selected_bg: Color::Rgb(200, 220, 255),
            selected_fg: Color::Black,
            header_bg: Color::Rgb(180, 200, 230),
            header_fg: Color::Black,
            stale_indicator: Color::Rgb(200, 0, 0),

            progress_full: Color::Rgb(0, 140, 0),
            progress_empty: Color::Rgb(180, 180, 180),
            progress_warn: Color::Rgb(200, 120, 0),
            progress_crit: Color::Rgb(200, 0, 0),

            account_highlight: Color::Rgb(0, 100, 180),
        }
    }

    /// Create theme from name string
    pub fn from_name(name: &str) -> Self {
        match ThemeName::from_str(name) {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
        }
    }

    /// Get color for a job state
    pub fn job_state_color(&self, state: JobState) -> Color {
        match state {
            JobState::Running | JobState::Completing => self.running,
            JobState::Pending | JobState::Suspended => self.pending,
            JobState::Completed => self.completed,
            JobState::Failed
            | JobState::OutOfMemory
            | JobState::NodeFail
            | JobState::BootFail => self.failed,
            JobState::Cancelled | JobState::Preempted => self.cancelled,
            JobState::Timeout | JobState::Deadline => self.timeout,
            JobState::Unknown => self.fg,
        }
    }

    /// Get color for node state string
    pub fn node_state_color(&self, state: &str) -> Color {
        match state {
            "IDLE" => self.running,
            "MIXED" => self.pending,
            "ALLOCATED" => self.completed,
            "DOWN" | "FAIL" | "FAILING" => self.failed,
            "DRAINING" | "DRAINED" => self.timeout,
            _ => self.fg,
        }
    }

    /// Get appropriate progress bar color based on utilization percentage
    pub fn progress_color(&self, percent: f64) -> Color {
        if percent >= 95.0 {
            self.progress_crit
        } else if percent >= 80.0 {
            self.progress_warn
        } else {
            self.progress_full
        }
    }

    /// Get fairshare factor color (green=good priority, red=poor priority)
    pub fn fairshare_color(&self, factor: f64) -> Color {
        if factor >= 0.5 {
            self.running
        } else if factor >= 0.3 {
            self.pending
        } else {
            self.failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_from_name() {
        let dark = Theme::from_name("dark");
        assert_eq!(dark.name, ThemeName::Dark);

        let light = Theme::from_name("light");
        assert_eq!(light.name, ThemeName::Light);

        // Unknown defaults to dark
        let unknown = Theme::from_name("unknown");
        assert_eq!(unknown.name, ThemeName::Dark);
    }

    #[test]
    fn test_job_state_colors() {
        let theme = Theme::dark();
        assert_eq!(theme.job_state_color(JobState::Running), theme.running);
        assert_eq!(theme.job_state_color(JobState::Pending), theme.pending);
        assert_eq!(theme.job_state_color(JobState::Failed), theme.failed);
    }

    #[test]
    fn test_progress_color() {
        let theme = Theme::dark();
        assert_eq!(theme.progress_color(50.0), theme.progress_full);
        assert_eq!(theme.progress_color(85.0), theme.progress_warn);
        assert_eq!(theme.progress_color(98.0), theme.progress_crit);
    }
}
