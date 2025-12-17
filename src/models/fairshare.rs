//! Fairshare and sshare types.
//!
//! This module contains types for handling Slurm fairshare data from the sshare command,
//! including tree building for hierarchical display.

use serde::{Deserialize, Serialize};

use super::time::{FloatValue, TimeValue};

/// Slurm API response wrapper for sshare
#[derive(Debug, Deserialize, Serialize)]
pub struct SshareResponse {
    #[serde(default)]
    pub shares: SshareWrapper,

    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareWrapper {
    #[serde(default)]
    pub shares: Vec<SshareEntry>,
}

/// Individual fairshare entry from sshare
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SshareEntry {
    #[serde(default)]
    pub id: u32,

    #[serde(default)]
    pub cluster: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub parent: String,

    #[serde(default)]
    pub partition: String,

    /// Normalized shares (floating-point)
    #[serde(default)]
    pub shares_normalized: FloatValue,

    /// Raw shares (can be float in Slurm 24+)
    #[serde(default)]
    pub shares: FloatValue,

    #[serde(default)]
    pub tres: SshareTres,

    /// Raw usage value (integer)
    #[serde(default)]
    pub usage: u64,

    #[serde(default)]
    pub fairshare: SshareFairshare,

    /// Effective usage (floating-point)
    #[serde(default)]
    pub effective_usage: FloatValue,

    /// Normalized usage (floating-point)
    #[serde(default)]
    pub usage_normalized: FloatValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareTres {
    #[serde(default)]
    pub run_seconds: Vec<SshareTresItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareTresItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: TimeValue,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SshareFairshare {
    /// Fairshare factor (floating-point value struct)
    #[serde(default)]
    pub factor: FloatValue,
    /// Fairshare level (floating-point value struct)
    #[serde(default)]
    pub level: FloatValue,
}

impl SshareEntry {
    /// Get shares as a fraction (0.0 to 1.0)
    #[must_use]
    pub fn shares_fraction(&self) -> f64 {
        if self.shares_normalized.set {
            self.shares_normalized.number
        } else {
            0.0
        }
    }

    /// Get fairshare factor (0.0 to 1.0, higher is better)
    #[must_use]
    pub fn fairshare_factor(&self) -> f64 {
        if self.fairshare.factor.set {
            self.fairshare.factor.number
        } else {
            0.0
        }
    }

    /// Check if this is a user entry (has a parent that's an account)
    #[must_use]
    pub fn is_user(&self) -> bool {
        !self.parent.is_empty() && self.parent != "root"
    }

    /// Get CPU hours from TRES run_seconds
    #[must_use]
    pub fn cpu_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "cpu" {
                if let Some(val) = item.value.value() {
                    return val as f64 / 3600.0;
                }
            }
        }
        0.0
    }

    /// Get GPU hours from TRES run_seconds
    #[must_use]
    pub fn gpu_hours(&self) -> f64 {
        self.tres
            .run_seconds
            .iter()
            .filter(|item| item.name.starts_with("gres/gpu"))
            .filter_map(|item| item.value.value())
            .map(|val| val as f64 / 3600.0)
            .sum()
    }

    /// Get memory GB-hours from TRES run_seconds
    #[allow(dead_code)]
    #[must_use]
    pub fn mem_gb_hours(&self) -> f64 {
        for item in &self.tres.run_seconds {
            if item.name == "mem" {
                if let Some(val) = item.value.value() {
                    // Memory is in MB-seconds, convert to GB-hours
                    return val as f64 / (1024.0 * 3600.0);
                }
            }
        }
        0.0
    }
}

/// Fairshare tree node for hierarchical display
#[derive(Debug, Clone)]
pub struct FairshareNode {
    pub name: String,
    #[allow(dead_code)]
    pub parent: String,
    pub depth: usize,
    pub is_user: bool,
    pub is_current_user: bool,
    pub shares_percent: f64,
    pub fairshare_factor: f64,
    pub cpu_hours: f64,
    pub gpu_hours: f64,
    pub children: Vec<FairshareNode>,
}

impl FairshareNode {
    /// Build a tree from flat sshare entries
    pub fn build_tree(entries: &[SshareEntry], current_user: &str) -> Vec<FairshareNode> {
        let mut root_nodes = Vec::new();

        // Find all root-level entries (parent is "root" or empty)
        for entry in entries {
            if entry.parent == "root" || entry.parent.is_empty() {
                let node = Self::build_node(entry, entries, 0, current_user);
                root_nodes.push(node);
            }
        }

        root_nodes
    }

    fn build_node(
        entry: &SshareEntry,
        all_entries: &[SshareEntry],
        depth: usize,
        current_user: &str,
    ) -> Self {
        let mut node = FairshareNode {
            name: entry.name.clone(),
            parent: entry.parent.clone(),
            depth,
            is_user: entry.is_user(),
            is_current_user: entry.name == current_user,
            shares_percent: entry.shares_fraction() * 100.0,
            fairshare_factor: entry.fairshare_factor(),
            cpu_hours: entry.cpu_hours(),
            gpu_hours: entry.gpu_hours(),
            children: Vec::new(),
        };

        // Find children (entries whose parent matches this name)
        for child_entry in all_entries {
            if child_entry.parent == entry.name {
                let child_node =
                    Self::build_node(child_entry, all_entries, depth + 1, current_user);
                node.children.push(child_node);
            }
        }

        node
    }

    /// Flatten the tree for display with proper indentation
    #[must_use]
    pub fn flatten(&self) -> Vec<FlatFairshareRow> {
        let mut rows = Vec::new();
        self.flatten_recursive(&mut rows);
        rows
    }

    fn flatten_recursive(&self, rows: &mut Vec<FlatFairshareRow>) {
        rows.push(FlatFairshareRow {
            name: self.name.clone(),
            depth: self.depth,
            is_user: self.is_user,
            is_current_user: self.is_current_user,
            shares_percent: self.shares_percent,
            fairshare_factor: self.fairshare_factor,
            cpu_hours: self.cpu_hours,
            gpu_hours: self.gpu_hours,
            has_children: !self.children.is_empty(),
        });

        for child in &self.children {
            child.flatten_recursive(rows);
        }
    }
}

/// Flattened fairshare row for table display
#[derive(Debug, Clone)]
pub struct FlatFairshareRow {
    pub name: String,
    pub depth: usize,
    pub is_user: bool,
    pub is_current_user: bool,
    pub shares_percent: f64,
    pub fairshare_factor: f64,
    pub cpu_hours: f64,
    pub gpu_hours: f64,
    pub has_children: bool,
}

impl FlatFairshareRow {
    /// Get display name with tree indentation
    #[must_use]
    pub fn display_name(&self) -> String {
        let indent = "  ".repeat(self.depth);
        let prefix = if self.has_children { "+" } else { "-" };
        format!("{}{} {}", indent, prefix, self.name)
    }
}
