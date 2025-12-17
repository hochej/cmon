# Codebase Refactoring Plan: cmon

## Executive Summary

**Scope:** Complete refactoring of the codebase with full module restructuring (no backwards compatibility constraints).

**Key Goals:**
1. Split large files into logical submodules
2. Eliminate cross-file duplication via shared utilities
3. Fix non-idiomatic Rust patterns (stringly-typed state checking, unnecessary allocations)
4. Consolidate duplicate type definitions (JobState exists in two places)
5. Reduce boilerplate through traits and helpers

**Estimated Impact:**
- ~1,100+ lines reduced
- ~200 lines moved to new shared modules
- Significantly improved maintainability and contributor-friendliness

---

## Target File Structure

```
src/
  lib.rs                    # NEW: Library root (optional, for better testing)
  main.rs                   # CLI entry point (simplified)
  formatting.rs             # NEW: Shared formatting utilities

  models/                   # SPLIT from models.rs (3,201 lines)
    mod.rs                  # Re-exports
    job.rs                  # JobInfo, JobHistoryInfo
    node.rs                 # NodeInfo
    state.rs                # State enums, predicates, priority constants
    config.rs               # TuiConfig, RefreshConfig, DisplayConfig
    slurm_responses.rs      # Squeue/Sinfo/Sacct/Sshare response types
    time.rs                 # TimeValue, duration helpers
    fairshare.rs            # SshareEntry, FairshareNode
    scheduler.rs            # SchedulerStats

  slurm.rs                  # Slurm interface (simplified)
  display.rs                # CLI output (imports from formatting.rs)

  tui/
    mod.rs                  # TUI entry point (simplified)
    runtime.rs              # Async orchestration
    event.rs                # Event types
    theme.rs                # Color themes
    format.rs               # NEW: TUI-specific formatting (ratatui Spans)

    app/                    # SPLIT from app.rs (2,768 lines)
      mod.rs                # App struct, main logic
      state.rs              # ListState, ModalState, ViewState types
      types.rs              # TuiJobInfo, JobId, SlurmTime
      export.rs             # Export logic with Exportable trait
      filter.rs             # Filter types and matching

    ui/                     # SPLIT from ui.rs (2,567 lines)
      mod.rs                # Main render dispatch
      jobs.rs               # Jobs view rendering
      nodes.rs              # Nodes view rendering
      partitions.rs         # Partitions view rendering
      personal.rs           # Personal dashboard rendering
      problems.rs           # Problem nodes rendering
      overlays.rs           # Help, filter, sort, detail popups
      widgets.rs            # Reusable table/panel helpers
```

---

## Phase 0: Critical Idiomatic Rust Fixes (HIGH PRIORITY)

### 0.1 Fix Stringly-Typed State Checking (Performance + Safety) [DONE]

**File:** `src/models.rs`

**Problem:** Every state check allocates a new String:
```rust
// Current - allocates on EVERY call
pub fn is_running(&self) -> bool {
    self.state.contains(&"RUNNING".to_string())
}
```

**Fix:** Use `&str` comparison:
```rust
pub fn is_running(&self) -> bool {
    self.state.iter().any(|s| s == "RUNNING")
}
```

Apply to all 20+ `is_*()` methods in both `JobInfo` and `NodeInfo`.

### 0.2 Consolidate Duplicate JobState Definitions [DONE]

**Problem:** Two separate implementations:
- `models::JobInfo` - uses `Vec<String>` with string predicates
- `tui::app::JobState` - proper enum with conversions

**Fix:** Create single authoritative `JobState` enum in `models/state.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Running,
    Pending,
    Completing,
    Completed,
    Failed,
    Cancelled,
    Timeout,
    NodeFail,
    OutOfMemory,
    Suspended,
    Preempted,
    Unknown,
}

impl JobState {
    pub fn from_slurm_strings(states: &[String]) -> Self {
        // Priority-based matching using STATE_PRIORITY constant
    }

    pub fn as_str(&self) -> &'static str { ... }
    pub fn short_str(&self) -> &'static str { ... }
}
```

Remove duplicate from `tui/app.rs`, import from models.

### 0.3 Use Iterator Combinators for GPU Parsing [DONE]

**File:** `src/models.rs` (lines 862-870)

