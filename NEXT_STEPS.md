# cmon Codebase Improvement Plan (v2)

Consolidated from four independent code reviews.

---

## Architecture Validation

### Single Source of Truth for Slurm Parsing — PASS

Both CLI and TUI correctly share the same parsing pipeline:

```
slurm.rs (fetch + parse) → models.rs (data structs) → display.rs / tui/
```

No action required. This is well-implemented.

---

## P0 — Critical (Blocks Portability)

### [DONE] 1. Remove Hardcoded Node Prefixes

The tool is unusable on any cluster not named `demu4x*`.

| File | Line | Hardcoded Value |
|------|------|-----------------|
| `display.rs` | 458-464 | `"demu4xcpu"`, `"demu4xfat"`, `"demu4xgpu"`, `"demu4xvdi"` |
| `slurm.rs` | 509 | `shorten_node_name()` strips `"demu4x"` |
| `tui/ui.rs` | 1942 | Duplicate `shorten_node_name()` |

**Fix:**
- Delete or make `shorten_node_name()` configurable via regex in config
- Rewrite `format_partition_stats()` to group by `node.partition` field from Slurm, not node name prefix

```rust
// Before (broken for other clusters)
if node.name.starts_with("demu4xcpu") {
    partitions.get_mut("CPU Nodes").unwrap().push(node);
}

// After (portable)
partitions
    .entry(node.partition.clone())
    .or_default()
    .push(node);
```

---

### [DONE] 2. Remove Hardcoded Partition Order

| File | Line | Hardcoded Value |
|------|------|-----------------|
| `tui/app.rs` | 1813 | `["cpu", "gpu", "fat", "vdi"]` |
| `tui/ui.rs` | 520 | `["cpu", "gpu", "fat", "vdi"]` |
| `display.rs` | 470 | `["CPU Nodes", "GPU Nodes", "Fat Nodes", "VDI Nodes"]` |

**Fix:**
- Detect partitions dynamically (already partially done in `compute_partition_stats()`)
- Make display order configurable, default to alphabetical
- Add to config:
  ```toml
  [display]
  partition_order = []  # Empty = alphabetical, or specify preferred order
  node_prefix_strip = ""  # Optional regex to strip from node names
  ```

---

### [DONE] 3. Fix `unwrap()` Panics

**File:** `display.rs:459-465`

```rust
// Before (panics if key missing)
partitions.get_mut("CPU Nodes").unwrap().push(node);

// After (safe)
partitions.entry("CPU Nodes").or_default().push(node);
```

---

## P1 — High Priority

### [DONE] 4. Consolidate Duplicate Code

`shorten_node_name()` now exists in a single location (`src/slurm.rs:511-517`) and is imported by both `display.rs` and `tui/ui.rs`.

---

### [DONE] 5. Auto-Detect Slurm Binary Paths

Implemented auto-detection of Slurm binary paths using the `which` crate:

- Added `find_slurm_bin_path()` function to `src/slurm.rs` that:
  1. First checks config file (highest priority)
  2. Then tries PATH via `which sinfo`
  3. Falls back to `/usr/bin`
- Added `SystemConfig` to `TuiConfig` with optional `slurm_bin_path` field
- Added `CMON_SLURM_PATH` environment variable override
- Updated both CLI and TUI to use the auto-detected path

---

### [DONE] 6. Auto-Detect Slurm Version

Implemented auto-detection of Slurm version for backward compatibility:

- Added `SlurmVersion` struct with `major`, `minor`, `patch` fields in `src/slurm.rs`
- Added `detect_slurm_version()` function that runs `sinfo --version` and parses the output
- Added `check_slurm_json_support()` function that warns if version < 21.08
- Integrated version check into `main.rs` startup (after connection test)
- Added comprehensive unit tests for version parsing (8 test cases)

---

### [DONE] 7. Enforce Clippy in CI

Removed `|| true` from the clippy step, so clippy warnings now fail the CI build.
Fixed all 53 clippy warnings including:
- Unused imports, variables, and dead code
- Style issues (useless format!, collapsible if, redundant closures)
- Modern Rust idioms (is_none_or, is_some_and, and_then, strip_prefix)

**File:** `.github/workflows/build-release.yml:45`

