"""Display formatting using Rich for beautiful terminal output."""

from typing import List, Dict, Optional, Any
from datetime import datetime
from rich.console import Console
from rich.table import Table
from rich.text import Text
from rich.panel import Panel
from rich import box
from rich.align import Align

from .models import NodeInfo, JobInfo, ClusterStatus


# Color palette definitions
COLOR_PALETTES = {
    "standard": {
        "primary": "blue",      # CPU partition, primary elements
        "success": "green",     # GPU partition, running state, L40S
        "warning": "yellow",    # Pending state, time warnings
        "danger": "red",        # Failed state, B200, urgent warnings
        "accent": "magenta",    # Fat partition, unknown GPUs
        "info": "cyan",         # VDI partition, array jobs
    },
    "neon": {
        "primary": "#0016ee",   # Electric blue
        "success": "#defe47",   # Bright yellow-green
        "warning": "#fe00fe",   # Bright magenta/pink
        "danger": "#7700a6",    # Dark magenta/pink
        "accent": "#fe00fe",    # Bright magenta/pink
        "info": "#00b3fe",      # Bright cyan
    },
    "cyber": {
        "primary": "#1261d1",   # Royal blue
        "success": "#08deea",   # Turquoise
        "warning": "#fd8090",   # Coral pink
        "danger": "#af43be",    # Purple
        "accent": "#af43be",    # Purple
        "info": "#c4ffff",      # Light cyan
    },
    "scifi": {
        "primary": "#44786a",   # Dark teal
        "success": "#4d9e9b",   # Teal
        "warning": "#daae6d",   # Golden tan
        "danger": "#8f704b",    # Brown
        "accent": "#8f704b",    # Brown
        "info": "#89e3f6",      # Light blue
    },
    "tech": {
        "primary": "#003062",   # Dark navy
        "success": "#0a9cf5",   # Bright blue
        "warning": "#ffccdc",   # Light pink
        "danger": "#ff184c",    # Bright red
        "accent": "#ff577d",    # Pink-red
        "info": "#0a9cf5",      # Bright blue
    },
    "default": {
        "primary": "#001eff",   # Electric blue
        "success": "#00ff9f",   # Bright green
        "warning": "#d600ff",   # Magenta
        "danger": "#bd00ff",    # Purple
        "accent": "#bd00ff",    # Purple
        "info": "#00b8ff",      # Bright blue
    }
}