```rust
// Current - manual loop
for (key, value) in resources.iter() {
    if key.contains("gres/gpu") && let Ok(count) = value.parse::<u32>() {
        return count;
    }
}
0

// Idiomatic
self.allocated_resources()
    .iter()
    .find_map(|(k, v)| k.contains("gres/gpu").then(|| v.parse().ok()).flatten())
    .unwrap_or(0)
```

### 0.4 Replace Clone with as_deref() Where Appropriate [DONE]

**File:** `src/tui/ui.rs` (line 718 and similar)
```rust
// When you only need &str (not owned):
let partition = node.partition.name.as_deref().unwrap_or("");

// When you need owned String with empty default, keep the original:
let partition: String = node.partition.name.clone().unwrap_or_default();

// When you need owned String with non-empty default:
let partition: String = node.partition.name.as_deref().unwrap_or("unknown").to_string();
```

**Applied to:** TUI code where only `&str` is needed (3 locations in ui.rs).
**Not applied to:** `display.rs` where owned `String` with empty default is needed.

---

## Phase 1: Create Shared Formatting Module

### 1.1 Create `src/formatting.rs` [DONE]

Consolidate duplicated functions from across the codebase:

```rust
//! Shared formatting utilities used by both CLI and TUI

/// Truncate string with ellipsis (duplicated in display.rs:1275, ui.rs:2031)
pub fn truncate_string(s: &str, max_len: usize) -> String;

/// Format duration as human-readable "1h 30m" (from models.rs:1527)
pub fn format_duration_human(seconds: u64) -> String;

/// Format duration as HH:MM:SS (duplicated in app.rs:499, ui.rs:2053)
pub fn format_duration_hms(seconds: u64) -> String;

/// Format bytes from raw bytes (from display.rs:1602)
pub fn format_bytes(bytes: u64) -> String;

/// Format bytes from megabytes (duplicated in display.rs:96, ui.rs:2042)
pub fn format_bytes_mb(mb: u64) -> String;

/// Layout constants
pub mod layout {
    pub const BOX_WIDTH: usize = 78;
    pub const TABLE_WIDTH_DEFAULT: usize = 200;
    pub const TABLE_WIDTH_COMPACT: usize = 120;
    pub const TEXT_WRAP_WIDTH: usize = 72;
    pub const PATH_TRUNCATE_LEN: usize = 50;
    pub const JOB_NAME_MAX_LEN: usize = 35;
    pub const JOB_NAME_BRIEF_LEN: usize = 20;
    pub const BAR_LENGTH: usize = 20;
}

/// Efficiency/utilization thresholds
pub mod thresholds {
    pub const EFFICIENCY_LOW: f64 = 30.0;
    pub const EFFICIENCY_HIGH: f64 = 70.0;
    pub const UTILIZATION_LOW: f64 = 50.0;
    pub const UTILIZATION_HIGH: f64 = 80.0;
}
```

### 1.2 Update Imports [DONE]

- `display.rs`: Remove local `truncate_string`, `format_bytes`, import from `formatting`
- `tui/ui.rs`: Remove local `truncate_string`, `format_memory`, `format_duration_display`
- `tui/app.rs`: Remove local `format_duration`
- `models.rs`: Move duration helpers to `formatting`, keep re-export

**Savings:** ~80 lines eliminated, single source of truth

---

## Phase 2: Split models.rs into Submodules

### 2.1 Create Module Structure [DONE]

**From:** `src/models.rs` (3,201 lines)
**To:** `src/models/` directory

| New File | Contents | Est. Lines |
|----------|----------|------------|
| `mod.rs` | Re-exports | ~50 |
| `job.rs` | JobInfo, JobHistoryInfo, job methods | ~400 |
| `node.rs` | NodeInfo, node methods | ~350 |
| `state.rs` | JobState, NodeState enums, STATE_PRIORITY constants | ~200 |
| `config.rs` | TuiConfig, RefreshConfig, DisplayConfig, validation | ~400 |
| `slurm_responses.rs` | Squeue/Sinfo/Sacct/Sshare response wrappers | ~300 |
| `time.rs` | TimeValue enum, serialization | ~150 |
| `fairshare.rs` | SshareEntry, FairshareNode, tree building | ~200 |
| `scheduler.rs` | SchedulerStats, sdiag parsing | ~150 |

### 2.2 Refactor `primary_state()` with Const Arrays [DONE]

