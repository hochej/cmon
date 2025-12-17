//! Configuration types for the TUI.
//!
//! This module contains configuration structures for customizing the TUI behavior,
//! including refresh intervals, display settings, and behavior options.

use serde::{Deserialize, Serialize};

/// TUI configuration
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TuiConfig {
    pub system: SystemConfig,

    pub refresh: RefreshConfig,

    pub display: DisplayConfig,

    pub behavior: BehaviorConfig,
}

/// System configuration for paths and environment
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SystemConfig {
    /// Path to directory containing Slurm binaries (sinfo, squeue, etc.)
    /// If empty or not set, auto-detected via PATH
    pub slurm_bin_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RefreshConfig {
    /// Jobs refresh interval in seconds
    pub jobs_interval: u64,

    /// Nodes refresh interval in seconds
    pub nodes_interval: u64,

    /// Fairshare refresh interval in seconds
    pub fairshare_interval: u64,

    /// Enable idle slowdown
    pub idle_slowdown: bool,

    /// Seconds before considered idle
    pub idle_threshold: u64,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        }
    }
}

/// Minimum allowed refresh interval in seconds (prevents tight polling loops)
const MIN_REFRESH_INTERVAL: u64 = 1;

/// Minimum idle threshold in seconds
const MIN_IDLE_THRESHOLD: u64 = 1;

/// Fields in RefreshConfig that require interval validation.
#[derive(Clone, Copy)]
enum RefreshField {
    JobsInterval,
    NodesInterval,
    FairshareInterval,
    IdleThreshold,
}

impl RefreshField {
    /// Returns the config path for error messages (e.g., "refresh.jobs_interval").
    const fn as_str(self) -> &'static str {
        match self {
            Self::JobsInterval => "jobs_interval",
            Self::NodesInterval => "nodes_interval",
            Self::FairshareInterval => "fairshare_interval",
            Self::IdleThreshold => "idle_threshold",
        }
    }
}

/// Validate that an interval value meets the minimum requirement.
/// In non-strict mode, corrects invalid values to the default and adds a warning.
/// In strict mode, returns an error for invalid values.
fn validate_interval(
    value: &mut u64,
    field: RefreshField,
    min: u64,
    default: u64,
    strict: bool,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if *value < min {
        let field_name = field.as_str();
        let msg = format!(
            "refresh.{field_name} must be at least {min} second(s), got {value}",
        );
        if strict {
            return Err(msg);
        }
        warnings.push(format!("{msg} - using default ({default})"));
        *value = default;
    }
    Ok(())
}

impl RefreshConfig {
    /// Validate refresh configuration values.
    /// Returns a list of warnings for invalid values that were corrected to defaults.
    /// If `strict` is true, returns Err instead of correcting values.
    pub fn validate(&mut self, strict: bool) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        let defaults = Self::default();

        validate_interval(
            &mut self.jobs_interval,
            RefreshField::JobsInterval,
            MIN_REFRESH_INTERVAL,
            defaults.jobs_interval,
            strict,
            &mut warnings,
        )?;

        validate_interval(
            &mut self.nodes_interval,
            RefreshField::NodesInterval,
            MIN_REFRESH_INTERVAL,
            defaults.nodes_interval,
            strict,
            &mut warnings,
        )?;

        validate_interval(
            &mut self.fairshare_interval,
            RefreshField::FairshareInterval,
            MIN_REFRESH_INTERVAL,
            defaults.fairshare_interval,
            strict,
            &mut warnings,
        )?;

        // idle_threshold only validated if idle_slowdown is enabled
        if self.idle_slowdown {
            validate_interval(
                &mut self.idle_threshold,
                RefreshField::IdleThreshold,
                MIN_IDLE_THRESHOLD,
                defaults.idle_threshold,
                strict,
                &mut warnings,
            )?;
        }

        Ok(warnings)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Default view on startup
    pub default_view: String,

    /// Show all jobs by default
    pub show_all_jobs: bool,

    /// Start with grouped-by-account mode
    pub show_grouped_by_account: bool,

    /// Theme name
    pub theme: String,

    /// Partition display order (empty = alphabetical)
    /// Example: ["cpu", "gpu", "fat", "vdi"]
    pub partition_order: Vec<String>,

    /// Prefix to strip from node names for display (optional)
    /// Example: "demu4x" would turn "demu4xcpu01" into "cpu01"
    pub node_prefix_strip: String,

    /// Maximum length for job names before truncation (default: 35)
    pub job_name_max_length: usize,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            default_view: "jobs".to_string(),
            show_all_jobs: false,
            show_grouped_by_account: false,
            theme: "dark".to_string(),
            partition_order: Vec::new(),
            node_prefix_strip: String::new(),
            job_name_max_length: 35,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Require confirmation before cancelling jobs
    pub confirm_cancel: bool,

