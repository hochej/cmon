//! Export functionality for the TUI
//!
//! This module provides:
//! - `Exportable` trait for types that can be exported to JSON/CSV
//! - `export_items()` generic function for format-agnostic export
//! - `escape_csv()` helper for proper CSV field escaping
//!
//! The actual export methods remain on `App` since they need access to
//! feedback state for error reporting.

use super::state::ExportFormat;

/// Trait for types that can be exported to JSON and CSV formats.
///
/// Implementors define how their data maps to export formats:
/// - `csv_headers()`: Column names for CSV export
/// - `to_csv_row()`: Row values (raw strings, escaping handled by `export_items`)
/// - `to_json_value()`: Custom JSON representation for export
///
/// This design allows exporting a subset of fields with custom formatting,
/// rather than requiring `Serialize` on the full type.
pub trait Exportable {
    /// CSV column headers for this type
    fn csv_headers() -> &'static [&'static str];

    /// Convert this item to a CSV row.
    /// Values should be raw strings; escaping is handled by `export_items()`.
    fn to_csv_row(&self) -> Vec<String>;

    /// Convert this item to a JSON value for export.
    /// This allows custom field selection and formatting.
    fn to_json_value(&self) -> serde_json::Value;
}

/// Blanket implementation for references - allows exporting Vec<&T>
impl<T: Exportable> Exportable for &T {
    fn csv_headers() -> &'static [&'static str] {
        T::csv_headers()
    }

    fn to_csv_row(&self) -> Vec<String> {
        (*self).to_csv_row()
    }

    fn to_json_value(&self) -> serde_json::Value {
        (*self).to_json_value()
    }
}

/// Export a slice of items to the specified format.
///
/// For CSV: joins headers with commas, escapes each field value, outputs rows.
/// For JSON: collects JSON values into an array and pretty-prints.
///
/// Returns the formatted string ready for writing to a file.
pub fn export_items<T: Exportable>(items: &[T], format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => {
            let values: Vec<_> = items.iter().map(Exportable::to_json_value).collect();
            serde_json::to_string_pretty(&values).unwrap_or_else(|_| "[]".to_string())
        }
        ExportFormat::Csv => {
            let mut out = T::csv_headers().join(",") + "\n";
            for item in items {
                let row: Vec<String> = item.to_csv_row().iter().map(|s| escape_csv(s)).collect();
                out.push_str(&row.join(","));
                out.push('\n');
            }
            out
        }
    }
}

/// Escape a string for CSV (handle commas, quotes, newlines)
pub fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ============================================================================
// Exportable Implementations
// ============================================================================

use super::types::{PartitionStatus, TuiJobInfo};
use crate::models::NodeInfo;

impl Exportable for TuiJobInfo {
    fn csv_headers() -> &'static [&'static str] {
        &[
            "job_id",
            "name",
            "user",
            "account",
            "partition",
            "state",
            "state_reason",
            "priority",
            "qos",
            "cpus",
            "gpus",
            "gpu_type",
            "nodes",
            "elapsed_seconds",
            "time_limit_seconds",
            "time_remaining_seconds",
            "working_directory",
        ]
    }

    fn to_csv_row(&self) -> Vec<String> {
        let time_remaining = self.time_remaining().map(|d| d.as_secs()).unwrap_or(0);
        vec![
            self.job_id.to_string(),
            self.name.clone(),
            self.user_name.clone(),
            self.account.clone(),
            self.partition.clone(),
            self.state.as_str().to_string(),
            self.state_reason.clone(),
            self.priority.to_string(),
            self.qos.clone(),
            self.cpus.to_string(),
            self.gpu_count.to_string(),
            self.gpu_type.as_deref().unwrap_or("").to_string(),
            self.nodes.clone(),
            self.elapsed_seconds.to_string(),
            self.time_limit_seconds.to_string(),
            time_remaining.to_string(),
            self.working_directory.clone(),
        ]
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "job_id": self.job_id.to_string(),
            "name": self.name,
            "user": self.user_name,
            "account": self.account,
            "partition": self.partition,
            "state": self.state.as_str(),
            "state_reason": self.state_reason,
            "priority": self.priority,
            "qos": self.qos,
            "cpus": self.cpus,
            "gpus": self.gpu_count,
            "gpu_type": self.gpu_type,
            "nodes": self.nodes,
            "elapsed_seconds": self.elapsed_seconds,
            "time_limit_seconds": self.time_limit_seconds,
            "working_directory": self.working_directory,
        })
    }
}

