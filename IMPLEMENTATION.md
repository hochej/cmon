# cmon - Modern Slurm Cluster Monitor

## Overview
A completely reimagined cluster monitoring tool for Slurm 24.11, built with Python 3.13. No backward compatibility constraints - focused entirely on delivering the best possible user experience with modern CLI design.

## Key Technologies
- **Python 3.13**: Latest Python version
- **uv**: Modern Python package manager
- **Typer**: CLI framework with type hints
- **Pydantic**: Data validation and settings management
- **Rich**: Terminal formatting and colors
- **Slurm 24.11**: Native JSON output support

## Project Structure
```
cmon/
├── pyproject.toml         # uv project configuration
├── src/
│   └── cmon/
│       ├── __init__.py
│       ├── __main__.py    # Entry point
│       ├── cli.py         # Typer CLI interface
│       ├── models.py      # Pydantic JSON models
│       ├── slurm.py       # Slurm JSON API wrapper
│       ├── display.py     # Rich table formatting
│       ├── analysis.py    # Node flagging logic
│       ├── config.py      # Settings management
│       └── utils.py       # Helper functions
├── tests/
│   ├── test_models.py
│   ├── test_slurm.py
│   └── test_analysis.py
└── README.md
```

## Module Descriptions

### 1. Project Setup (pyproject.toml)
```toml
[project]
name = "cmon"
version = "1.0.0"
requires-python = ">=3.13"
dependencies = [
    "typer[all]",  # Includes rich
    "pydantic>=2.0",
    "python-hostlist",  # Optional, can use scontrol
]
```

### 2. Data Models (models.py)
Define Pydantic models matching Slurm JSON structure:

```python
class NodeInfo(BaseModel):
    """Model for node information from sinfo --json"""
    name: str
    state: list[str]
    partition_name: str
    cpus_allocated: int
    cpus_idle: int
    cpus_total: int
    cpu_load: float
    memory_total: int
    memory_free: int
    memory_allocated: int
    gres_total: str
    gres_used: str
    
class JobInfo(BaseModel):
    """Model for job information from squeue --json"""
    job_id: int
    name: str
    user_name: str
    group_name: str
    account: str
    partition: str
    job_state: list[str]
    nodes: str
    tres_alloc_str: str
    start_time: datetime
    end_time: Optional[datetime]
    
class PestatConfig(BaseModel):
    """Configuration settings"""
    cpuload_delta1: float = 4.0
    cpuload_delta2: float = 2.0
    memory_thres1: float = 0.1
    memory_thres2: float = 0.2
    job_grace_time: int = 300
    omit_states: list[str] = ["down", "down~", "idle~", "drain", "drng"]
    colors_enabled: bool = True
```

### 3. Slurm Interface (slurm.py)
Wrapper for Slurm commands with JSON parsing:

```python
def get_nodes(partition: Optional[str] = None, 
              nodelist: Optional[str] = None,
              states: Optional[list[str]] = None) -> list[NodeInfo]:
    """Get node information from sinfo --json"""
    
def get_jobs(users: Optional[list[str]] = None,
             accounts: Optional[list[str]] = None,
             partitions: Optional[list[str]] = None) -> list[JobInfo]:
    """Get job information from squeue --json"""
    
def expand_hostlist(hostlist: str) -> list[str]:
    """Expand hostlist using scontrol show hostnames"""
```

### 4. Display Module (display.py)
Rich table formatting with color support:

```python
def create_node_table(nodes: list[NodeInfo], 
                     jobs: dict[str, list[JobInfo]],
                     config: PestatConfig) -> Table:
    """Create Rich table with colored output"""
    
def format_cpu_usage(allocated: int, total: int) -> str:
    """Format CPU usage as 'used/total'"""
    
def get_state_color(state: str) -> str:
    """Determine color based on node state"""
    
def flag_node(node: NodeInfo, jobs: list[JobInfo], 
              config: PestatConfig) -> tuple[str, str]:
    """Determine if node should be flagged and why"""
```

### 5. CLI Interface (cli.py)
Modern Typer application with intuitive commands and options:

```python
app = typer.Typer(
    name="pstat",
    help="Modern Slurm cluster monitoring tool",
    rich_markup_mode="rich"
)

# Main command - show cluster status
@app.command()
def show(
    # Filtering options
    partition: Optional[str] = typer.Option(None, "--partition", "-p", 
        help="Filter by partition name"),
    user: Optional[str] = typer.Option(None, "--user", "-u", 
        help="Show only jobs for this user"),
    group: Optional[str] = typer.Option(None, "--group", "-g", 
        help="Show only jobs for users in this UNIX group"),
    account: Optional[str] = typer.Option(None, "--account", "-a", 
        help="Filter by Slurm account"),
    nodes: Optional[str] = typer.Option(None, "--nodes", "-n", 
        help="Show only these nodes (supports ranges)"),
    
    # Display options
    format: str = typer.Option("table", "--format", "-f",
        help="Output format: table, json, csv, compact"),
    details: bool = typer.Option(False, "--details", "-d",
        help="Show detailed information including jobs"),
    gpus: bool = typer.Option(False, "--gpus", 
        help="Show GPU allocation and availability"),
    issues: bool = typer.Option(False, "--issues", "-i",
        help="Show only nodes with issues/warnings"),
    
    # Sorting and grouping
    sort: str = typer.Option("node", "--sort", "-s",
        help="Sort by: node, cpu, memory, gpu, jobs, load"),
    group_by: str = typer.Option(None, "--group-by",
        help="Group by: partition, state, user"),
    
    # Other options
    watch: Optional[int] = typer.Option(None, "--watch", "-w",
        help="Auto-refresh every N seconds"),
    no_color: bool = typer.Option(False, "--no-color",
        help="Disable colored output"),
    export: Optional[str] = typer.Option(None, "--export",
        help="Export to file (format based on extension)"),
):
    """Show current cluster status with smart filtering and formatting"""

# Subcommand for job analysis
@app.command()
def jobs(
    user: Optional[str] = typer.Option(None, "--user", "-u"),
    running: bool = typer.Option(True, "--running/--all"),
    sort: str = typer.Option("start_time", "--sort", "-s",
        help="Sort by: id, user, time, nodes, gpu"),
    limit: int = typer.Option(None, "--limit", "-l",
        help="Show only top N jobs"),
):
    """Analyze running jobs and resource usage"""

# Subcommand for node analysis
@app.command()
def nodes(
    state: Optional[str] = typer.Option(None, "--state", "-s",
        help="Filter by state: idle, mixed, allocated, down"),
    min_free_cpu: Optional[int] = typer.Option(None, "--min-cpu"),
    min_free_mem: Optional[int] = typer.Option(None, "--min-mem"),
    min_free_gpu: Optional[int] = typer.Option(None, "--min-gpu"),
    efficiency: bool = typer.Option(False, "--efficiency", "-e",
        help="Show resource efficiency metrics"),
):
    """Analyze node availability and efficiency"""

# Subcommand for user statistics
@app.command()
def stats(
    user: Optional[str] = typer.Option(None, "--user", "-u"),
    group: Optional[str] = typer.Option(None, "--group", "-g"),
    period: str = typer.Option("today", "--period", "-p",
        help="Time period: today, week, month"),
    top: int = typer.Option(10, "--top",
        help="Show top N users"),
):
    """Show usage statistics and trends"""

# Interactive dashboard
@app.command()
def dashboard():
    """Launch interactive terminal dashboard (using Textual)"""
```

### 6. Node Analysis (analysis.py)
Logic for flagging nodes based on resource usage:

```python
def calculate_ideal_load(allocated_cores: int, threads_per_core: int) -> tuple[float, float]:
    """Calculate ideal CPU load range based on allocated cores"""
    
def check_cpu_anomaly(node: NodeInfo, config: PestatConfig) -> Optional[str]:
    """Check for CPU load anomalies"""
    
def check_memory_pressure(node: NodeInfo, config: PestatConfig) -> Optional[str]:
    """Check for memory pressure"""
    
def check_job_issues(node: NodeInfo, jobs: list[JobInfo]) -> Optional[str]:
    """Check for job-related issues"""
    
def should_flag_node(node: NodeInfo, jobs: list[JobInfo], 
                    config: PestatConfig) -> tuple[bool, str, str]:
    """Determine if node should be flagged and severity"""
```

### 7. Configuration Management (config.py)
Handle configuration files and environment variables:

```python
def load_config() -> PestatConfig:
    """Load configuration from files and environment"""
    # Priority order:
    # 1. Command-line arguments
    # 2. Environment variables
    # 3. ~/.pestat.conf
    # 4. /etc/pestat.conf
    # 5. Default values
    
def parse_config_file(filepath: Path) -> dict:
    """Parse shell-style configuration file"""
```