```rust
// In models/state.rs
pub const JOB_STATE_PRIORITY: &[(&str, &[&str])] = &[
    ("RUNNING", &["RUNNING", "RUN", "R"]),
    ("PENDING", &["PENDING", "PD"]),
    ("COMPLETING", &["COMPLETING", "CG"]),
    // ... rest
];

pub const NODE_STATE_PRIORITY: &[(&str, &[&str])] = &[
    ("DOWN", &["DOWN"]),
    ("FAIL", &["FAIL"]),
    ("DRAINING", &["DRAINING", "DRAIN", "DRNG"]),
    // ... rest
];

impl JobInfo {
    pub fn primary_state(&self) -> &str {
        for (display, variants) in JOB_STATE_PRIORITY {
            if self.has_state(variants) { return display; }
        }
        self.state.first().map(|s| s.as_str()).unwrap_or("UNKNOWN")
    }
}
```

**Savings:** ~100 lines (2 methods x 50 lines each)

### 2.2b Add `define_state_checkers!` Macro for is_* Methods [DONE]

Added declarative macro to generate all `is_*()` state checking methods from a single
definition, eliminating ~50 repetitive methods across JobInfo, NodeInfo, and JobHistoryInfo:

```rust
// In models/state.rs
macro_rules! define_state_checkers {
    ($($method:ident => [$($state:literal),+ $(,)?]),* $(,)?) => {
        $(
            #[must_use]
            pub fn $method(&self) -> bool {
                self.has_state(&[$($state),+])
            }
        )*
    }
}

// Usage in job.rs, node.rs:
define_state_checkers! {
    is_running => ["RUNNING"],
    is_pending => ["PENDING"],
    is_draining => ["DRAINING", "DRAIN", "DRNG"],
    // ... comprehensive list of all Slurm states
}
```

Also extended `JobState` enum with `BootFail` and `Deadline` variants for complete
Slurm base state coverage.

**Savings:** ~140 lines (net reduction)

### 2.3 Extract Config Validation Helper [DONE]

Extracted `validate_interval()` helper function with a type-safe `RefreshField` enum
to reduce repetitive validation code in `RefreshConfig::validate()`. Uses an enum
instead of string literals for compile-time field name verification:

```rust
// In models/config.rs
#[derive(Clone, Copy)]
enum RefreshField {
    JobsInterval,
    NodesInterval,
    FairshareInterval,
    IdleThreshold,
}

fn validate_interval(
    value: &mut u64,
    field: RefreshField,  // Type-safe instead of &str
    min: u64,
    default: u64,
    strict: bool,
    warnings: &mut Vec<String>,
) -> Result<(), String> { ... }

// Usage (replaces 4 repetitive blocks):
validate_interval(&mut self.jobs_interval, RefreshField::JobsInterval, ...)?;
validate_interval(&mut self.nodes_interval, RefreshField::NodesInterval, ...)?;
```

**Benefits:** Type-safe field names, compile-time verification, improved maintainability

---

## Phase 3: Simplify slurm.rs and main.rs

### 3.1 DRY Command Execution in slurm.rs [DONE]

**Problem:** `get_nodes()`, `get_jobs()`, `get_job_history()`, `get_fairshare()` repeat same 8-step pattern.

**Solution:** Created a `SlurmResponse` trait and generic `execute_slurm_command` helper:

```rust
// In models/slurm_responses.rs
/// Trait for Slurm command responses that have an errors field.
pub trait SlurmResponse {
    fn errors(&self) -> &[String];
}

// In slurm.rs
fn execute_slurm_command<T>(&self, mut cmd: Command, error_context: &str) -> Result<T>
where
    T: DeserializeOwned + SlurmResponse,
{
    let output = cmd.output()
        .with_context(|| format!("Failed to execute {} command", error_context))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{} command failed: {}", error_context, stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: T = serde_json::from_str(&stdout)
        .with_context(|| format!("Failed to parse {} JSON output", error_context))?;

    if !response.errors().is_empty() {
        anyhow::bail!("{} errors: {}", error_context, response.errors().join("; "));
    }

    Ok(response)
}
```

**Key improvements over original plan:**
- Added `SlurmResponse` trait for type-safe error handling across all response types
- Helper takes a pre-built `Command` (more flexible than args slice)
- Trait implemented for all 4 response types: `SinfoResponse`, `SqueueResponse`, `SacctResponse`, `SshareResponse`