```yaml
- name: Run clippy
  run: cargo clippy -- -D warnings
```

---

## P2 — Medium Priority

### [DONE] 8. Implement Zero-Config First Run

Zero-config first run is now fully implemented:

1. **Auto-detect Slurm path on startup**: `find_slurm_bin_path()` in `src/slurm.rs:49` checks:
   - Config file setting (highest priority)
   - PATH via `which sinfo`
   - Fallback to `/usr/bin`

2. **Auto-detect Slurm version**: `check_slurm_json_support()` called in `main.rs:183`
   - Runs `sinfo --version` and parses output
   - Warns if version < 21.08 (no JSON support)

3. **Works without any config file**: `TuiConfig::load()` in `src/models.rs:1881`
   - Uses sensible defaults via `Self::default()`
   - Gracefully handles missing config files (no error)
   - Merges user config if present (overrides auto-detected values)

4. **User config overrides auto-detected**: Priority order in `find_slurm_bin_path()`:
   - Config path (if set) takes precedence
   - Environment variable `CMON_SLURM_PATH` also supported

Note: Implementation differs from original suggestion - auto-detection lives in `SlurmInterface::with_config()` rather than `TuiConfig::load()`, maintaining better separation of concerns.

---

### [DONE] 9. Add `--init-config` Command

Implemented the `init-config` subcommand to generate a template configuration file:

- Added `InitConfig` command variant with `--force` flag to overwrite existing config
- Handles command early in `main()` before Slurm connection check (works without Slurm)
- Generates well-documented template config at `~/.config/cmon/config.toml`
- Respects `XDG_CONFIG_HOME` environment variable for config path
- Proper error handling when config already exists (use `--force` to overwrite)

Usage:
```bash
cmon init-config           # Create template config file
cmon init-config --force   # Overwrite existing config file
```

---

### 10. Add Integration Tests with Mocked Slurm Output

**Problem:** No tests verify parsing without a live cluster.

**Fix:**
- Create `SlurmInterface` trait
- Implement `MockSlurmInterface` returning sample JSON fixtures
- Add `tests/` directory with integration tests

---

### 11. Split Large Files

| File | Lines | Action |
|------|-------|--------|
| `models.rs` | ~2,400 | Split: `models/job.rs`, `models/node.rs`, `models/config.rs` |
| `tui/app.rs` | ~2,400 | Extract filter logic, view state into submodules |
| `tui/ui.rs` | ~1,600 | Split renderers: `ui/jobs.rs`, `ui/nodes.rs` |

---

## P3 — Low Priority (Tech Debt)

### 12. Use XDG Base Directories

**File:** `src/models.rs` (TuiConfig::load)

```rust
use dirs::config_dir;

fn config_path() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::config_dir().unwrap_or_default())
        .join("cmon/config.toml")
}
```

---

### [DONE] 13. Log Invalid Config Files

Implemented proper warning handling for invalid config files in `TuiConfig::load_config_file()`:

- **Parse errors**: Warns user with path and error details, continues with defaults
- **Missing files**: Silently ignored (expected behavior for optional config)
- **Permission errors**: Warns user about access issues

**File:** `src/models.rs:1901-1920`

```rust
fn load_config_file(config: &mut Self, path: &str) {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            match toml::from_str::<TuiConfig>(&content) {
                Ok(parsed) => config.merge(parsed),
                Err(e) => {
                    eprintln!("Warning: Failed to parse config file '{}': {}", path, e);
                    eprintln!("Using default settings for this file.");
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File not found is expected and not an error
        }
        Err(e) => {
            // Other errors (permissions, etc.) should be reported
            eprintln!("Warning: Could not read config file '{}': {}", path, e);
        }
    }
}
```

---

### [DONE] 14. Extract Navigation Helper

Implemented `with_current_list()` helper in `src/tui/app.rs:1592-1612` that:

- Accepts a closure `F: FnOnce(&mut ListState, usize)` to apply navigation operations
- Centralizes view/panel matching logic in one place (was duplicated 6 times)
- Reduced navigation code from ~135 lines to ~50 lines
- Handles all views: Jobs, Nodes, Partitions, Personal (with panel variants), Problems (with panel variants)
- Gracefully handles Summary panel (no-op since it has no list)

