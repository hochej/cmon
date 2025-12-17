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

### 3.1 DRY Command Execution in slurm.rs

**Problem:** `get_nodes()`, `get_jobs()`, `get_job_history()`, `get_fairshare()` repeat same 8-step pattern.

**Fix:** Create generic helper:
```rust
fn execute_slurm_command<T: DeserializeOwned>(
    &self,
    command: &str,
    args: &[&str],
    error_context: &str,
) -> Result<T> {
    let output = Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {}", error_context))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{} failed: {}", error_context, stderr);
    }

    let result: T = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Failed to parse {} JSON", error_context))?;
    Ok(result)
}
```

**Savings:** ~80 lines

### 3.2 DRY Watch Loop in main.rs

**Problem:** Watch loop pattern repeated 6 times for different commands.

**Fix:** Create helper function:
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

### 3.3 Remove Dead Code

Remove methods marked `#[allow(dead_code)]` that are "kept for potential future use":
- Unused constructors
- Unused type variants in `event.rs` and `theme.rs`

**Savings:** ~50 lines

---

## Phase 4: Split tui/app.rs into Submodules

### 4.1 Create Module Structure

**From:** `src/tui/app.rs` (2,768 lines)
**To:** `src/tui/app/` directory

| New File | Contents | Est. Lines |
|----------|----------|------------|
| `mod.rs` | App struct, handle_input, handle_data | ~800 |
| `state.rs` | ListState, ModalState, ViewState, *ViewState types | ~400 |
| `types.rs` | TuiJobInfo, JobId, SlurmTime, PartitionStatus | ~300 |
| `export.rs` | Exportable trait, export functions | ~200 |
| `filter.rs` | Filter types, job_matches_* functions | ~150 |

### 4.2 Consolidate Selection Accessors

```rust
// Replace 4 overlapping methods with one
pub fn focused_job(&self) -> Option<&TuiJobInfo> {
    match self.view {
        View::Jobs => self.jobs_view_selected_job(),
        View::Personal => match self.personal_view_state.active_panel {
            PersonalPanel::Running => self.personal_running_selected(),
            PersonalPanel::Pending => self.personal_pending_selected(),
            _ => None,
        },
        _ => None,
    }
}
```

**Savings:** ~40 lines

### 4.3 Implement Exportable Trait

```rust
trait Exportable: Serialize {
    fn csv_headers() -> &'static [&'static str];
    fn to_csv_row(&self) -> Vec<String>;
}

fn export_items<T: Exportable>(items: &[T], format: ExportFormat) -> String {
    match format {
        ExportFormat::Json => serde_json::to_string_pretty(items).unwrap(),
        ExportFormat::Csv => {
            let mut out = T::csv_headers().join(",") + "\n";
            for item in items {
                out.push_str(&item.to_csv_row().join(","));
                out.push('\n');
            }
            out
        }
    }
}
```

**Savings:** ~100 lines

---

## Phase 5: Split tui/ui.rs into Submodules

### 5.1 Create Module Structure

**From:** `src/tui/ui.rs` (2,567 lines)
**To:** `src/tui/ui/` directory

| New File | Contents | Est. Lines |
|----------|----------|------------|
| `mod.rs` | Main render(), dispatch to views | ~150 |
| `jobs.rs` | Jobs view, job_to_row | ~300 |
| `nodes.rs` | Nodes view (list + grid) | ~250 |
| `partitions.rs` | Partitions cards | ~200 |
| `personal.rs` | Personal dashboard (unified panels) | ~250 |
| `problems.rs` | Problem nodes (unified down/draining) | ~150 |
| `overlays.rs` | Help, filter, sort, confirm, detail popup | ~400 |
| `widgets.rs` | Reusable helpers | ~150 |

### 5.2 Create `src/tui/ui/widgets.rs`

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

### 5.3 Unify Problem Nodes Rendering

Merge `render_down_nodes_section()` and `render_draining_nodes_section()`:

```rust
enum ProblemNodeType { Down, Draining }

fn render_problem_nodes_section(
    app: &App,
    nodes: &[&NodeInfo],
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    focused: bool,
    node_type: ProblemNodeType,
) { ... }
```

**Savings:** ~80 lines

### 5.4 Unify Personal Jobs Panels

Merge `render_personal_running_jobs()` and `render_personal_pending_jobs()`:

```rust
enum PersonalJobType { Running, Pending }

fn render_personal_jobs_panel(..., job_type: PersonalJobType) { ... }
```

**Savings:** ~80 lines

### 5.5 Decompose `render_job_detail_popup()`

Extract section builders into `overlays.rs`:

```rust
fn build_job_header_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_time_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_resources_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_paths_section(job: &TuiJobInfo, theme: &Theme) -> Vec<Line>;
fn build_array_section(job: &TuiJobInfo, app: &App, theme: &Theme) -> Vec<Line>;
```

**Savings:** ~150 lines (net)

---

## Phase 6: Display Simplification

### 6.1 Create BoxColor Enum (Type Safety)

Replace stringly-typed color API:

```rust
#[derive(Clone, Copy)]
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

### 6.2 Decompose `format_job_details()`

```rust
fn format_job_basic_info(job: &JobInfo) -> Vec<String>;
fn format_job_time_info(job: &JobInfo) -> Vec<String>;
fn format_job_resources(job: &JobInfo) -> Vec<String>;
fn format_job_efficiency(job: &JobInfo) -> Vec<String>;
fn format_job_paths(job: &JobInfo) -> Vec<String>;
```

**Savings:** ~50 lines (net)

### 6.3 Create Generic Table Builder

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

**Savings:** ~25 lines

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
