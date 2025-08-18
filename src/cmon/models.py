"""Pydantic models for Slurm JSON data structures."""

from datetime import datetime
from typing import Optional, List, Dict, Any
from pydantic import BaseModel, Field, validator


class TimeValue(BaseModel):
    """Slurm time value with set/infinite/number structure."""
    set: bool
    infinite: bool
    number: int


class NodeInfo(BaseModel):
    """Model for node information from sinfo --json."""
    name: str
    state: List[str]
    partition_name: Optional[str] = None
    
    # CPU information
    cpus_allocated: int
    cpus_idle: int
    cpus_total: int
    cpu_load: float  # Will be converted from integer ÷ 100
    
    # Memory information (in MB)
    memory_total: int
    memory_free: int
    memory_allocated: int
    
    # Hardware info
    sockets: int
    cores_per_socket: int
    threads_per_core: int
    
    # GRES (Generic Resources)
    gres_total: str = ""
    gres_used: str = ""
    
    # Additional info
    features: str = ""
    reason: str = ""
    weight: int = 1
    
    @classmethod
    def from_sinfo_json(cls, node_data: Dict[str, Any]) -> "NodeInfo":
        """Create NodeInfo from sinfo JSON data."""
        # Extract CPU load and convert from integer × 100
        cpu_load_raw = node_data.get("cpus", {}).get("load", {}).get("minimum", 0)
        cpu_load = cpu_load_raw / 100.0 if cpu_load_raw else 0.0
        
        # Extract memory information
        memory_data = node_data.get("memory", {})
        memory_free_obj = memory_data.get("free", {}).get("minimum", {})
        memory_free = memory_free_obj.get("number", 0) if memory_free_obj.get("set", False) else 0
        
        # Extract GRES information
        gres_data = node_data.get("gres", {})
        
        return cls(
            name=node_data.get("nodes", {}).get("nodes", [""])[0],
            state=node_data.get("node", {}).get("state", []),
            partition_name=node_data.get("partition", {}).get("name"),
            
            cpus_allocated=node_data.get("cpus", {}).get("allocated", 0),
            cpus_idle=node_data.get("cpus", {}).get("idle", 0),
            cpus_total=node_data.get("cpus", {}).get("total", 0),
            cpu_load=cpu_load,
            
            memory_total=memory_data.get("minimum", 0),
            memory_free=memory_free,
            memory_allocated=memory_data.get("allocated", 0),
            
            sockets=node_data.get("sockets", {}).get("minimum", 1),
            cores_per_socket=node_data.get("cores", {}).get("minimum", 1),
            threads_per_core=node_data.get("threads", {}).get("minimum", 1),
            
            gres_total=gres_data.get("total", ""),
            gres_used=gres_data.get("used", ""),
            
            features=node_data.get("features", {}).get("total", ""),
            reason=node_data.get("reason", {}).get("description", "") if isinstance(node_data.get("reason", ""), dict) else str(node_data.get("reason", "")),
            weight=node_data.get("weight", {}).get("minimum", 1),
        )
    
    @property
    def is_idle(self) -> bool:
        """Check if node is idle."""
        return "IDLE" in self.state
    
    @property
    def is_down(self) -> bool:
        """Check if node is down."""
        # Only consider truly down if DOWN is present, or if DRAIN/DRNG without IDLE
        if "DOWN" in self.state:
            return True
        elif any(state in ["DRAIN", "DRNG"] for state in self.state):
            # Only down if draining AND not currently idle
            return "IDLE" not in self.state
        return False
    
    @property
    def is_mixed(self) -> bool:
        """Check if node is in mixed state."""
        return "MIXED" in self.state
    
    @property
    def cpu_utilization(self) -> float:
        """Calculate CPU utilization percentage."""
        if self.cpus_total == 0:
            return 0.0
        return (self.cpus_allocated / self.cpus_total) * 100
    
    @property
    def memory_utilization(self) -> float:
        """Calculate memory utilization percentage."""
        if self.memory_total == 0:
            return 0.0
        used_memory = self.memory_total - self.memory_free
        return (used_memory / self.memory_total) * 100
    
    @property
    def gpu_info(self) -> Dict[str, Any]:
        """Parse GPU information from GRES."""
        gpu_info = {"total": 0, "used": 0, "type": ""}
        
        # Parse total GPUs
        if "gpu:" in self.gres_total:
            parts = self.gres_total.split("gpu:")
            for part in parts[1:]:
                if ":" in part:
                    gpu_type, count_info = part.split(":", 1)
                    gpu_info["type"] = gpu_type
                    # Extract count (before any parentheses)
                    count_str = count_info.split("(")[0].split(",")[0]
                    try:
                        gpu_info["total"] = int(count_str)
                    except ValueError:
                        pass
        
        # Parse used GPUs
        if "gpu:" in self.gres_used:
            parts = self.gres_used.split("gpu:")
            for part in parts[1:]:
                if ":" in part:
                    # Format: gpu:l40s:4(IDX:0-3)
                    subparts = part.split(":")
                    if len(subparts) >= 2:
                        count_str = subparts[1].split("(")[0].split(",")[0]
                        try:
                            gpu_info["used"] = int(count_str)
                        except ValueError:
                            pass
        
        return gpu_info