class DisplayFormatter:
    """Formatter for cluster data using Rich."""
    
    def __init__(self, console: Optional[Console] = None, color_palette: str = "standard"):
        """Initialize display formatter.
        
        Args:
            console: Rich console instance (creates default if None)
            color_palette: Color palette name from COLOR_PALETTES
        """
        # Force truecolor support for modern terminals like Ghostty
        if console is None:
            console = Console(force_terminal=True, color_system='truecolor')
        self.console = console
        self.set_color_palette(color_palette)
    
    def set_color_palette(self, palette_name: str):
        """Set the color palette for all displays.
        
        Args:
            palette_name: Name of the palette from COLOR_PALETTES
        """
        if palette_name not in COLOR_PALETTES:
            palette_name = "standard"
        
        self.palette = COLOR_PALETTES[palette_name]
        self.palette_name = palette_name
    
    def format_memory(self, memory_mb: int) -> str:
        """Format memory from MB to human-readable format.
        
        Args:
            memory_mb: Memory in megabytes
            
        Returns:
            Formatted memory string (e.g., "4.5G", "512M")
        """
        if memory_mb >= 1024 * 1024:  # TB
            return f"{memory_mb / (1024 * 1024):.1f}T"
        elif memory_mb >= 1024:  # GB
            return f"{memory_mb / 1024:.1f}G"
        else:  # MB
            return f"{memory_mb}M"
    
    def format_cpu_usage(self, allocated: int, total: int) -> str:
        """Format CPU usage as allocated/total.
        
        Args:
            allocated: Allocated CPU cores
            total: Total CPU cores
            
        Returns:
            Formatted CPU string (e.g., "48/128")
        """
        return f"{allocated}/{total}"
    
    def get_state_style(self, node: NodeInfo) -> tuple[str, str]:
        """Get color and symbol for node state.
        
        Args:
            node: Node information
            
        Returns:
            Tuple of (color, symbol)
        """
        if node.is_down:
            return "red", "○"  # Down/offline
        elif node.is_idle:
            # Check if idle but draining
            if any(state in ["DRAIN", "DRNG"] for state in node.state):
                return "yellow", "○"  # Idle but draining (not accepting new jobs)
            else:
                return "green", "●"  # Active/available
        elif node.is_mixed:
            return "yellow", "●"  # Partially used
        else:
            return "blue", "●"  # Allocated
    
    def get_warning_indicators(self, node: NodeInfo, jobs: List[JobInfo]) -> List[str]:
        """Get warning indicators for a node.
        
        Args:
            node: Node information
            jobs: Jobs running on this node
            
        Returns:
            List of warning indicators
        """
        warnings = []
        
        # CPU load warnings
        ideal_load_max = node.cpus_allocated if node.threads_per_core == 1 else node.cpus_allocated
        if node.cpu_load > ideal_load_max + 4.0:
            warnings.append("⚠")  # High CPU load
        
        # Memory warnings
        if node.memory_utilization > 90:
            warnings.append("⚠")  # High memory usage
        
        # Too many jobs warnings
        if len(jobs) > node.cpus_allocated:
            warnings.append("⚠")  # Oversubscribed
        
        return warnings
    
    def create_utilization_bar(self, utilization: float, width: int = 20) -> str:
        """Create a visual bar chart for utilization percentage.
        
        Args:
            utilization: Percentage (0-100)
            width: Width of the bar in characters
            
        Returns:
            Formatted bar string with color coding
        """
        filled_chars = int((utilization / 100) * width)
        empty_chars = width - filled_chars
        
        # Choose color based on utilization
        if utilization >= 80:
            color = "red"
        elif utilization >= 50:
            color = "yellow" 
        else:
            color = "green"
        
        bar = "█" * filled_chars + "░" * empty_chars
        return f"[{color}]{bar}[/{color}]"
    
    def format_resource_line(self, resource_type: str, used: int, total: int, 
                           utilization: float, unit: str = "") -> str:
        """Format a resource utilization line with bar chart.
        
        Args:
            resource_type: Type of resource (e.g., "CPUs", "GPUs")
            used: Used amount
            total: Total amount  
            utilization: Utilization percentage
            unit: Unit suffix (e.g., "B200", "L40S")
            
        Returns:
            Formatted line with bar chart and statistics
        """
        bar = self.create_utilization_bar(utilization)
        unit_str = f" {unit}" if unit else ""
        
        # Left-align resource types to ensure consistent bar alignment
        resource_label = f"{resource_type}:".ljust(8)  # Pad to 8 characters for alignment
        
        return f"  {resource_label} {bar} {utilization:3.0f}% ({used:,}/{total:,}{unit_str})"
    
    def create_nodes_table(self, 
                          cluster: ClusterStatus,
                          show_details: bool = False,
                          show_gpus: bool = False,
                          issues_only: bool = False) -> Table:
        """Create a Rich table for node information.
        
        Args:
            cluster: Cluster status data
            show_details: Show detailed information
            show_gpus: Show GPU information
            issues_only: Show only nodes with issues
            
        Returns:
            Rich Table object
        """
        # Create jobs mapping by node
        jobs_by_node: Dict[str, List[JobInfo]] = {}
        for job in cluster.jobs:
            if job.nodes:
                # Handle multiple nodes (expand if needed)
                node_names = [job.nodes] if "," not in job.nodes and "[" not in job.nodes else job.nodes.split(",")
                for node_name in node_names:
                    node_name = node_name.strip()
                    if node_name not in jobs_by_node:
                        jobs_by_node[node_name] = []
                    jobs_by_node[node_name].append(job)
        
        # Create table
        table = Table(box=box.ROUNDED, show_header=True, header_style="bold blue")
        
        # Add columns
        table.add_column("Node", style="white", min_width=12)
        table.add_column("State", style="white", min_width=8)
        table.add_column("CPU", style="white", min_width=10)
        table.add_column("Memory", style="white", min_width=12)
        
        if show_gpus:
            table.add_column("GPU", style="white", min_width=10)
        
        if show_details:
            table.add_column("Jobs", style="white", min_width=15)
            table.add_column("Load", style="white", min_width=8)
        else:
            table.add_column("Jobs", style="white", min_width=8)
        
        # Deduplicate nodes by name (since nodes can appear in multiple partitions)
        unique_nodes = {}
        for node in cluster.nodes:
            if node.name not in unique_nodes:
                unique_nodes[node.name] = node
            else:
                # If node appears in multiple partitions, merge partition info
                existing = unique_nodes[node.name]
                if existing.partition_name and node.partition_name:
                    if node.partition_name not in existing.partition_name:
                        existing.partition_name = f"{existing.partition_name}+{node.partition_name}"
        
        # Add rows
        for node in unique_nodes.values():
            node_jobs = jobs_by_node.get(node.name, [])
            warnings = self.get_warning_indicators(node, node_jobs)
            
            # Skip if issues_only and no warnings
            if issues_only and not warnings and not node.is_down:
                continue
            
            # State column with color and symbol
            state_color, state_symbol = self.get_state_style(node)
            
            # Determine the most appropriate state to display
            display_state = "unknown"
            if node.state:
                # Priority order for display: DOWN > DRAIN > MIXED > ALLOCATED > IDLE
                state_priority = ["DOWN", "DRAIN", "DRNG", "MIXED", "ALLOCATED", "IDLE"]
                for priority_state in state_priority:
                    if priority_state in node.state:
                        display_state = priority_state
                        break
                if display_state == "unknown":
                    display_state = node.state[0]  # Fallback to first state
            
            state_text = Text()
            state_text.append(state_symbol, style=state_color)
            state_text.append(f" {display_state}", style=state_color)
            
            # CPU column
            cpu_text = self.format_cpu_usage(node.cpus_allocated, node.cpus_total)
            cpu_util = node.cpu_utilization
            if cpu_util > 90:
                cpu_text = f"[red]{cpu_text}[/red]"
            elif cpu_util > 70:
                cpu_text = f"[yellow]{cpu_text}[/yellow]"
            else:
                cpu_text = f"[green]{cpu_text}[/green]"
            
            # Memory column
            used_mem = node.memory_total - node.memory_free
            memory_text = f"{self.format_memory(used_mem)}/{self.format_memory(node.memory_total)}"
            mem_util = node.memory_utilization
            if mem_util > 90:
                memory_text = f"[red]{memory_text}[/red]"
            elif mem_util > 70:
                memory_text = f"[yellow]{memory_text}[/yellow]"
            else:
                memory_text = f"[green]{memory_text}[/green]"
            
            # GPU column (if requested)
            gpu_text = ""
            if show_gpus:
                gpu_info = node.gpu_info
                if gpu_info["total"] > 0:
                    gpu_text = f"{gpu_info['used']}/{gpu_info['total']}"
                    if gpu_info["type"]:
                        gpu_text += f" {gpu_info['type']}"
                    
                    if gpu_info["used"] == gpu_info["total"]:
                        gpu_text = f"[red]{gpu_text}[/red]"
                    elif gpu_info["used"] > 0:
                        gpu_text = f"[yellow]{gpu_text}[/yellow]"
                    else:
                        gpu_text = f"[green]{gpu_text}[/green]"
                else:
                    gpu_text = "-"
            
            # Jobs column
            if show_details and node_jobs:
                job_details = []
                for job in node_jobs[:3]:  # Show up to 3 jobs
                    job_text = f"{job.job_id} {job.user_name}"
                    if job.allocated_gpus > 0:
                        job_text += f" (GPU:{job.allocated_gpus})"
                    job_details.append(job_text)
                
                if len(node_jobs) > 3:
                    job_details.append(f"... +{len(node_jobs) - 3} more")
                
                jobs_text = "\\n".join(job_details)
            else:
                jobs_text = f"{len(node_jobs)} jobs" if node_jobs else "-"
            
            # Load column (if detailed)
            load_text = ""
            if show_details:
                load_text = f"{node.cpu_load:.2f}"
                if warnings:
                    load_text += " " + "".join(warnings)
            else:
                if warnings:
                    jobs_text += " " + "".join(warnings)
            
            # Build row
            row = [
                node.name,
                state_text,
                cpu_text,
                memory_text
            ]
            
            if show_gpus:
                row.append(gpu_text)
            
            row.append(jobs_text)
            
            if show_details:
                row.append(load_text)
            
            table.add_row(*row)
        
        return table
    
    def create_summary_panel(self, cluster: ClusterStatus) -> Panel:
        """Create a summary panel for cluster overview.
        
        Args:
            cluster: Cluster status data
            
        Returns:
            Rich Panel with summary information
        """
        # Calculate summary statistics
        total_nodes = cluster.total_nodes
        idle_nodes = cluster.idle_nodes
        down_nodes = cluster.down_nodes
        mixed_nodes = len([n for n in cluster.nodes if n.is_mixed])
        allocated_nodes = len([n for n in cluster.nodes if "ALLOCATED" in n.state])
        
        total_cpus = cluster.total_cpus
        allocated_cpus = cluster.allocated_cpus
        cpu_util = cluster.cpu_utilization
        
        total_jobs = cluster.total_jobs
        
        # Calculate GPU statistics
        total_gpus = sum(node.gpu_info["total"] for node in cluster.nodes)
        used_gpus = sum(node.gpu_info["used"] for node in cluster.nodes)
        
        # Format summary text
        summary_lines = [
            f"[bold]Cluster Overview[/bold] (as of {cluster.timestamp.strftime('%H:%M:%S')})",
            "",
            f"[green]Nodes:[/green] {total_nodes} total • {idle_nodes} idle • {mixed_nodes} mixed • {allocated_nodes} allocated • [red]{down_nodes} down[/red]",
            f"[blue]CPUs:[/blue] {allocated_cpus:,}/{total_cpus:,} cores ({cpu_util:.1f}% utilized)",
            f"[yellow]Jobs:[/yellow] {total_jobs} running"
        ]
        
        if total_gpus > 0:
            gpu_util = (used_gpus / total_gpus * 100) if total_gpus > 0 else 0
            summary_lines.append(f"[magenta]GPUs:[/magenta] {used_gpus}/{total_gpus} ({gpu_util:.1f}% utilized)")
        
        summary_text = "\n".join(summary_lines)
        
        return Panel(
            Align.left(summary_text),
            title="Cluster Status",
            border_style="blue",
            padding=(1, 2)
        )
    
    def create_partition_panel(self, cluster: ClusterStatus) -> Panel:
        """Create a partition utilization panel with bar charts.
        
        Args:
            cluster: Cluster status data
            
        Returns:
            Rich Panel with partition utilization information
        """
        partition_stats = cluster.partition_stats
        
        if not partition_stats:
            return Panel(
                Align.center("[dim]No partition data available[/dim]"),
                title="Partition Utilization",
                border_style="blue",
                padding=(1, 2)
            )
        
        panel_lines = []
        
        for partition_name, stats in partition_stats.items():
            node_count = stats["node_count"]
            
            # Partition header
            panel_lines.append(f"[bold]{partition_name} ({node_count}):[/bold]")
            
            # CPU utilization bar
            cpu_line = self.format_resource_line(
                "CPUs", 
                stats["cpu_allocated"], 
                stats["cpu_total"],
                stats["cpu_utilization"]
            )
            panel_lines.append(cpu_line)
            
            # Memory utilization bar
            memory_used_gb = stats["memory_used"] // 1024  # Convert MB to GB
            memory_total_gb = stats["memory_total"] // 1024
            memory_line = self.format_resource_line(
                "Memory",
                memory_used_gb,
                memory_total_gb,
                stats["memory_utilization"],
                "GB"
            )
            panel_lines.append(memory_line)
            
            # GPU utilization bar (if partition has GPUs)
            if stats["gpu_total"] > 0:
                gpu_types_str = "/".join(stats["gpu_types"]).upper() if stats["gpu_types"] else ""
                gpu_line = self.format_resource_line(
                    "GPUs",
                    stats["gpu_used"],
                    stats["gpu_total"],
                    stats["gpu_utilization"],
                    gpu_types_str
                )
                panel_lines.append(gpu_line)
            
            # Add spacing between partitions
            panel_lines.append("")
        
        # Remove final empty line
        if panel_lines and panel_lines[-1] == "":
            panel_lines.pop()
        
        panel_text = "\n".join(panel_lines)
        
        return Panel(
            Align.left(panel_text),
            title="Partition Utilization",
            border_style="blue",
            padding=(1, 2)
        )
    
    def create_jobs_table(self, 
                         jobs: List[JobInfo], 
                         limit: Optional[int] = None,
                         compact: bool = False,
                         show_columns: Optional[List[str]] = None,
                         use_short_state: bool = True) -> Table:
        """Create a Rich table for job information with enhanced features.
        
        Args:
            jobs: List of jobs to display
            limit: Maximum number of jobs to show
            compact: Use compact layout for narrow terminals
            show_columns: Specific columns to show (None for auto-detect)
            use_short_state: Use short state names (R vs RUNNING)
            
        Returns:
            Rich Table object with dynamic layout
        """
        from .models import shorten_node_list
        import os
        
        # Detect terminal width
        try:
            terminal_width = int(os.popen('tput cols').read().strip())
        except:
            terminal_width = 80  # Default fallback
        
        # Determine layout based on terminal width and compact mode
        if compact or terminal_width < 80:
            layout = "narrow"
        elif terminal_width < 120:
            layout = "medium"
        else:
            layout = "wide"
        
        # Calculate dynamic Name column width based on actual job names
        def calculate_name_column_width(job_list: List[JobInfo], layout_type: str) -> int:
            """Calculate optimal Name column width based on actual job names."""
            if not job_list:
                return 8  # Minimum fallback
            
            # Find the longest job name
            max_name_length = max(len(job.name) for job in job_list)
            
            # Set bounds based on layout
            if layout_type == "narrow":
                min_width = 8
                max_width = 20
            elif layout_type == "medium":
                min_width = 10
                max_width = 25
            else:  # wide
                min_width = 12
                max_width = 35
            
            # Use actual max length, bounded by layout constraints
            return max(min_width, min(max_name_length, max_width))
        
        # Sort jobs by start time (most recent first)
        sorted_jobs = sorted(jobs, key=lambda j: j.start_time or datetime.min, reverse=True)
        
        # Apply limit if specified
        if limit:
            sorted_jobs = sorted_jobs[:limit]
        
        # Calculate dynamic name column width based on final job list
        dynamic_name_width = calculate_name_column_width(sorted_jobs, layout)
        
        # Create table with dynamic expansion
        table = Table(
            box=box.ROUNDED, 
            show_header=True, 
            header_style=f"bold {self.palette['warning']}",  # Use palette warning color for headers
            expand=True,
            width=None,  # Let Rich handle width automatically
            collapse_padding=True  # Reduce padding when space is tight
        )
        
        # Define partition colors using current palette
        partition_colors = {
            "cpu": self.palette["primary"],    # Blue/primary
            "gpu": self.palette["success"],    # Green/success
            "fat": self.palette["accent"],     # Magenta/accent
            "vdi": self.palette["info"]        # Cyan/info
        }
        
        # Define columns based on layout
        if layout == "narrow":
            columns = [
                ("Job ID", 6, None),
                ("User", 4, None),  # "me" takes 2 chars, usernames are 7
                ("Name", dynamic_name_width, None),  # Use dynamic width, no expansion ratio
                ("State", 3, None),
                ("Part", 4, None),
                ("GPUs", 4, None),  # Shortened for count only
                ("Time", 5, None),
                ("Nodes", 8, 1)  # Allow nodes to expand if there's extra space
            ]
        elif layout == "medium":
            columns = [
                ("Job ID", 7, None),
                ("User", 4, None),  # "me" saves space
                ("Account", 6, None),
                ("State", 4, None),
                ("Partition", 7, None),
                ("Name", dynamic_name_width, None),  # Use dynamic width, no expansion ratio
                ("CPUs", 4, None),
                ("GPUs", 7, None),
                ("Time", 5, None),
                ("Remaining", 7, None),
                ("Nodes", 8, 1)  # Allow nodes to expand if there's extra space
            ]
        else:  # wide
            columns = [
                ("Job ID", 8, None),
                ("User", 8, None),  # Allow space for non-current users
                ("Account", 10, None),
                ("State", 8, None),
                ("Partition", 10, None),
                ("Name", dynamic_name_width, None),  # Use dynamic width, no expansion ratio
                ("CPUs", 6, None),
                ("GPUs", 10, None),
                ("Time", 10, None),
                ("Remaining", 10, None),
                ("Nodes", 15, 1)  # Allow nodes to expand if there's extra space
            ]
        
        # Filter columns based on show_columns parameter
        filtered_columns = []
        for col_name, min_width, ratio in columns:
            if show_columns is None or col_name in show_columns:
                filtered_columns.append((col_name, min_width, ratio))
                table.add_column(
                    col_name, 
                    style="white", 
                    min_width=min_width,
                    ratio=ratio,
                    no_wrap=(ratio is None)  # Don't wrap fixed-width columns
                )
        
        for job in sorted_jobs:
            row_data = []
            
            # Build row data based on filtered columns
            for col_name, _, _ in filtered_columns:
                if col_name == "Job ID":
                    job_id_text = str(job.job_id)
                    if job.is_array_job:
                        job_id_text = f"[{self.palette['info']}]{job_id_text}[][/{self.palette['info']}]"
                    row_data.append(job_id_text)
                
                elif col_name == "User":
                    user_text = job.user_name
                    
                    # Smart user display: show "me" for current user
                    # Username pattern is [m,x,s][\d{6}] = always 7 characters
                    current_user = os.environ.get('USER', '')
                    if user_text == current_user:
                        user_text = "me"
                    # No need to truncate since usernames are always 7 chars and "me" is 2 chars
                    
                    row_data.append(user_text)
                
                elif col_name == "Account":
                    account_text = job.account or "-"
                    if layout == "medium" and len(account_text) > 8:
                        account_text = account_text[:8]
                    row_data.append(account_text)
                
                elif col_name == "State":
                    state_text = "UNK"
                    if job.state:
                        if use_short_state:
                            state_map = {
                                "RUNNING": "R", "PENDING": "PD", "COMPLETED": "CD", 
                                "CANCELLED": "CA", "FAILED": "F", "TIMEOUT": "TO",
                                "SUSPENDED": "S", "CONFIGURING": "CF"
                            }
                            state_text = state_map.get(job.state[0], job.state[0][:2])
                        else:
                            state_text = job.state[0]
                    
                    # Color state based on status using palette
                    if state_text in ["R", "RUNNING"]:
                        state_text = f"[{self.palette['success']}]{state_text}[/{self.palette['success']}]"
                    elif state_text in ["PD", "PENDING"]:
                        state_text = f"[{self.palette['warning']}]{state_text}[/{self.palette['warning']}]"
                    elif state_text in ["F", "FAILED", "CA", "CANCELLED", "TO", "TIMEOUT"]:
                        state_text = f"[{self.palette['danger']}]{state_text}[/{self.palette['danger']}]"
                    
                    row_data.append(state_text)
                
                elif col_name in ["Partition", "Part"]:
                    partition_text = job.partition
                    if col_name == "Part":  # Narrow layout
                        partition_text = partition_text[:4]
                    
                    # Apply partition-specific coloring
                    color = partition_colors.get(job.partition, "white")
                    partition_text = f"[{color}]{partition_text}[/{color}]"
                    row_data.append(partition_text)
                
                elif col_name == "Name":
                    name_text = job.name
                    # Only truncate if the name is longer than our calculated dynamic width
                    if len(name_text) > dynamic_name_width:
                        name_text = name_text[:dynamic_name_width-3] + "..."
                    row_data.append(name_text)
                
                elif col_name == "CPUs":
                    total_cpus = job.cpus_per_task * job.tasks
                    row_data.append(str(total_cpus))
                
                elif col_name == "GPUs":
                    gpu_info = job.gpu_type_info
                    if gpu_info["count"] > 0:
                        # Color GPUs by type using palette colors:
                        # L40S (gpu partition) = success, B200 (fat partition) = danger
                        gpu_type = gpu_info["type"].upper()
                        if "L40S" in gpu_type:
                            color = self.palette["success"]  # Matches gpu partition color
                        elif "B200" in gpu_type:
                            color = self.palette["danger"]   # Distinctive color for fat partition B200s
                        else:
                            color = self.palette["accent"]   # Fallback for unknown GPU types
                        
                        # Simplify GPU display for narrow terminals (show count only)
                        if layout == "narrow":
                            gpu_text = f"[{color}]{gpu_info['count']}[/{color}]"
                        else:
                            gpu_text = f"[{color}]{gpu_info['display']}[/{color}]"
                    else:
                        gpu_text = "-"
                    row_data.append(gpu_text)
                
                elif col_name == "Time":
                    time_text = "-"
                    if job.start_time:
                        elapsed = datetime.now() - job.start_time
                        total_seconds = int(elapsed.total_seconds())
                        
                        if elapsed.days > 0:
                            time_text = f"{elapsed.days}d {elapsed.seconds // 3600}h"
                        elif total_seconds >= 3600:
                            time_text = f"{total_seconds // 3600}h {(total_seconds % 3600) // 60}m"
                        elif total_seconds >= 60:
                            time_text = f"{total_seconds // 60}m"
                        else:
                            # Show seconds for jobs running less than 1 minute
                            time_text = f"{total_seconds}s"
                    row_data.append(time_text)
                
                elif col_name == "Remaining":
                    remaining_text = job.remaining_time_display
                    if remaining_text != "-" and job.remaining_time_minutes is not None:
                        if job.remaining_time_minutes < 60:  # Less than 1 hour remaining
                            remaining_text = f"[{self.palette['danger']}]{remaining_text}[/{self.palette['danger']}]"
                        elif job.remaining_time_minutes < 180:  # Less than 3 hours remaining
                            remaining_text = f"[{self.palette['warning']}]{remaining_text}[/{self.palette['warning']}]"
                    row_data.append(remaining_text)
                
                elif col_name == "Nodes":
                    nodes_text = shorten_node_list(job.nodes)
                    if layout == "narrow" and len(nodes_text) > 10:
                        nodes_text = nodes_text[:10]
                    row_data.append(nodes_text)
            
            table.add_row(*row_data)
        
        return table
    
    def print_cluster_status(self, 
                           cluster: ClusterStatus,
                           show_summary: bool = True,
                           show_partitions: bool = True,
                           show_details: bool = False,
                           show_gpus: bool = False,
                           issues_only: bool = False):
        """Print complete cluster status.
        
        Args:
            cluster: Cluster status data
            show_summary: Show summary panel
            show_partitions: Show partition utilization panel
            show_details: Show detailed node information
            show_gpus: Show GPU information
            issues_only: Show only nodes with issues
        """
        if show_summary:
            summary = self.create_summary_panel(cluster)
            self.console.print(summary)
            self.console.print()
        
        if show_partitions:
            partition_panel = self.create_partition_panel(cluster)
            self.console.print(partition_panel)
            self.console.print()
        
        nodes_table = self.create_nodes_table(
            cluster=cluster,
            show_details=show_details,
            show_gpus=show_gpus,
            issues_only=issues_only
        )
        
        self.console.print(nodes_table)
    
    def print_jobs(self, 
                  jobs: List[JobInfo], 
                  limit: Optional[int] = None,
                  compact: bool = False,
                  show_columns: Optional[List[str]] = None,
                  use_short_state: bool = True):
        """Print job information table with enhanced formatting.
        
        Args:
            jobs: List of jobs to display
            limit: Maximum number of jobs to show
            compact: Use compact layout for narrow terminals
            show_columns: Specific columns to show (None for auto-detect)
            use_short_state: Use short state names (R vs RUNNING)
        """
        if not jobs:
            self.console.print("[yellow]No jobs found[/yellow]")
            return
        
        jobs_table = self.create_jobs_table(
            jobs, 
            limit=limit,
            compact=compact,
            show_columns=show_columns,
            use_short_state=use_short_state
        )
        self.console.print(jobs_table)
        
        # Add concise hint about node name shortening
        self.console.print("[dim]Note: Node names are shortened (full name: demu4x{shown})[/dim]")


# Global formatter instance (will be updated with palette choice)
formatter = DisplayFormatter()


def get_available_palettes() -> List[str]:
    """Get list of available color palette names."""
    return list(COLOR_PALETTES.keys())


def create_formatter_with_palette(palette_name: str = "standard") -> DisplayFormatter:
    """Create a new formatter instance with the specified palette."""
    return DisplayFormatter(color_palette=palette_name)