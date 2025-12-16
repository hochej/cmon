//! Shared formatting utilities used by both CLI and TUI
//!
//! This module consolidates duplicated formatting functions from across the codebase
//! to provide a single source of truth for string truncation, duration formatting,
//! and byte/memory size formatting.

/// Memory size constants (in bytes)
pub mod size {
    pub const KB: u64 = 1024;
    pub const MB: u64 = KB * 1024;
    pub const GB: u64 = MB * 1024;
    pub const TB: u64 = GB * 1024;
}

/// Layout constants used across CLI and TUI
pub mod layout {
    pub const BOX_WIDTH: usize = 78;
    pub const PATH_TRUNCATE_LEN: usize = 50;
    pub const BAR_LENGTH: usize = 20;
}

/// Efficiency/utilization thresholds for color coding
pub mod thresholds {
    pub const EFFICIENCY_LOW: f64 = 30.0;
    pub const EFFICIENCY_HIGH: f64 = 70.0;
    pub const UTILIZATION_LOW: f64 = 50.0;
    pub const UTILIZATION_HIGH: f64 = 80.0;
}

/// Truncate a string to a maximum length, adding "..." at the end if truncated.
///
/// # Examples
/// ```
/// use cmon::formatting::truncate_string;
/// assert_eq!(truncate_string("hello", 10), "hello");
/// assert_eq!(truncate_string("hello world", 8), "hello...");
/// assert_eq!(truncate_string("ab", 2), "ab");
/// ```
#[must_use]
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        // Edge case: if max_len is very small, just truncate without ellipsis
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Truncate a path, keeping the end visible (opposite of truncate_string).
///
/// Useful for file paths where the filename is more important than the directory prefix.
///
/// # Examples
/// ```
/// use cmon::formatting::truncate_path;
/// assert_eq!(truncate_path("/home/user/file.txt", 30), "/home/user/file.txt");
/// assert_eq!(truncate_path("/very/long/path/to/file.txt", 15), ".../to/file.txt");
/// ```
#[must_use]
pub fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else if max_len <= 3 {
        path[path.len().saturating_sub(max_len)..].to_string()
    } else {
        format!("...{}", &path[path.len().saturating_sub(max_len - 3)..])
    }
}

/// Format duration as HH:MM:SS or D-HH:MM:SS (timestamp style).
///
/// Used primarily in TUI displays where compact, fixed-width output is preferred.
///
/// # Examples
/// ```
/// use cmon::formatting::format_duration_hms;
/// assert_eq!(format_duration_hms(3661), "01:01:01");
/// assert_eq!(format_duration_hms(90061), "1-01:01:01");
/// ```
#[must_use]
pub fn format_duration_hms(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours >= 24 {
        let days = hours / 24;
        let remaining_hours = hours % 24;
        format!("{}-{:02}:{:02}:{:02}", days, remaining_hours, minutes, secs)
    } else {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }
}

/// Format duration in verbose human-readable style (e.g., "2d 3h", "5h 30m").
///
/// Shows at most 2 time units for readability. Used in CLI displays.
///
/// # Examples
/// ```
/// use cmon::formatting::format_duration_human;
/// assert_eq!(format_duration_human(0), "0s");
/// assert_eq!(format_duration_human(45), "45s");
/// assert_eq!(format_duration_human(3600), "1h");
/// assert_eq!(format_duration_human(3660), "1h 1m");
/// assert_eq!(format_duration_human(90000), "1d 1h");
/// ```
#[must_use]
pub fn format_duration_human(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}h", hours)
        }
    } else if minutes > 0 {
        if secs > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}m", minutes)
        }
    } else {
        format!("{}s", secs)
    }
}

/// Format duration from minutes to verbose human-readable style.
///
/// Convenience wrapper around [`format_duration_human`].
#[must_use]
pub fn format_duration_human_minutes(minutes: u64) -> String {
    format_duration_human(minutes * 60)
}

/// Format megabytes to human-readable size (input is in MB).
///
/// # Examples
/// ```
/// use cmon::formatting::format_bytes_mb;
/// assert_eq!(format_bytes_mb(512), "512M");
/// assert_eq!(format_bytes_mb(1024), "1.0G");
/// assert_eq!(format_bytes_mb(1536), "1.5G");
/// assert_eq!(format_bytes_mb(1048576), "1.0T");
/// ```
#[must_use]
pub fn format_bytes_mb(mb: u64) -> String {
    const GB_IN_MB: u64 = 1024;
    const TB_IN_MB: u64 = 1024 * 1024;

    if mb >= TB_IN_MB {
        format!("{:.1}T", mb as f64 / TB_IN_MB as f64)
    } else if mb >= GB_IN_MB {
        format!("{:.1}G", mb as f64 / GB_IN_MB as f64)
    } else {
        format!("{}M", mb)
    }
}

/// Format raw bytes to human-readable size.
///
/// Handles sizes from bytes to terabytes with appropriate precision.
///
/// # Examples
/// ```
/// use cmon::formatting::format_bytes;
/// assert_eq!(format_bytes(512), "512B");
/// assert_eq!(format_bytes(1536), "1.5K");
/// assert_eq!(format_bytes(1073741824), "1.0G");
/// ```
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    use size::{GB, KB, MB, TB};

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("abc", 3), "abc");
        assert_eq!(truncate_string("abcd", 3), "abc"); // edge case: max_len <= 3
        assert_eq!(truncate_string("abcdefgh", 6), "abc...");
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(truncate_path("/home/user/file.txt", 30), "/home/user/file.txt");
        assert_eq!(truncate_path("/very/long/path/file.txt", 12), ".../file.txt");
        assert_eq!(truncate_path("/a/b", 3), "a/b"); // edge case: max_len <= 3 shows last max_len chars
        assert_eq!(truncate_path("/abc/def", 7), ".../def"); // shows "..." + last 4 chars
    }

    #[test]
    fn test_format_duration_hms() {
        assert_eq!(format_duration_hms(0), "00:00:00");
        assert_eq!(format_duration_hms(61), "00:01:01");
        assert_eq!(format_duration_hms(3661), "01:01:01");
        assert_eq!(format_duration_hms(86400), "1-00:00:00");
        assert_eq!(format_duration_hms(90061), "1-01:01:01");
    }

    #[test]
    fn test_format_duration_human() {
        assert_eq!(format_duration_human(0), "0s");
        assert_eq!(format_duration_human(45), "45s");
        assert_eq!(format_duration_human(65), "1m 5s");
        assert_eq!(format_duration_human(3600), "1h");
        assert_eq!(format_duration_human(3660), "1h 1m");
        assert_eq!(format_duration_human(86400), "1d");
        assert_eq!(format_duration_human(90000), "1d 1h");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes_mb(512), "512M");
        assert_eq!(format_bytes_mb(1024), "1.0G");
        assert_eq!(format_bytes_mb(1536), "1.5G");
        assert_eq!(format_bytes_mb(2048), "2.0G");
        assert_eq!(format_bytes_mb(1048576), "1.0T");
        assert_eq!(format_bytes_mb(1572864), "1.5T");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(1536), "1.5K");
        assert_eq!(format_bytes(1048576), "1.0M");
        assert_eq!(format_bytes(1073741824), "1.0G");
        assert_eq!(format_bytes(1099511627776), "1.0T");
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(size::KB, 1024);
        assert_eq!(size::MB, 1024 * 1024);
        assert_eq!(size::GB, 1024 * 1024 * 1024);
        assert_eq!(size::TB, 1024u64 * 1024 * 1024 * 1024);
    }
}