class JobInfo(BaseModel):
    """Model for job information from squeue --json."""
    job_id: int
    array_job_id: int = 0
    name: str
    user_name: str
    group_name: str
    account: str
    partition: str
    state: List[str]
    nodes: str
    
    # Resource allocation
    tres_alloc_str: str = ""
    cpus_per_task: int = 1
    tasks: int = 1
    
    # Time information
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None
    time_limit: Optional[int] = None  # in minutes
    
    # Additional info
    qos: str = ""
    flags: List[str] = []
    batch_host: str = ""
    
    @classmethod
    def from_squeue_json(cls, job_data: Dict[str, Any]) -> "JobInfo":
        """Create JobInfo from squeue JSON data."""
        # Extract time values
        start_time = None
        start_time_obj = job_data.get("start_time", {})
        if start_time_obj.get("set", False):
            start_time = datetime.fromtimestamp(start_time_obj.get("number", 0))
        
        end_time = None
        end_time_obj = job_data.get("end_time", {})
        if end_time_obj.get("set", False):
            end_time = datetime.fromtimestamp(end_time_obj.get("number", 0))
        
        time_limit = None
        time_limit_obj = job_data.get("time_limit", {})
        if time_limit_obj.get("set", False):
            time_limit = time_limit_obj.get("number", 0)
        
        # Extract other values with safe defaults
        cpus_per_task_obj = job_data.get("cpus_per_task", {})
        cpus_per_task = cpus_per_task_obj.get("number", 1) if cpus_per_task_obj.get("set", False) else 1
        
        tasks_obj = job_data.get("tasks", {})
        tasks = tasks_obj.get("number", 1) if tasks_obj.get("set", False) else 1
        
        array_job_id_obj = job_data.get("array_job_id", {})
        array_job_id = array_job_id_obj.get("number", 0) if array_job_id_obj.get("set", False) else 0
        
        return cls(
            job_id=job_data.get("job_id", 0),
            array_job_id=array_job_id,
            name=job_data.get("name", ""),
            user_name=job_data.get("user_name", ""),
            group_name=job_data.get("group_name", ""),
            account=job_data.get("account", ""),
            partition=job_data.get("partition", ""),
            state=job_data.get("job_state", []),
            nodes=job_data.get("nodes", ""),
            
            tres_alloc_str=job_data.get("tres_alloc_str", ""),
            cpus_per_task=cpus_per_task,
            tasks=tasks,
            
            start_time=start_time,
            end_time=end_time,
            time_limit=time_limit,
            
            qos=job_data.get("qos", ""),
            flags=job_data.get("flags", []),
            batch_host=job_data.get("batch_host", ""),
        )
    
    @property
    def is_running(self) -> bool:
        """Check if job is running."""
        return "RUNNING" in self.state
    
    @property
    def is_array_job(self) -> bool:
        """Check if this is an array job."""
        return self.array_job_id != 0
    
    @property
    def allocated_resources(self) -> Dict[str, str]:
        """Parse allocated resources from TRES string."""
        resources = {}
        if self.tres_alloc_str:
            for item in self.tres_alloc_str.split(","):
                if "=" in item:
                    key, value = item.split("=", 1)
                    resources[key.strip()] = value.strip()
        return resources
    
    @property
    def allocated_gpus(self) -> int:
        """Get number of allocated GPUs."""
        resources = self.allocated_resources
        for key, value in resources.items():
            if "gres/gpu" in key:
                try:
                    return int(value)
                except ValueError:
                    pass
        return 0
    
    @property
    def gpu_type_info(self) -> Dict[str, Any]:
        """Parse GPU type information from TRES allocation string.
        
        Returns:
            Dict with 'count', 'type', and 'display' keys
        """
        resources = self.allocated_resources
        gpu_info = {"count": 0, "type": "", "display": "-"}
        
        # Look for specific GPU type allocations in tres_alloc_str
        for key, value in resources.items():
            if "gres/gpu:" in key:
                # Extract GPU type from key like "gres/gpu:l40s"
                gpu_type = key.split("gres/gpu:")[-1]
                try:
                    count = int(value)
                    gpu_info["count"] = count
                    gpu_info["type"] = gpu_type.upper()
                    gpu_info["display"] = f"{count}x{gpu_type.upper()}"
                    break
                except ValueError:
                    pass
        
        # Fallback to generic GPU count if no specific type found
        if gpu_info["count"] == 0:
            gpu_count = self.allocated_gpus
            if gpu_count > 0:
                gpu_info["count"] = gpu_count
                gpu_info["display"] = str(gpu_count)
        
        return gpu_info
    
    @property
    def remaining_time_minutes(self) -> Optional[int]:
        """Calculate remaining time in minutes.
        
        Returns:
            Remaining time in minutes, or None if not calculable
        """
        if not self.time_limit or not self.start_time:
            return None
        
        from datetime import datetime
        elapsed_time = datetime.now() - self.start_time
        elapsed_minutes = int(elapsed_time.total_seconds() / 60)
        
        remaining = self.time_limit - elapsed_minutes
        return max(0, remaining)  # Don't return negative values
    
    @property
    def remaining_time_display(self) -> str:
        """Get formatted remaining time display.
        
        Returns:
            Formatted time string like "2h 30m" or "45m" or "-"
        """
        remaining = self.remaining_time_minutes
        if remaining is None:
            return "-"
        
        if remaining == 0:
            return "0m"
        elif remaining < 60:
            return f"{remaining}m"
        elif remaining < 1440:  # Less than 24 hours
            hours = remaining // 60
            minutes = remaining % 60
            if minutes == 0:
                return f"{hours}h"
            else:
                return f"{hours}h {minutes}m"
        else:  # 24 hours or more
            days = remaining // 1440
            hours = (remaining % 1440) // 60
            if hours == 0:
                return f"{days}d"
            else:
                return f"{days}d {hours}h"
    
    @property
    def allocated_memory_gb(self) -> Optional[float]:
        """Get allocated memory in GB."""
        resources = self.allocated_resources
        mem_str = resources.get("mem", "")
        if mem_str:
            try:
                if mem_str.endswith("G"):
                    return float(mem_str[:-1])
                elif mem_str.endswith("M"):
                    return float(mem_str[:-1]) / 1024
                elif mem_str.endswith("K"):
                    return float(mem_str[:-1]) / (1024 * 1024)
                else:
                    # Assume MB if no unit
                    return float(mem_str) / 1024
            except ValueError:
                pass
        return None