**Before (repeated 6 times):**
```rust
fn navigate_up(&mut self) {
    let len = self.current_list_len();
    match self.current_view {
        View::Jobs => self.jobs_view.list_state.move_up(len),
        // ... 20+ more lines per function
    }
}
```

**After:**
```rust
fn with_current_list<F>(&mut self, f: F)
where F: FnOnce(&mut ListState, usize)
{
    let len = self.current_list_len();
    match self.current_view {
        View::Jobs => f(&mut self.jobs_view.list_state, len),
        View::Nodes => f(&mut self.nodes_view.list_state, len),
        // ...
    }
}

fn navigate_up(&mut self) {
    self.with_current_list(|state, len| state.move_up(len));
}
```

---

### [DONE] 15. Add `#[must_use]` Annotations

Added `#[must_use]` attribute to 90+ pure functions across the codebase:

**Files updated:**
- `src/models.rs`: TimeValue, FloatValue, ReasonInfo, NodeInfo (30+ is_* methods, memory_*, gpu_info), JobInfo (20+ is_* methods, allocated_*, gpu_*, remaining_time_*), ClusterStatus, JobHistoryInfo, SshareEntry, FairshareNode, FlatFairshareRow, SchedulerStats, format_duration_*
- `src/slurm.rs`: SlurmPathResult::is_fallback, SlurmVersion::supports_json, SlurmInterface::test_connection
- `src/tui/app.rs`: ConfirmAction, SortMenuState, ClipboardFeedback, SlurmTime, JobState, TuiJobInfo, PartitionStatus, DataSlice, JobsViewState, View, AccountContext, FilterState, App query methods (selected_*, current_*, compute_*, my_*, *_nodes, *_count, detail_job, is_modal_active, etc.)
- `src/tui/mod.rs`: ConnectionError::is_suitable, ConnectionError::error_message
- `src/tui/runtime.rs`: FetcherThrottle::get_multiplier, FetcherThrottle::is_idle

This helps catch bugs where computed values are accidentally discarded.

---

## Summary

| Priority | Done | Remaining | Focus |
|----------|------|-----------|-------|
| **P0** | 3 | 0 | Portability blockers — hardcoded cluster values |
| **P1** | 4 | 0 | Robustness — dedup, auto-detection, CI |
| **P2** | 2 | 2 | Usability — zero-config, testing, file structure |
| **P3** | 3 | 1 | Polish — XDG, logging, refactoring, #[must_use] |

**Recommended sequence:**

1. ~~P0.1-3 (remove all `demu4x` hardcoding)~~ DONE
2. ~~P1.4 (consolidate `shorten_node_name`)~~ DONE
3. ~~P1.5-6 (auto-detect binaries + version)~~ DONE
4. ~~P1.7 (CI enforcement)~~ DONE
5. ~~P2.8 (zero-config)~~ DONE
6. ~~P2.9 (`--init-config` command)~~ DONE
7. Remaining items as time permits

---

## Config File Schema (Current)

```toml
# ~/.config/cmon/config.toml

[system]
slurm_bin_path = ""       # [IMPLEMENTED] Auto-detected via PATH if empty
# Note: Slurm version is auto-detected at runtime (item 6), warns if < 21.08

[display]
partition_order = []      # [IMPLEMENTED] Empty = alphabetical
node_prefix_strip = ""    # [IMPLEMENTED] String to strip from node names
theme = "dark"            # dark | light
default_view = "jobs"     # jobs | nodes | partitions
show_all_jobs = false
show_grouped_by_account = false

[refresh]
jobs_interval = 5         # seconds
nodes_interval = 10       # seconds
fairshare_interval = 60   # seconds
idle_slowdown = true
idle_threshold = 30       # seconds before considered idle

[behavior]
confirm_cancel = true
copy_to_clipboard = true
```

Environment variable overrides:
- `CMON_SLURM_PATH` - Override slurm binary path
- `CMON_REFRESH_JOBS` - Override jobs refresh interval
- `CMON_REFRESH_NODES` - Override nodes refresh interval
- `CMON_DEFAULT_VIEW` - Override default view
- `CMON_THEME` - Override theme
- `CMON_NO_CLIPBOARD` - Disable clipboard support