**Savings:** ~60 lines of duplicated command execution/parsing code

### 3.2 DRY Watch Loop in main.rs [DONE]

**Problem:** Watch loop pattern repeated 6 times for different commands.

**Solution:** Created `run_with_optional_watch` helper function that reuses the existing
`watch_loop` for watch mode and adds a simple "run once" branch:

```rust
fn run_with_optional_watch<F>(watch: f64, render_fn: F) -> Result<()>
where
    F: Fn() -> Result<String>,
{
    if watch > 0.0 {
        watch_loop(watch, render_fn)
    } else {
        println!("{}", render_fn()?);
        Ok(())
    }
}
```

Refactored all 6 commands (Jobs, Nodes, Status, Partitions, Me, Down) to use this helper,
eliminating ~36 lines of duplicated if/else boilerplate.

**Original proposed fix:**
```rust
fn run_with_optional_watch<F>(
    watch: bool,
    interval: u64,
    render_fn: F,
) -> Result<()>
where
    F: Fn() -> Result<String>,
{
    if watch {
        loop {
            print_synchronized(&render_fn()?);
            std::thread::sleep(Duration::from_secs(interval));
        }
    } else {
        println!("{}", render_fn()?);
        Ok(())
    }
}
```

**Savings:** ~60 lines

### 3.3 Remove Dead Code [DONE]

Removed methods and types marked `#[allow(dead_code)]` that were "kept for potential future use":

**event.rs:**
- Removed `DataSource::Partitions` variant (never constructed as error source)
- Removed `DataEvent::PartitionsUpdated`, `ForceRefresh`, `Shutdown` variants (never emitted)
- Removed unused `job_id` field from `JobCancelResult`

**theme.rs:**
- Removed `bg` field (never used after construction)
- Removed `progress_empty` field (never used)

**slurm.rs:**
- Removed `SlurmPathResult::is_fallback()` method
- Removed `SlurmInterface::new()` constructor (redundant with `Default`)

**Savings:** ~40 lines

---

## Phase 4: Split tui/app.rs into Submodules

### 4.1 Create Module Structure [DONE]

Split `src/tui/app.rs` (2,653 lines) into `src/tui/app/` directory with logical separation:

| New File | Contents | Lines |
|----------|----------|-------|
| `mod.rs` | App struct, handle_input, handle_data, re-exports | ~850 |
| `state.rs` | ListState, ModalState, ViewState, *ViewState types, DataCache, FeedbackState | ~600 |
| `types.rs` | TuiJobInfo, JobId, SlurmTime, PartitionStatus, DataSlice | ~350 |
| `export.rs` | escape_csv helper | ~40 |
| `filter.rs` | job_matches_* functions with comprehensive tests | ~170 |

**Key decisions:**
- Export methods remain on App (need access to feedback state)
- Filter functions extracted as standalone (pure functions)
- JobsViewState gains `new()` constructor to handle private fields
- All 85 tests pass, clippy clean

### 4.2 Consolidate Selection Accessors [DONE]

Renamed `detail_job()` to `focused_job()` and refactored to use the existing helper methods
instead of duplicating code. The new unified accessor consolidates:
- `selected_job()` (Jobs view)
- `personal_running_job()` (Personal view, Running panel)
- `personal_pending_job()` (Personal view, Pending panel)

```rust
/// Get the currently focused job across any view where a job can be selected
pub fn focused_job(&self) -> Option<&TuiJobInfo> {
    match self.current_view {
        View::Jobs => self.selected_job(),
        View::Personal => self.personal_running_job().or_else(|| self.personal_pending_job()),
        _ => None,
    }
}
```

**Additional fixes:**
- Fixed bug in `handle_detail_action`: Cancel from detail view now works when opened from Personal view
- `yank_selected_job_id()` now uses `focused_job()`, enabling clipboard copy from Personal view
- Simplified `KeyAction::Select` handler from 15 lines to 4 lines

**Savings:** ~20 lines (code reduction) + 1 bug fix

### 4.3 Implement Exportable Trait [DONE]

Implemented an `Exportable` trait that consolidates export logic for jobs, nodes, and partitions:

```rust
/// Trait for types that can be exported to JSON and CSV formats.
pub trait Exportable {
    fn csv_headers() -> &'static [&'static str];
    fn to_csv_row(&self) -> Vec<String>;
    fn to_json_value(&self) -> serde_json::Value;
}

/// Blanket implementation for references - allows exporting Vec<&T>
impl<T: Exportable> Exportable for &T { ... }

/// Generic export function
pub fn export_items<T: Exportable>(items: &[T], format: ExportFormat) -> String { ... }
```

**Key design decisions:**
- Used `to_json_value()` instead of `Serialize` bound - allows custom field selection for export
- Added blanket implementation for `&T` to support `Vec<&TuiJobInfo>` from `get_display_jobs()`
- CSV escaping handled centrally in `export_items()`, not in each `to_csv_row()` impl
- Implemented trait for `TuiJobInfo`, `NodeInfo`, and `PartitionStatus`

**Refactored methods (before -> after):**
- `export_jobs()`: 75 lines -> 10 lines
- `export_nodes()`: 65 lines -> 10 lines
- `export_partitions()`: 65 lines -> 10 lines

**Savings:** ~175 lines reduced + cleaner separation of concerns

---

## Phase 5: Split tui/ui.rs into Submodules

### 5.1 Create Module Structure [DONE]

Split `src/tui/ui.rs` (2,532 lines) into `src/tui/ui/` directory with logical submodules:

| New File | Contents | Lines |
|----------|----------|-------|
| `mod.rs` | Main render(), tab/info/status bar, dispatch | ~316 |
| `widgets.rs` | Shared helpers (table header, scroll, progress bar) | ~62 |
| `jobs.rs` | Jobs view (flat list, grouped-by-account), job_to_row | ~270 |
| `nodes.rs` | Nodes view (list + grid modes), node detail footer | ~361 |
| `partitions.rs` | Partitions cards, resource utilization bars | ~282 |
| `personal.rs` | Personal dashboard (summary, fairshare, jobs panels) | ~439 |
| `problems.rs` | Problem nodes (down/draining sections) | ~245 |
| `overlays.rs` | Help, filter, detail popup, confirm, sort, toast | ~640 |

**Key design decisions:**
- Kept each view's rendering logic self-contained in its own file
- Shared helpers (`create_table_header`, `calculate_scroll_offset`, `centered_rect`,
  `create_progress_bar`) extracted to widgets.rs
- Status bar and info bar kept in mod.rs as they access multiple view states
- Total: ~2,615 lines (slight increase from imports/headers, expected for better organization)

### 5.2 Create `src/tui/ui/widgets.rs` [DONE]

Added `section_header()` and `detail_row()` helpers to widgets.rs, refactoring
`render_job_detail_popup()` in overlays.rs to use them. The more complex proposed
helpers (`render_panel()` and `render_table()`) were not implemented as they would
add abstraction without clear benefit - each usage has unique variations that don't
fit a single pattern cleanly.

**Implemented:**
- `section_header(title, theme)` - Creates styled section headers for detail popups
- `detail_row(label, value)` - Creates simple key-value detail rows

**Savings:** ~26 lines (net reduction)

**Original proposed patterns (preserved for reference):**

Shared rendering patterns:

```rust
/// Render a panel with focus-aware border
pub fn render_panel(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    focused: bool,
    theme: &Theme,
    content: impl FnOnce(&mut Frame, Rect),
);

/// Render a scrollable table with selection
pub fn render_table<T, F>(
    frame: &mut Frame,
    area: Rect,
    items: &[T],
    headers: &[&str],
    widths: &[Constraint],
    selected: usize,
    scroll_offset: usize,
    theme: &Theme,
    row_builder: F,
) where F: Fn(&T, bool) -> Row<'_>;

/// Section header for detail popup
pub fn section_header<'a>(title: &'a str, theme: &Theme) -> Line<'a>;

/// Key-value row for detail popup
pub fn detail_row<'a>(label: &'a str, value: impl Into<Span<'a>>) -> Line<'a>;
```

### 5.3 Unify Problem Nodes Rendering [DONE]

Merged `render_down_nodes_section()` and `render_draining_nodes_section()` into a single
`render_problem_nodes_section()` function that uses the existing `ProblemsPanel` enum:

