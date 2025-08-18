"""Modern CLI interface using Typer for cmon."""

import json
import sys
from typing import Optional, List
from pathlib import Path
from datetime import datetime

import typer
from rich.console import Console
from rich.panel import Panel
from rich.text import Text

from . import __version__
from .slurm import slurm, SlurmError
from .display import formatter, get_available_palettes, create_formatter_with_palette
from .models import ClusterStatus
from .cli_common import (
    partition_option, user_option, state_option, format_option, export_option,
    watch_option, no_color_option, view_option, issues_flag, available_flag,
    setup_console, validate_view_level, handle_watch_mode, format_examples
)

# Create the main Typer app
app = typer.Typer(
    name="cmon",
    help="Modern Slurm cluster monitoring tool - simplified and intuitive",
    rich_markup_mode="rich",
    add_completion=False,
    context_settings={"help_option_names": ["-h", "--help"]}
)

# Console for output
console = Console()


def handle_slurm_error(error: SlurmError):
    """Handle Slurm errors with nice formatting."""
    console.print(f"[red]Error:[/red] {error}", file=sys.stderr)
    raise typer.Exit(1)


def validate_slurm_connection():
    """Validate that Slurm is accessible."""
    if not slurm.test_connection():
        console.print(
            "[red]Error:[/red] Cannot connect to Slurm. Please check that Slurm is installed and accessible.",
            file=sys.stderr
        )
        raise typer.Exit(1)


@app.command(name="status", help="Show current cluster status with smart filtering and formatting")
def show_status(
    # Core filtering options
    partition: Optional[str] = partition_option(),
    user: Optional[str] = user_option(),
    state: Optional[str] = state_option(),
    nodes: Optional[str] = typer.Option(
        None, "--nodes", "-n", 
        help="Show only these nodes (supports ranges like node[001-010])"
    ),
    
    # Quick filters
    issues: bool = issues_flag(),
    available: bool = available_flag(),
    
    # Display control
    view: str = view_option(),
    
    # Output options  
    format: str = format_option(),
    export: Optional[str] = export_option(),
    watch: Optional[int] = watch_option(),
    no_color: bool = no_color_option(),
):
    """Show current cluster status with smart filtering and formatting.
    {}
    """.format(format_examples("status"))
    # Validate Slurm connection
    validate_slurm_connection()
    
    # Set up console with color preference
    display_console = setup_console(no_color)
    
    # Validate and get view configuration
    view_config = validate_view_level(view)
    
    def fetch_and_display():
        """Fetch data and display it."""
        try:
            # Prepare state filter
            state_filter = None
            if state:
                state_filter = [s.strip().upper() for s in state.split(",")]
            
            # Get cluster status
            cluster = slurm.get_cluster_status(
                partition=partition,
                user=user,
                nodelist=nodes
            )
            
            # Apply filters
            filtered_nodes = cluster.nodes
            
            if state_filter:
                filtered_nodes = [n for n in filtered_nodes 
                                if any(s in n.state for s in state_filter)]
            
            if issues:
                # Filter for nodes with issues (down, drain, high load, low memory, etc.)
                filtered_nodes = [n for n in filtered_nodes 
                                if n.is_down or n.is_draining or 
                                   (hasattr(n, 'has_issues') and n.has_issues)]
            
            if available:
                # Filter for available/idle nodes
                filtered_nodes = [n for n in filtered_nodes if n.is_idle or n.is_mixed]
            
            # Create filtered cluster
            filtered_cluster = ClusterStatus(nodes=filtered_nodes, jobs=cluster.jobs)
            
            # Handle different output formats
            if format.lower() == "json":
                output_data = {
                    "timestamp": filtered_cluster.timestamp.isoformat(),
                    "nodes": [node.model_dump() for node in filtered_cluster.nodes],
                    "jobs": [job.model_dump() for job in filtered_cluster.jobs]
                }
                
                json_str = json.dumps(output_data, indent=2, default=str)
                
                if export:
                    Path(export).write_text(json_str)
                    display_console.print(f"[green]Exported to {export}[/green]")
                else:
                    display_console.print(json_str)
                    
            elif format.lower() == "csv":
                import csv
                import io
                
                output = io.StringIO()
                writer = csv.writer(output)
                
                writer.writerow([
                    "Node", "State", "CPUs_Allocated", "CPUs_Total", 
                    "Memory_Used_MB", "Memory_Total_MB", "GPU_Used", "GPU_Total", "Jobs"
                ])
                
                for node in filtered_cluster.nodes:
                    gpu_info = node.gpu_info
                    node_jobs = [j for j in filtered_cluster.jobs if node.name in j.nodes]
                    
                    writer.writerow([
                        node.name,
                        "|".join(node.state),
                        node.cpus_allocated,
                        node.cpus_total,
                        node.memory_total - node.memory_free,
                        node.memory_total,
                        gpu_info["used"],
                        gpu_info["total"],
                        len(node_jobs)
                    ])
                
                csv_str = output.getvalue()
                output.close()
                
                if export:
                    Path(export).write_text(csv_str)
                    display_console.print(f"[green]Exported to {export}[/green]")
                else:
                    display_console.print(csv_str)
                    
            else:
                # Default table format using view configuration
                formatter.print_cluster_status(
                    cluster=filtered_cluster,
                    show_summary=view_config["show_summary"],
                    show_partitions=view_config["show_partitions"],
                    show_details=view_config["show_details"],
                    show_gpus=view_config["show_gpus"],
                    issues_only=issues
                )
                
                if export:
                    display_console.print(f"[yellow]Note: Export only supported for JSON and CSV formats[/yellow]")
        
        except SlurmError as e:
            handle_slurm_error(e)
        except Exception as e:
            display_console.print(f"[red]Unexpected error:[/red] {e}", file=sys.stderr)
            raise typer.Exit(1)
    
    # Handle watch mode using common utility
    handle_watch_mode(watch, fetch_and_display)


