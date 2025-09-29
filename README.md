<div align="center">

![cmon Logo](./public/cmon_logo.svg)

# cmon

[![Build and Release](https://github.com/merckgroup/cmon/actions/workflows/build-release.yml/badge.svg)](https://github.com/merckgroup/cmon/actions/workflows/build-release.yml)

**Cluster monitoring tool for Slurm**

[`cmon`](https://github.com/merckgroup/cmon) provides an enhanced view of cluster jobs and nodes

</div>

## Performance

| Command | Rust | Python | Speedup |
|---------|------|--------|---------|
| `cmon jobs` | **8ms** | 263ms | **32x** |
| `cmon nodes` | **33ms** | 279ms | **8x** |
| `cmon status` | **42ms** | 336ms | **8x** |

## Installation

### From Binary (Recommended)

```bash
# Download the latest RHEL 9 compatible binary
curl -L https://github.com/merckgroup/cmon/releases/latest/download/cmon-linux-x86_64 -o cmon
chmod +x cmon

# Test it works
./cmon --version

# Install to your PATH
sudo mv cmon /usr/local/bin/
# or
mv cmon ~/bin/  # if ~/bin is in your PATH
```

### From Source

Requires Rust 1.70+:

```bash
git clone https://github.com/merckgroup/cmon.git
cd cmon
cargo build --release
sudo cp target/release/cmon /usr/local/bin/
```

## Usage

### Basic Commands

```bash
# View cluster status (default)
cmon
cmon status

# View running jobs
cmon jobs

# View all jobs (including pending, completed, etc.)
cmon jobs --all

# View nodes
cmon nodes
```

### Filtering

```bash
# Filter jobs by state
cmon jobs --state PENDING
cmon jobs --state RUNNING,COMPLETING
cmon jobs --state FAILED,TIMEOUT

# Filter nodes by state
cmon nodes --state IDLE
cmon nodes --state DRAINING,DOWN

# Filter by user
cmon jobs --user $USER

# Filter by partition
cmon jobs --partition gpu
cmon nodes --partition fat
```

### Watch Mode

Auto-refresh display every N seconds:

```bash
# Refresh every 5 seconds (Ctrl+C to exit)
cmon status --watch 5
cmon jobs --watch 10
cmon nodes --state DRAINING --watch 3
```

## Output Examples

### Cluster Status

```
╭─────────────────────────────── Cluster Status ───────────────────────────────╮
│                                                                              │
│  Cluster Overview (as of 14:30:45)                                          │
│                                                                              │
│  Nodes: 107 total • 45 idle • 12 mixed • 35 allocated • 15 down             │
│  CPUs: 8542/13696 cores (62.4% utilized)                                    │
│  Jobs: 89 running                                                           │
│  GPUs: 156/176 (88.6% utilized)                                             │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
```

### Jobs Table

```
╭───────┬──────────┬──────┬───────────┬─────────┬────────────┬───────┬──────┬──────┬──────╮
│ JobID │   Name   │ User │ Partition │  State  │   Reason   │ Nodes │ CPUs │ GPUs │ Time │
├───────┼──────────┼──────┼───────────┼─────────┼────────────┼───────┼──────┼──────┼──────┤
│ 82894 │ train_ml │ user │ gpu       │ RUNNING │ None       │ gpu01 │  16  │ 2xL40│ 2h   │
│ 82901 │ infer_lg │ user │ gpu       │ PENDING │ Resources  │   -   │  32  │ 4xL40│  -   │
╰───────┴──────────┴──────┴───────────┴─────────┴────────────┴───────┴──────┴──────┴──────╯
```

### Nodes Table with Reasons

```
╭────────┬────────────┬────────────────────────────────────────────────┬─────────┬──────────────┬──────────╮
│  Node  │   State    │                     Reason                     │   CPU   │    Memory    │   GPU    │
├────────┼────────────┼────────────────────────────────────────────────┼─────────┼──────────────┼──────────┤
│ cpu001 │ ● MIXED    │ -                                              │ 64/128  │ 512G/1.5T    │ -        │
│ cpu002 │ ● IDLE     │ -                                              │ 0/128   │ 55G/1.5T     │ -        │
│ cpu003 │ ◐ DRAINING │ NHC: check_fs_mount:  /shared/apps not mounted │ 0/128   │ 9.8G/1.5T    │ -        │
│ gpu001 │ ● MIXED    │ -                                              │ 113/128 │ 40.2G/1.5T   │ 3/4 l40s │
╰────────┴────────────┴────────────────────────────────────────────────┴─────────┴──────────────┴──────────╯
```

### State Symbols

- `●` (green) - Available (IDLE)
- `●` (yellow) - Partially used (MIXED/ALLOCATED)
- `◐` (yellow) - Maintenance (DRAINING/MAINT)
- `○` (red) - Problem (DOWN/FAIL)

## Requirements

- **Binary**: RHEL 9 compatible Linux (glibc 2.34+, x86_64)
- **Source**: Rust 1.70+
- **Cluster**: Slurm 24.11+ (for full JSON support)
- **Access**: `squeue`, `sinfo` commands must be available

## Development

### Build

```bash
# Development build (fast compile)
cargo build

# Release build (optimized)
cargo build --release

# Run without building
cargo run -- status
```

### Testing

```bash
# Run all tests (36 tests)
cargo test

# Run benchmarks
cargo bench --bench cmon_bench

# Check code
cargo clippy
cargo fmt
```
## License

MIT License - see LICENSE file for details