```rust
fn render_problem_nodes_section(
    app: &App,
    nodes: &[&NodeInfo],
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
    panel: ProblemsPanel,  // Reuses existing enum instead of new ProblemNodeType
) {
    // Configuration derived from panel type via match:
    // - title_prefix, empty_msg, unfocused_color, state_color, selected_idx
}
```

**Key decision:** Reused the existing `ProblemsPanel` enum from `state.rs` instead of
creating a new `ProblemNodeType` enum, avoiding redundant type definitions.

**Savings:** 54 lines (94 deletions, 40 insertions)

### 5.4 Unify Personal Jobs Panels [DONE]

Merged `render_personal_running_jobs()` and `render_personal_pending_jobs()` into a single
`render_personal_jobs_panel()` function that uses the existing `PersonalPanel` enum:

```rust
/// Unified rendering function for personal job panels (Running and Pending).
/// Uses the existing `PersonalPanel` enum to distinguish between panel types.
fn render_personal_jobs_panel(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
    panel: PersonalPanel,  // Reuses existing enum instead of new PersonalJobType
) {
    // Configuration derived from panel type via match:
    // - title_prefix, empty_msg, selected_idx, headers, column widths
    // - Row rendering logic with match for running (time remaining w/ colors) vs pending
}
```

**Key decision:** Reused the existing `PersonalPanel` enum from `state.rs` instead of
creating a new `PersonalJobType` enum, following the pattern established in Phase 5.3.

**Actual savings:** 34 lines (130 deletions, 96 insertions) - less than predicted ~80 lines
because the row rendering logic differs significantly (running jobs have color-coded time
remaining display while pending jobs show reason and estimated start).

### 5.5 Decompose `render_job_detail_popup()` [DONE]

Extracted section builders into `overlays.rs`:

```rust
fn build_job_header_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_time_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_resources_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_paths_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_extras_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_footer_section(theme: &Theme) -> Vec<Line>;
```

**Key decisions:**
- Renamed `build_array_section` to `build_extras_section` - handles dependencies, array job info,
  priority, and constraint (all optional job configuration details)
- Added `build_footer_section` for keybindings (kept separate for clarity)
- None of the helpers need `app: &App` - they work purely on `TuiJobInfo` and `Theme`
- The main `render_job_detail_popup` is now ~45 lines (down from ~280 lines)

**Actual savings:** ~235 lines (280 -> 45 in main function) reorganized into focused helpers

---

## Phase 6: Display Simplification

### 6.1 Create BoxColor Enum (Type Safety) [DONE]

Replaced stringly-typed color API with type-safe `BoxColor` enum:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // Yellow and Blue kept for API completeness
pub enum BoxColor { Green, Red, Yellow, Blue }