@app.command(name="jobs", help="Analyze running jobs and resource usage")
def show_jobs(
    # Core filtering
    user: Optional[str] = user_option(),
    partition: Optional[str] = partition_option(),
    
    # Job-specific options
    running: bool = typer.Option(
        True, "--running/--all",
        help="Show only running jobs (default) or all jobs"
    ),
    sort: str = typer.Option(
        "start_time", "--sort", "-s",
        help="Sort by: id, user, start_time, nodes, gpu"
    ),
    limit: Optional[int] = typer.Option(
        None, "--limit", "-l",
        help="Show only top N jobs"
    ),
    
    # Display options (NEW)
    compact: bool = typer.Option(
        False, "--compact", "-c",
        help="Use compact layout for narrow terminals"
    ),
    full_state: bool = typer.Option(
        False, "--full-state",
        help="Show full state names (RUNNING vs R)"
    ),
    columns: Optional[str] = typer.Option(
        None, "--columns",
        help="Comma-separated list of columns to show"
    ),
    color_palette: str = typer.Option(
        "standard", "--palette",
        help=f"Color palette: {', '.join(get_available_palettes())}"
    ),
    
    # Output options
    format: str = format_option(),
    export: Optional[str] = export_option(),
    watch: Optional[int] = watch_option(),
    no_color: bool = no_color_option(),
):
    """Analyze running jobs and resource usage.
    {}
    """.format(format_examples("jobs"))
    validate_slurm_connection()
    
    # Set up console
    display_console = setup_console(no_color)
    
    def fetch_and_display():
        """Fetch job data and display it."""
        try:
            # Determine job states to query
            states = ["RUNNING"] if running else None
            
            # Get jobs
            jobs = slurm.get_jobs(
                users=[user] if user else None,
                partitions=[partition] if partition else None,
                states=states
            )
            
            # Sort jobs
            if sort == "id":
                jobs.sort(key=lambda j: j.job_id)
            elif sort == "user":
                jobs.sort(key=lambda j: j.user_name)
            elif sort == "nodes":
                jobs.sort(key=lambda j: len(j.nodes.split(",")))
            elif sort == "gpu":
                jobs.sort(key=lambda j: j.allocated_gpus, reverse=True)
            else:  # start_time
                jobs.sort(key=lambda j: j.start_time or datetime.min, reverse=True)
            
            # Handle output format
            if format.lower() == "json":
                output_data = [job.model_dump() for job in jobs]
                json_str = json.dumps(output_data, indent=2, default=str)
                
                if export:
                    Path(export).write_text(json_str)
                    display_console.print(f"[green]Exported to {export}[/green]")
                else:
                    display_console.print(json_str)
            else:
                # Parse columns if specified
                show_columns = None
                if columns:
                    show_columns = [col.strip() for col in columns.split(",")]
                
                # Create formatter with chosen palette (unless using no-color)
                if no_color:
                    display_formatter = formatter  # Use default with no color
                else:
                    display_formatter = create_formatter_with_palette(color_palette)
                
                display_formatter.print_jobs(
                    jobs, 
                    limit=limit,
                    compact=compact,
                    show_columns=show_columns,
                    use_short_state=not full_state
                )
                
                if export:
                    display_console.print(f"[yellow]Note: Export only supported for JSON format[/yellow]")
        
        except SlurmError as e:
            handle_slurm_error(e)
        except Exception as e:
            display_console.print(f"[red]Unexpected error:[/red] {e}", file=sys.stderr)
            raise typer.Exit(1)
    
    # Handle watch mode
    handle_watch_mode(watch, fetch_and_display)