impl Exportable for NodeInfo {
    fn csv_headers() -> &'static [&'static str] {
        &[
            "name",
            "partition",
            "state",
            "cpus_allocated",
            "cpus_total",
            "memory_allocated_mb",
            "memory_total_mb",
            "gpus_used",
            "gpus_total",
            "gpu_type",
        ]
    }

    fn to_csv_row(&self) -> Vec<String> {
        let gpu_info = self.gpu_info();
        vec![
            self.name().to_string(),
            self.partition.name.as_deref().unwrap_or("").to_string(),
            self.primary_state().to_string(),
            self.cpus.allocated.to_string(),
            self.cpus.total.to_string(),
            self.memory.allocated.to_string(),
            self.memory.minimum.to_string(),
            gpu_info.used.to_string(),
            gpu_info.total.to_string(),
            gpu_info.gpu_type.to_string(),
        ]
    }

    fn to_json_value(&self) -> serde_json::Value {
        let gpu_info = self.gpu_info();
        serde_json::json!({
            "name": self.node_names.nodes.first().unwrap_or(&String::new()),
            "partition": self.partition.name,
            "state": self.primary_state(),
            "cpus_allocated": self.cpus.allocated,
            "cpus_total": self.cpus.total,
            "memory_allocated_mb": self.memory.allocated,
            "memory_total_mb": self.memory.minimum,
            "gpus_used": gpu_info.used,
            "gpus_total": gpu_info.total,
            "gpu_type": gpu_info.gpu_type,
        })
    }
}

impl Exportable for PartitionStatus {
    fn csv_headers() -> &'static [&'static str] {
        &[
            "name",
            "total_nodes",
            "available_nodes",
            "down_nodes",
            "total_cpus",
            "allocated_cpus",
            "cpu_utilization",
            "total_gpus",
            "allocated_gpus",
            "gpu_utilization",
            "gpu_type",
        ]
    }

    fn to_csv_row(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.total_nodes.to_string(),
            self.available_nodes.to_string(),
            self.down_nodes.to_string(),
            self.total_cpus.to_string(),
            self.allocated_cpus.to_string(),
            format!("{:.1}", self.cpu_utilization()),
            self.total_gpus.to_string(),
            self.allocated_gpus.to_string(),
            format!("{:.1}", self.gpu_utilization()),
            self.gpu_type.as_deref().unwrap_or("").to_string(),
        ]
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "total_nodes": self.total_nodes,
            "available_nodes": self.available_nodes,
            "down_nodes": self.down_nodes,
            "total_cpus": self.total_cpus,
            "allocated_cpus": self.allocated_cpus,
            "cpu_utilization": self.cpu_utilization(),
            "total_gpus": self.total_gpus,
            "allocated_gpus": self.allocated_gpus,
            "gpu_utilization": self.gpu_utilization(),
            "gpu_type": self.gpu_type,
        })
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

    // Test struct for Exportable trait
    struct TestItem {
        name: String,
        value: i32,
    }

    impl Exportable for TestItem {
        fn csv_headers() -> &'static [&'static str] {
            &["name", "value"]
        }

        fn to_csv_row(&self) -> Vec<String> {
            vec![self.name.clone(), self.value.to_string()]
        }

        fn to_json_value(&self) -> serde_json::Value {
            serde_json::json!({
                "name": self.name,
                "value": self.value,
            })
        }
    }

    #[test]
    fn test_export_items_csv() {
        let items = vec![
            TestItem { name: "foo".to_string(), value: 1 },
            TestItem { name: "bar,baz".to_string(), value: 2 },
        ];
        let csv = export_items(&items, ExportFormat::Csv);
        assert!(csv.starts_with("name,value\n"));
        assert!(csv.contains("foo,1\n"));
        assert!(csv.contains("\"bar,baz\",2\n")); // comma should be escaped
    }

    #[test]
    fn test_export_items_json() {
        let items = vec![
            TestItem { name: "foo".to_string(), value: 1 },
        ];
        let json = export_items(&items, ExportFormat::Json);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["name"], "foo");
        assert_eq!(parsed[0]["value"], 1);
    }

    #[test]
    fn test_export_items_reference() {
        // Test that blanket implementation for &T works
        let items = vec![
            TestItem { name: "a".to_string(), value: 10 },
        ];
        let refs: Vec<&TestItem> = items.iter().collect();
        let csv = export_items(&refs, ExportFormat::Csv);
        assert!(csv.contains("a,10\n"));
    }
}
