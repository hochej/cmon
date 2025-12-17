//! Export functionality for the TUI
//!
//! This module provides utilities for exporting data to various formats.
//! The actual export methods remain on `App` since they need access to
//! feedback state for error reporting.

/// Escape a string for CSV (handle commas, quotes, newlines)
pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_csv_simple() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv(""), "");
    }

    #[test]
    fn test_escape_csv_comma() {
        assert_eq!(escape_csv("hello,world"), "\"hello,world\"");
    }

    #[test]
    fn test_escape_csv_quotes() {
        assert_eq!(escape_csv("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_newline() {
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }
}