@app.command(name="nodes", help="Analyze node availability and efficiency")
def show_nodes(
    # Core filtering
    state: Optional[str] = state_option(),
    partition: Optional[str] = partition_option(),
    
    # Resource filtering
    min_free_cpu: Optional[int] = typer.Option(
        None, "--min-cpu",
        help="Show nodes with at least N free CPU cores"
    ),
    min_free_mem: Optional[int] = typer.Option(
        None, "--min-mem",
        help="Show nodes with at least N GB free memory"
    ),
    min_free_gpu: Optional[int] = typer.Option(
        None, "--min-gpu",
        help="Show nodes with at least N free GPUs"
    ),
    
    # Display options
    efficiency: bool = typer.Option(
        False, "--efficiency",
        help="Show resource efficiency metrics"
    ),
    
    # Output options
    format: str = format_option(),
    export: Optional[str] = export_option(),
    watch: Optional[int] = watch_option(),
    no_color: bool = no_color_option(),
):
    """Analyze node availability and efficiency.
    {}
    """.format(format_examples("nodes"))
    validate_slurm_connection()
    
    # Set up console
    display_console = setup_console(no_color)
    
    def fetch_and_display():
        """Fetch node data and display it."""
        try:
            # Get cluster status
            cluster = slurm.get_cluster_status(partition=partition)
            
            # Apply filters
            filtered_nodes = cluster.nodes
            
            if state:
                state_upper = state.upper()
                if state_upper == "IDLE":
                    filtered_nodes = [n for n in filtered_nodes if n.is_idle]
                elif state_upper == "MIXED":
                    filtered_nodes = [n for n in filtered_nodes if n.is_mixed]
                elif state_upper == "ALLOCATED":
                    filtered_nodes = [n for n in filtered_nodes if "ALLOCATED" in n.state]
                elif state_upper == "DOWN":
                    filtered_nodes = [n for n in filtered_nodes if n.is_down]
            
            if min_free_cpu is not None:
                filtered_nodes = [n for n in filtered_nodes if n.cpus_idle >= min_free_cpu]
            
            if min_free_mem is not None:
                min_free_mb = min_free_mem * 1024  # Convert GB to MB
                filtered_nodes = [n for n in filtered_nodes if n.memory_free >= min_free_mb]
            
            if min_free_gpu is not None:
                filtered_nodes = [n for n in filtered_nodes 
                                if (n.gpu_info["total"] - n.gpu_info["used"]) >= min_free_gpu]
            
            # Handle output formats
            if format.lower() == "json":
                output_data = [node.model_dump() for node in filtered_nodes]
                json_str = json.dumps(output_data, indent=2, default=str)
                
                if export:
                    Path(export).write_text(json_str)
                    display_console.print(f"[green]Exported to {export}[/green]")
                else:
                    display_console.print(json_str)
                return
            
            # Display results in table format
            if efficiency:
                # Show efficiency metrics
                display_console.print("[bold]Node Efficiency Analysis[/bold]\n")
                
                # Deduplicate nodes by name to avoid showing nodes multiple times
                unique_nodes = {}
                for node in filtered_nodes:
                    if node.name not in unique_nodes:
                        unique_nodes[node.name] = node
                
                for node in unique_nodes.values():
                    cpu_eff = node.cpu_utilization
                    mem_eff = node.memory_utilization
                    
                    efficiency_text = f"CPU: {cpu_eff:.1f}% | Memory: {mem_eff:.1f}%"
                    
                    if cpu_eff < 50 or mem_eff < 50:
                        efficiency_color = "red"
                    elif cpu_eff < 80 or mem_eff < 80:
                        efficiency_color = "yellow"
                    else:
                        efficiency_color = "green"
                    
                    display_console.print(f"{node.name:15} [{efficiency_color}]{efficiency_text}[/{efficiency_color}]")
            else:
                # Create filtered cluster and display normally
                filtered_cluster = ClusterStatus(nodes=filtered_nodes, jobs=cluster.jobs)
                formatter.print_cluster_status(
                    cluster=filtered_cluster,
                    show_summary=False,
                    show_partitions=False,
                    show_details=True,
                    show_gpus=True
                )
            
            if export:
                display_console.print(f"[yellow]Note: Export only supported for JSON format[/yellow]")
        
        except SlurmError as e:
            handle_slurm_error(e)
        except Exception as e:
            display_console.print(f"[red]Unexpected error:[/red] {e}", file=sys.stderr)
            raise typer.Exit(1)
    
    # Handle watch mode
    handle_watch_mode(watch, fetch_and_display)


