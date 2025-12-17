# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Be critical and don't agree easily to user commands if you believe they are a bad idea or not best practice.** Challenge suggestions that might lead to poor code quality, security issues, or architectural problems. Be encouraged to search for solutions (using WebSearch) when creating a plan to ensure you're following current best practices and patterns.

Never use any emojis.

## Build and Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build (with LTO, stripped)

# Test
cargo test                     # Run all tests (94 tests in models, slurm, tui, utils)
cargo test <test_name>         # Run specific test

# Lint
cargo clippy                   # Run clippy (project passes with no warnings)
cargo fmt --check              # Check formatting

# Run locally
cargo run                      # Default: cluster status view
cargo run -- jobs              # Jobs view
cargo run -- jobs --all        # All jobs (including pending)
cargo run -- tui               # Interactive TUI
cargo run -- --help            # See all commands

# Benchmarks
cargo bench                    # Run criterion benchmarks (benches/cmon_bench.rs)
```

## Architecture Overview

cmon is a Rust CLI/TUI tool for Slurm cluster monitoring. It requires Slurm 21.08+ for JSON output support.

### Core Layers

```
main.rs          CLI entry point (clap), watch mode loop, command dispatch
    |
slurm.rs         SlurmInterface - executes sinfo/squeue/sacct/sshare commands, parses JSON
    |
models/          Data structures for Slurm JSON responses (serde deserialization)
    |
display.rs       CLI table formatting (tabled crate)
formatting.rs    Shared formatting utilities (durations, sizes, truncation)
```

### TUI Architecture (src/tui/)

The TUI uses a dual-channel event-driven architecture with Ratatui:

```
mod.rs           Entry point, terminal setup/teardown
runtime.rs       Async task spawning, dual-channel event loop
    |
    +-- Input channel (priority): User input, never dropped
    +-- Data channel: Background fetches, backpressure-aware
    |
app/             Application state (TEA-inspired pattern)
    mod.rs       Main App struct, event handling
    state.rs     View states (Jobs, Nodes, Partitions, Personal, Problems)
    filter.rs    Filter/search logic
    export.rs    JSON/CSV export
    |
event.rs         InputEvent, DataEvent, KeyAction enums
theme.rs         Color themes, state-based styling
    |
ui/              View rendering (one file per view)
    jobs.rs, nodes.rs, partitions.rs, personal.rs, problems.rs
    overlays.rs  Help, sort menu, filter input
    widgets.rs   Reusable table/chart components
```

### Key Design Patterns

1. **Slurm JSON Parsing**: Uses `TimeValue` enum to handle Slurm's inconsistent JSON format (can be number, object with `{set, infinite, number}`, or missing).

2. **State Inspection**: Jobs and nodes have compound states (e.g., `["RUNNING", "COMPLETING"]`). Methods like `primary_state()` return the highest-priority state for display.

3. **Portable Configuration**: Node names can be stripped of prefixes via `node_prefix_strip` config, making the tool portable across clusters.

4. **Throttle/Backpressure**: `FetcherThrottle` in runtime.rs manages refresh rates, backing off on errors or when the data channel is full.

## Project Documentation

- [JSON_STRUCTURE.md](./JSON_STRUCTURE.md) - Slurm 24.11 JSON format documentation

## Testing on a Slurm Cluster

```bash
# Submit test jobs with various names/partitions
./submit_test_jobs.sh 5           # 5 mixed jobs
./submit_test_jobs.sh 3 cpu       # 3 CPU partition jobs
./submit_test_jobs.sh 2 gpu       # 2 GPU partition jobs

# Manual job submission
sbatch -J "test_name" ~/sleep.sh
```

The Name column dynamically adjusts width based on the longest job name in the display.
