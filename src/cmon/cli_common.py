"""Common CLI options and utilities for cmon."""

from typing import Optional
import typer
from rich.console import Console


console = Console()


# Reusable option definitions for consistency across commands
def partition_option():
    """Reusable partition filter option."""
    return typer.Option(
        None, "--partition", "-p", 
        help="Filter by partition name"
    )


def user_option():
    """Reusable user filter option."""
    return typer.Option(
        None, "--user", "-u", 
        help="Filter by username"
    )


def state_option():
    """Reusable state filter option."""
    return typer.Option(
        None, "--state", "-s",
        help="Filter by node state: idle, mixed, allocated, down, drain"
    )


def format_option():
    """Reusable format option."""
    return typer.Option(
        "table", "--format", "-f",
        help="Output format: table, json, csv"
    )


def export_option():
    """Reusable export option."""
    return typer.Option(
        None, "--export", "-e",
        help="Export to file (format based on extension: .json, .csv)"
    )


def watch_option():
    """Reusable watch option."""
    return typer.Option(
        None, "--watch", "-w",
        help="Auto-refresh every N seconds"
    )


def no_color_option():
    """Reusable no-color option."""
    return typer.Option(
        False, "--no-color",
        help="Disable colored output"
    )


# View levels for consistent display control
VIEW_LEVELS = {
    "compact": {
        "show_summary": False,
        "show_partitions": False,
        "show_details": False,
        "show_gpus": False
    },
    "standard": {
        "show_summary": True,
        "show_partitions": True,
        "show_details": False,
        "show_gpus": True
    },
    "detailed": {
        "show_summary": True,
        "show_partitions": True,
        "show_details": True,
        "show_gpus": True
    }
}


def view_option():
    """Standardized view level option."""
    return typer.Option(
        "standard", "--view", "-v",
        help="Display level: compact, standard, detailed"
    )


def issues_flag():
    """Reusable issues-only flag."""
    return typer.Option(
        False, "--issues", "-i",
        help="Show only nodes with issues/warnings"
    )


def available_flag():
    """Reusable available resources flag."""
    return typer.Option(
        False, "--available", "-a",
        help="Show only available/idle resources"
    )


# Common validation and setup functions
def setup_console(no_color: bool) -> Console:
    """Setup console with color preferences."""
    if no_color:
        return Console(color_system=None, force_terminal=False)
    return Console()


def validate_view_level(view: str) -> dict:
    """Validate and return view configuration."""
    if view not in VIEW_LEVELS:
        console.print(f"[red]Error:[/red] Invalid view level '{view}'. Must be one of: {', '.join(VIEW_LEVELS.keys())}")
        raise typer.Exit(1)
    return VIEW_LEVELS[view]


def validate_watch_interval(watch: Optional[int]) -> None:
    """Validate watch interval."""
    if watch and watch < 1:
        console.print("[red]Error:[/red] Watch interval must be at least 1 second")
        raise typer.Exit(1)


def handle_watch_mode(watch: Optional[int], fetch_function):
    """Handle watch mode execution."""
    if not watch:
        fetch_function()
        return
    
    validate_watch_interval(watch)
    
    import time
    try:
        while True:
            console.clear()
            fetch_function()
            console.print(f"\n[dim]Refreshing every {watch}s... Press Ctrl+C to stop[/dim]")
            time.sleep(watch)
    except KeyboardInterrupt:
        console.print("\n[yellow]Watch mode stopped[/yellow]")
        raise typer.Exit(0)


# Common help examples
EXAMPLES = {
    "status": [
        "cmon status                     # Standard cluster overview",
        "cmon status --view compact      # Minimal info, no panels",
        "cmon status --issues            # Show only problematic nodes",
        "cmon status --available         # Show only available resources",
        "cmon status --user john         # Show specific user's activity",
        "cmon status --watch 5           # Auto-refresh every 5 seconds"
    ],
    "jobs": [
        "cmon jobs                       # Show all running jobs",
        "cmon jobs --user john           # Show jobs for user 'john'",
        "cmon jobs --limit 10            # Show top 10 jobs",
        "cmon jobs --sort gpu            # Sort by GPU allocation"
    ],
    "nodes": [
        "cmon nodes --state idle         # Show available nodes",
        "cmon nodes --min-gpu 2          # Show nodes with 2+ free GPUs",
        "cmon nodes --efficiency         # Show efficiency metrics"
    ]
}


def format_examples(command: str) -> str:
    """Format help examples for a command."""
    if command not in EXAMPLES:
        return ""
    
    examples = "\n".join(f"        {ex}" for ex in EXAMPLES[command])
    return f"\n    Examples:\n{examples}"