class ClusterStatus(BaseModel):
    """Overall cluster status combining nodes and jobs."""
    nodes: List[NodeInfo]
    jobs: List[JobInfo]
    timestamp: datetime = Field(default_factory=datetime.now)
    
    @property
    def total_nodes(self) -> int:
        """Total number of nodes."""
        return len(self.nodes)
    
    @property
    def idle_nodes(self) -> int:
        """Number of idle nodes."""
        return len([n for n in self.nodes if n.is_idle])
    
    @property
    def down_nodes(self) -> int:
        """Number of down/drain nodes."""
        return len([n for n in self.nodes if n.is_down])
    
    @property
    def total_cpus(self) -> int:
        """Total CPU cores in cluster."""
        return sum(n.cpus_total for n in self.nodes)
    
    @property
    def allocated_cpus(self) -> int:
        """Total allocated CPU cores."""
        return sum(n.cpus_allocated for n in self.nodes)
    
    @property
    def total_jobs(self) -> int:
        """Total number of running jobs."""
        return len(self.jobs)
    
    @property
    def cpu_utilization(self) -> float:
        """Overall CPU utilization percentage."""
        if self.total_cpus == 0:
            return 0.0
        return (self.allocated_cpus / self.total_cpus) * 100
    
    @property
    def partition_stats(self) -> Dict[str, Dict[str, Any]]:
        """Get statistics grouped by hardware partition."""
        partitions = {
            "CPU Nodes": {"nodes": [], "prefix": "demu4xcpu"},
            "Fat Nodes": {"nodes": [], "prefix": "demu4xfat"},
            "GPU Nodes": {"nodes": [], "prefix": "demu4xgpu"},
            "VDI Nodes": {"nodes": [], "prefix": "demu4xvdi"}
        }
        
        # Group nodes by partition based on name prefix (with deduplication)
        for node in self.nodes:
            for partition_name, partition_info in partitions.items():
                if node.name.startswith(partition_info["prefix"]):
                    # Only add if not already present (avoid duplicates from multiple partitions)
                    if not any(n.name == node.name for n in partition_info["nodes"]):
                        partition_info["nodes"].append(node)
                    break
        
        # Calculate statistics for each partition
        stats = {}
        for partition_name, partition_info in partitions.items():
            nodes = partition_info["nodes"]
            if not nodes:
                continue
                
            total_cpus = sum(n.cpus_total for n in nodes)
            allocated_cpus = sum(n.cpus_allocated for n in nodes)
            
            # Calculate memory stats (Slurm-allocated memory)
            total_memory = sum(n.memory_total for n in nodes)
            allocated_memory = sum(n.memory_allocated for n in nodes)
            memory_utilization = (allocated_memory / total_memory * 100) if total_memory > 0 else 0
            
            # Calculate GPU stats
            total_gpus = sum(n.gpu_info["total"] for n in nodes)
            used_gpus = sum(n.gpu_info["used"] for n in nodes)
            gpu_types = set(n.gpu_info["type"] for n in nodes if n.gpu_info["type"])
            
            stats[partition_name] = {
                "node_count": len(nodes),
                "cpu_total": total_cpus,
                "cpu_allocated": allocated_cpus,
                "cpu_utilization": (allocated_cpus / total_cpus * 100) if total_cpus > 0 else 0,
                "memory_total": total_memory,
                "memory_used": allocated_memory,
                "memory_utilization": memory_utilization,
                "gpu_total": total_gpus,
                "gpu_used": used_gpus,
                "gpu_utilization": (used_gpus / total_gpus * 100) if total_gpus > 0 else 0,
                "gpu_types": list(gpu_types)
            }
            
        return stats


def shorten_node_name(node_name: str) -> str:
    """Shorten node names by removing the 'demu4x' prefix.
    
    Args:
        node_name: Full node name like 'demu4xcpu022' or 'demu4xgpu001'
        
    Returns:
        Shortened name like 'cpu022' or 'gpu001'
    """
    if node_name.startswith("demu4x"):
        return node_name[6:]  # Remove 'demu4x' prefix
    return node_name


def shorten_node_list(node_list: str) -> str:
    """Shorten a comma-separated list of node names.
    
    Args:
        node_list: Comma-separated node names like 'demu4xcpu022,demu4xcpu024'
        
    Returns:
        Shortened node list like 'cpu022,cpu024'
    """
    if not node_list:
        return node_list
    
    # Handle multiple nodes separated by commas
    nodes = [node.strip() for node in node_list.split(",")]
    shortened_nodes = [shorten_node_name(node) for node in nodes]
    return ",".join(shortened_nodes)