    /// Enable clipboard support
    pub copy_to_clipboard: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            confirm_cancel: true,
            copy_to_clipboard: true,
        }
    }
}

impl TuiConfig {
    /// Get the user config file path, respecting XDG_CONFIG_HOME
    ///
    /// Resolution order:
    /// 1. $XDG_CONFIG_HOME/cmon/config.toml (if XDG_CONFIG_HOME is set)
    /// 2. $HOME/.config/cmon/config.toml (if HOME is set)
    /// 3. dirs::config_dir()/cmon/config.toml (fallback using dirs crate)
    /// 4. None if no config directory can be determined
    #[must_use]
    pub fn user_config_path() -> Option<std::path::PathBuf> {
        // Prefer XDG_CONFIG_HOME if set
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME")
            && !xdg_config.is_empty()
        {
            return Some(std::path::PathBuf::from(xdg_config).join("cmon/config.toml"));
        }

        // Fall back to ~/.config
        if let Some(home) = std::env::var_os("HOME") {
            return Some(std::path::PathBuf::from(home).join(".config/cmon/config.toml"));
        }

        // Last resort: use dirs crate
        dirs::config_dir().map(|dir| dir.join("cmon/config.toml"))
    }

    /// Load configuration from files and environment.
    /// Returns the config and any warnings encountered during loading.
    pub fn load() -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();
        let strict = Self::is_strict_mode();

        // Try to load from /etc/cmon/config.toml
        Self::load_config_file(&mut config, "/etc/cmon/config.toml", &mut warnings);

        // Try to load from user config path (respects XDG_CONFIG_HOME)
        if let Some(user_path) = Self::user_config_path() {
            Self::load_config_file(&mut config, &user_path.to_string_lossy(), &mut warnings);
        }

        // Apply environment overrides
        config.apply_env_overrides();

        // Validate refresh intervals
        match config.refresh.validate(strict) {
            Ok(validation_warnings) => warnings.extend(validation_warnings),
            Err(err) => {
                eprintln!("Error: {}", err);
                eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                std::process::exit(1);
            }
        }