impl BoxColor {
    fn apply(&self, text: &str) -> String {
        match self {
            BoxColor::Green => text.green().to_string(),
            BoxColor::Red => text.red().to_string(),
            BoxColor::Yellow => text.yellow().to_string(),
            BoxColor::Blue => text.blue().to_string(),
        }
    }
}
```

**Refactored functions:**
- `box_top_colored(title, color: BoxColor)` - 10 lines -> 3 lines
- `box_bottom_colored(color: BoxColor)` - 9 lines -> 3 lines
- `box_empty_colored(color: BoxColor)` - 9 lines -> 3 lines
- `pad_line_colored(content, color: BoxColor)` - 23 lines -> 14 lines

**Savings:** ~31 lines + compile-time type safety (no more typos like `"gren"`)

### 6.2 Decompose `format_job_details()` [DONE]

Decomposed the 195-line `format_job_details()` function into 7 focused section builder helpers:

```rust
fn build_job_basic_info(job: &JobHistoryInfo, node_prefix_strip: &str) -> Vec<String>;
fn build_job_time_info(job: &JobHistoryInfo) -> Vec<String>;
fn build_job_resources(job: &JobHistoryInfo) -> Vec<String>;
fn build_job_efficiency(job: &JobHistoryInfo) -> Vec<String>;
fn build_job_exit_info(job: &JobHistoryInfo) -> Option<Vec<String>>;  // conditional
fn build_job_paths(job: &JobHistoryInfo) -> Vec<String>;
fn build_job_submit_line(job: &JobHistoryInfo) -> Option<Vec<String>>;  // conditional
```

**Key decisions:**
- Used `JobHistoryInfo` (actual type used by the function) instead of proposed `JobInfo`
- Added `build_job_exit_info()` and `build_job_submit_line()` for conditional sections
- Each helper returns padded lines ready for output (no need for caller to apply padding)
- Main function reduced to ~60 lines (clear section assembly with separator handling)
- Used `build_` prefix for consistency with TUI codebase conventions (Phase 5.5)

**Savings:** ~50 lines (net) + improved maintainability and testability

### 6.3 Create Generic Table Builder [DONE]

Added `build_styled_table<T: Tabled>(rows: Vec<T>, max_width: usize)` helper function to
consolidate the repeated table construction pattern across 5 functions in `display.rs`:

```rust
fn build_styled_table<T: Tabled>(rows: Vec<T>, max_width: usize) -> String {
    let mut table = Table::new(rows);
    table
        .with(Style::rounded())
        .with(Width::wrap(max_width).keep_words(true))
        .with(Modify::new(Rows::first()).with(Alignment::center()));
    table.to_string()
}
```

**Refactored functions:**
- `format_nodes()` (max_width: 200)
- `format_jobs()` (max_width: 200)
- `format_job_history_brief()` (max_width: 200)
- `format_job_history()` (max_width: 200)
- `format_problem_nodes_table()` (max_width: 120 - compact view)

**Savings:** ~25 lines + single source of truth for table styling

---

## Phase 7: TUI Support Files Cleanup

### 7.1 Simplify TerminalCapabilities (tui/mod.rs)

Replace 50-line struct with simple function:

```rust
fn check_terminal_capabilities() -> (bool, bool) {
    let is_tty = std::io::stdout().is_terminal();
    let term = std::env::var("TERM").unwrap_or_default();
    let supports_alt_screen = !term.contains("dumb") && !term.is_empty();
    (is_tty, supports_alt_screen)
}
```

### 7.2 Make Scheduler Stats Fetcher Consistent (tui/runtime.rs)

Use `handle_fetch_result` helper like other fetchers.

### 7.3 Audit Dead Code

Remove unused variants:
- `InputEvent::Resize` (if truly unused)
- `DataSource::Partitions` (if truly unused)
- `Theme::name`, `Theme::bg`, `Theme::progress_empty`

---

## Implementation Order

| Order | Phase | Risk | Impact | Est. Savings |
|-------|-------|------|--------|--------------|
| 1 | 0.1-0.4 | Low | Critical | ~0 lines, performance + safety |
| 2 | 1.1-1.2 | Low | High | ~80 lines |
| 3 | 2.1-2.3 | Medium | High | ~130 lines |
| 4 | 3.1-3.3 | Low | Medium | ~190 lines |
| 5 | 4.1-4.3 | Medium | High | ~140 lines |
| 6 | 5.1-5.5 | Medium | High | ~310 lines |
| 7 | 6.1-6.3 | Low | Medium | ~75 lines |
| 8 | 7.1-7.3 | Low | Low | ~80 lines |

**Total: ~1,005 lines reduced + ~200 lines reorganized**

---

## Files Summary

| Category | Before | After |
|----------|--------|-------|
| **models.rs** | 3,201 lines (1 file) | ~2,150 lines (9 files) |
| **tui/app.rs** | 2,768 lines (1 file) | ~1,850 lines (5 files) |
| **tui/ui.rs** | 2,567 lines (1 file) | ~1,700 lines (8 files) |
| **display.rs** | 1,656 lines | ~1,580 lines |
| **slurm.rs** | 1,162 lines | ~1,030 lines |
| **main.rs** | 737 lines | ~680 lines |
| **New: formatting.rs** | 0 | ~100 lines |

---

## Testing Strategy

1. **After each phase:** `cargo test && cargo clippy`
2. **Manual testing:**
   - `cmon jobs`, `cmon jobs --all`, `cmon nodes`, `cmon status`
   - `cmon tui` - all 5 views, filters, sorting, export
   - Job cancellation flow
   - Watch mode (`--watch`)
3. **Ensure no regressions in:**
   - State display colors
   - Column widths and truncation
   - Scroll behavior
   - Keyboard shortcuts

---

## Notes

- No backwards compatibility required - free to rename/restructure anything
- State predicate methods (`is_*()`) are kept for clear API - just fix the allocation issue
- O(N^2) fairshare tree building is acceptable for typical cluster sizes
- Theme structure is intentionally flexible for future customization
