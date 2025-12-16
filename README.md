<div align="center">

![cmon Logo](./public/cmon_logo.svg)

# cmon

[![Build and Release](https://github.com/hochej/cmon/actions/workflows/build-release.yml/badge.svg)](https://github.com/hochej/cmon/actions/workflows/build-release.yml)

**Slurm cluster monitoring for humans**

</div>

## Why cmon?

Skip the `squeue -o` format strings and `sinfo` flag combinations. cmon gives you:
- Filtered job views, sorting, and search without remembering flags
- Interactive TUI with real-time updates and job management
- Export to JSON/CSV for scripting and analysis

<!-- TODO: Add GIF demo here -->

## Installation

Download the latest binary from [Releases](https://github.com/hochej/cmon/releases).

<details>
<summary>Build from source</summary>

```bash
cargo build --release
```

</details>

## Usage

| Command | Description |
|---------|-------------|
| `cmon` | Cluster overview (partitions + nodes) |
| `cmon jobs` | Running jobs |
| `cmon jobs --all` | All jobs (including pending) |
| `cmon tui` | Interactive TUI |
| `cmon me` | Your jobs and stats |
| `cmon down` | Problematic nodes |

All commands support `--watch <seconds>` for continuous refresh.

## TUI Keybindings

| Key | Action |
|-----|--------|
| `1-5` | Switch view (Jobs/Nodes/Partitions/Personal/Problems) |
| `j/k` or arrows | Navigate |
| `/` | Filter |
| `s` | Sort |
| `c` | Cancel job |
| `y` | Copy job ID |
| `?` | Help |
| `q` | Quit |

## Configuration

Works without configuration. Optional: `~/.config/cmon/config.toml`

```bash
cmon init-config  # Generate template with documentation
```

## Requirements

- Slurm 21.08+ (JSON output support)

## License

MIT