### 8. Utilities (utils.py)
Helper functions:

```python
def parse_time_string(time_str: str) -> int:
    """Convert Slurm time format to seconds"""
    
def format_memory(memory_mb: int) -> str:
    """Format memory for display"""
    
def expand_node_range(node_expr: str) -> list[str]:
    """Expand node range expressions"""
```

## Implementation Phases

### Phase 1: Core Functionality
1. Set up project structure with uv
2. Implement basic Slurm JSON parsing
3. Create minimal CLI with node listing
4. Basic Rich table output

### Phase 2: Feature Parity
1. Add all command-line options
2. Implement node flagging logic
3. Add job correlation
4. Color coding for states

### Phase 3: Configuration
1. Configuration file parsing
2. Environment variable support
3. User preferences

### Phase 4: Polish
1. Performance optimizations
2. Error handling
3. Documentation
4. Unit tests

## Testing Strategy

### Unit Tests
- Model validation
- Flag logic calculations
- Time parsing functions
- Configuration loading

### Integration Tests
- Slurm command execution
- JSON parsing
- Output formatting

### Validation Tests
- Compare with original pestat output
- Verify all flags work correctly
- Check edge cases

## Key Features of Modern Design

### User Experience First
1. **Intuitive Commands**: Subcommands for different use cases (show, jobs, nodes, stats)
2. **Smart Defaults**: Sensible defaults that show what users typically want
3. **Progressive Disclosure**: Basic info by default, --details for more
4. **Rich Output**: Beautiful tables, colors, and formatting
5. **Interactive Mode**: Optional dashboard for real-time monitoring

### Modern CLI Patterns
1. **Descriptive Options**: --partition instead of -p, with short aliases
2. **Multiple Output Formats**: table, json, csv for different workflows
3. **Watch Mode**: Built-in auto-refresh instead of using external watch
4. **Export Capability**: Save results to files in various formats
5. **Grouping and Sorting**: Flexible data organization

### Technical Advantages
1. **JSON Native**: Direct JSON parsing from Slurm 24.11
2. **Type Safety**: Full Pydantic validation
3. **Async Support**: Optional async for better performance
4. **Extensible**: Easy to add new features and formats
5. **Testable**: Clean architecture for unit testing

## Performance Considerations

1. **Single Data Fetch**: One call each to sinfo and squeue
2. **Efficient Parsing**: JSON parsing is faster than text regex
3. **Dictionary Lookups**: O(1) job-to-node mapping
4. **Optional Async**: Can add async for parallel operations if needed

## Example Usage Patterns

### Basic Usage
```bash
# Show cluster overview
pstat

# Show only GPU nodes
pstat --gpus

# Show nodes with issues
pstat --issues

# Watch cluster status
pstat --watch 5
```

### Advanced Filtering
```bash
# Show specific user's jobs
pstat --user username --details

# Find available GPU nodes
pstat nodes --min-gpu 1 --state idle

# Show top resource users
pstat stats --top 20

# Export to JSON for automation
pstat --format json --export cluster-status.json
```

### Example Output
```
┏━━━━━━━━━━━━━━┳━━━━━━━━━┳━━━━━━━━━━━┳━━━━━━━━━━━━┳━━━━━━━━━━━┳━━━━━━━━━━━┓
┃ Node         ┃ State   ┃ CPU       ┃ Memory     ┃ GPU       ┃ Jobs      ┃
┡━━━━━━━━━━━━━━╇━━━━━━━━━╇━━━━━━━━━━━╇━━━━━━━━━━━━╇━━━━━━━━━━━╇━━━━━━━━━━━┩
│ node001      │ ● mixed │ 48/128    │ 384G/512G  │ 2/4 L40S  │ 3 jobs    │
│ node002      │ ● idle  │ 0/128     │ 8G/512G    │ 0/4 L40S  │ -         │
│ node003      │ ● alloc │ 128/128 ⚠ │ 500G/512G  │ 4/4 L40S  │ 8 jobs    │
│ node004      │ ○ down  │ -         │ -          │ -         │ -         │
└──────────────┴─────────┴───────────┴────────────┴───────────┴───────────┘
● Active  ○ Inactive  ⚠ Warning
```

## Future Enhancements

1. **Interactive Mode**: Using Textual for TUI
2. **Export Formats**: JSON, CSV output options
3. **Metrics Collection**: Prometheus export
4. **Watch Mode**: Auto-refresh display
5. **Historical Tracking**: Store and analyze trends