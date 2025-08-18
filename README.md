# cmon

Modern Slurm cluster monitoring tool with enhanced job and node visualization.

## Installation

### From Binary (Recommended for HPC)
```bash
# Download the latest RHEL 9 compatible binary
curl -L https://github.com/your-org/cmon/releases/latest/download/cmon-linux-x86_64 -o cmon
chmod +x cmon

# Test it works
./cmon --version

# Install to your PATH
mv cmon ~/bin/ # or any directory in your PATH
```

### From Source
```bash
# Clone and install with uv
git clone https://github.com/your-org/cmon.git
cd cmon
uv sync
uv run cmon
```

## Usage

```bash
# View running jobs
cmon jobs

# View all jobs (including pending, completing)
cmon jobs --all

# View cluster status
cmon status

# View node information
cmon nodes
```

## Requirements

- **Binary**: RHEL 9 compatible Linux (built on AlmaLinux 9)
- **Source**: Python 3.13+
- Slurm 24.11+ (for full JSON support)
- Access to `squeue`, `sinfo`, and `scontrol` commands

## Binary Compatibility

The pre-built binary is automatically built using GitHub Actions on AlmaLinux 9, ensuring compatibility with:
- RHEL 9 / CentOS Stream 9 / Rocky Linux 9 / AlmaLinux 9
- Any Linux distribution with glibc 2.34+
- x86_64 architecture

## Development

```bash
# Install dependencies
uv sync

# Run locally
uv run cmon

# Build binary for distribution
./build_binary.sh
```