        (config, warnings)
    }

    /// Check if strict config mode is enabled via CMON_STRICT_CONFIG
    fn is_strict_mode() -> bool {
        std::env::var("CMON_STRICT_CONFIG")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// Load a config file, collecting warnings on parse errors but not on missing files.
    /// If CMON_STRICT_CONFIG=1 is set, parse errors cause immediate exit.
    fn load_config_file(config: &mut Self, path: &str, warnings: &mut Vec<String>) {
        let strict = Self::is_strict_mode();

        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<TuiConfig>(&content) {
                Ok(parsed) => config.merge(parsed),
                Err(e) => {
                    if strict {
                        eprintln!("Error: Failed to parse config file '{}': {}", path, e);
                        eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                        std::process::exit(1);
                    } else {
                        // Collect warning for display in TUI status bar
                        warnings.push(format!("Config parse error in '{}': {}", path, e));
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File not found is expected and not an error
            }
            Err(e) => {
                // Other errors (permissions, etc.) should be reported
                if strict {
                    eprintln!("Error: Could not read config file '{}': {}", path, e);
                    eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
                    std::process::exit(1);
                } else {
                    warnings.push(format!("Could not read config '{}': {}", path, e));
                }
            }
        }
    }

    fn merge(&mut self, other: TuiConfig) {
        // Prefer other's slurm_bin_path if set, otherwise keep current
        self.system.slurm_bin_path = other
            .system
            .slurm_bin_path
            .or(self.system.slurm_bin_path.take());
        self.refresh = other.refresh;
        self.display = other.display;
        self.behavior = other.behavior;
    }

    fn apply_env_overrides(&mut self) {
        let strict = Self::is_strict_mode();

        // System overrides
        if let Ok(val) = std::env::var("CMON_SLURM_PATH")
            && !val.is_empty()
        {
            let path = std::path::PathBuf::from(&val);
            if path.is_dir() {
                self.system.slurm_bin_path = Some(path);
            } else {
                Self::report_env_error(
                    strict,
                    "CMON_SLURM_PATH",
                    &val,
                    "not a valid directory",
                );
            }
        }

        if let Ok(val) = std::env::var("CMON_REFRESH_JOBS") {
            match val.parse::<u64>() {
                Ok(secs) if secs >= MIN_REFRESH_INTERVAL => {
                    self.refresh.jobs_interval = secs;
                }
                Ok(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_JOBS",
                    &val,
                    &format!("must be at least {} second(s)", MIN_REFRESH_INTERVAL),
                ),
                Err(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_JOBS",
                    &val,
                    "expected a positive integer (seconds)",
                ),
            }
        }

        if let Ok(val) = std::env::var("CMON_REFRESH_NODES") {
            match val.parse::<u64>() {
                Ok(secs) if secs >= MIN_REFRESH_INTERVAL => {
                    self.refresh.nodes_interval = secs;
                }
                Ok(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_NODES",
                    &val,
                    &format!("must be at least {} second(s)", MIN_REFRESH_INTERVAL),
                ),
                Err(_) => Self::report_env_error(
                    strict,
                    "CMON_REFRESH_NODES",
                    &val,
                    "expected a positive integer (seconds)",
                ),
            }
        }

        if let Ok(val) = std::env::var("CMON_DEFAULT_VIEW") {
            self.display.default_view = val;
        }
        if let Ok(val) = std::env::var("CMON_THEME") {
            self.display.theme = val;
        }
        if std::env::var("CMON_NO_CLIPBOARD").is_ok() {
            self.behavior.copy_to_clipboard = false;
        }
    }

    /// Report an environment variable error, exiting if strict mode is enabled
    fn report_env_error(strict: bool, var_name: &str, value: &str, reason: &str) {
        if strict {
            eprintln!("Error: Invalid value '{}' for {}: {}", value, var_name, reason);
            eprintln!("(CMON_STRICT_CONFIG is set - config errors are fatal)");
            std::process::exit(1);
        } else {
            eprintln!(
                "Warning: Invalid value '{}' for {}, {} - using default",
                value, var_name, reason
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_config_validate_valid_values() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "No warnings expected for valid config");
    }

    #[test]
    fn test_refresh_config_validate_minimum_values() {
        // Minimum valid values (1 second each)
        let mut config = RefreshConfig {
            jobs_interval: 1,
            nodes_interval: 1,
            fairshare_interval: 1,
            idle_slowdown: true,
            idle_threshold: 1,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "No warnings expected for minimum valid values");
    }

    #[test]
    fn test_refresh_config_validate_zero_jobs_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("jobs_interval"));
        assert!(warnings[0].contains("at least 1"));
        // Value should be corrected to default
        assert_eq!(config.jobs_interval, RefreshConfig::default().jobs_interval);
    }

    #[test]
    fn test_refresh_config_validate_zero_nodes_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 0,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("nodes_interval"));
        // Value should be corrected to default
        assert_eq!(config.nodes_interval, RefreshConfig::default().nodes_interval);
    }

    #[test]
    fn test_refresh_config_validate_zero_fairshare_interval() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 0,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("fairshare_interval"));
        // Value should be corrected to default
        assert_eq!(
            config.fairshare_interval,
            RefreshConfig::default().fairshare_interval
        );
    }

    #[test]
    fn test_refresh_config_validate_zero_idle_threshold_with_slowdown_enabled() {
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("idle_threshold"));
        // Value should be corrected to default
        assert_eq!(
            config.idle_threshold,
            RefreshConfig::default().idle_threshold
        );
    }

    #[test]
    fn test_refresh_config_validate_zero_idle_threshold_with_slowdown_disabled() {
        // If idle_slowdown is disabled, idle_threshold doesn't matter
        let mut config = RefreshConfig {
            jobs_interval: 5,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: false,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_empty(),
            "No warnings when idle_slowdown is disabled"
        );
    }

    #[test]
    fn test_refresh_config_validate_multiple_zero_values() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 0,
            fairshare_interval: 0,
            idle_slowdown: true,
            idle_threshold: 0,
        };

        let result = config.validate(false);
        assert!(result.is_ok());
        let warnings = result.unwrap();
        // Should have 4 warnings: jobs, nodes, fairshare, idle_threshold
        assert_eq!(warnings.len(), 4);
        // All values should be corrected to defaults
        let defaults = RefreshConfig::default();
        assert_eq!(config.jobs_interval, defaults.jobs_interval);
        assert_eq!(config.nodes_interval, defaults.nodes_interval);
        assert_eq!(config.fairshare_interval, defaults.fairshare_interval);
        assert_eq!(config.idle_threshold, defaults.idle_threshold);
    }

    #[test]
    fn test_refresh_config_validate_strict_mode_error() {
        let mut config = RefreshConfig {
            jobs_interval: 0,
            nodes_interval: 10,
            fairshare_interval: 60,
            idle_slowdown: true,
            idle_threshold: 30,
        };

        let result = config.validate(true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("jobs_interval"));
        assert!(err.contains("at least 1"));
    }
}