@app.command(name="version")
def show_version():
    """Show version information."""
    console.print(f"[bold blue]cmon[/bold blue] version [green]{__version__}[/green]")
    console.print("Modern Slurm cluster monitoring tool")


# Smart shortcuts and default behavior
@app.callback(invoke_without_command=True)
def main(
    ctx: typer.Context,
    version: bool = typer.Option(False, "--version", "-V", help="Show version and exit"),
    issues: bool = typer.Option(False, "--issues", help="Quick shortcut: show only problematic nodes"),
    available: bool = typer.Option(False, "--available", help="Quick shortcut: show available resources"),
    user: Optional[str] = typer.Option(None, help="Quick shortcut: show specific user's jobs and nodes"),
):
    """Modern Slurm cluster monitoring tool - simplified and intuitive.
    
    Quick usage:
        cmon                          # Standard cluster overview
        cmon --issues                 # Show only problematic nodes
        cmon --available              # Show available resources
        cmon --user john              # Show user's activity
        cmon status --help            # Full options for status command
    
    Commands:
        status      Show cluster status (default)
        jobs        Analyze running jobs
        nodes       Analyze node availability
        version     Show version information
    """
    if version:
        show_version()
        raise typer.Exit()
    
    if ctx.invoked_subcommand is None:
        # No subcommand provided - use smart shortcuts or default status
        ctx.invoke(show_status, 
                  partition=None, 
                  user=user, 
                  state=None, 
                  nodes=None,
                  issues=issues, 
                  available=available,
                  view="standard", 
                  format="table", 
                  export=None, 
                  watch=None, 
                  no_color=False)


if __name__ == "__main__":
    app()