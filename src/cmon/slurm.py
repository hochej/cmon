"""Interface to Slurm commands using JSON output."""

import json
import subprocess
from typing import List, Optional, Dict, Any
from .models import NodeInfo, JobInfo, ClusterStatus


class SlurmError(Exception):
    """Exception raised for Slurm command errors."""
    pass


class SlurmInterface:
    """Interface to Slurm commands with JSON output."""
    
    def __init__(self, slurm_bin_path: str = "/usr/bin"):
        """Initialize Slurm interface.
        
        Args:
            slurm_bin_path: Path to Slurm binaries (default: /usr/bin)
        """
        self.slurm_bin_path = slurm_bin_path
        self.sinfo_cmd = f"{slurm_bin_path}/sinfo"
        self.squeue_cmd = f"{slurm_bin_path}/squeue"
        self.scontrol_cmd = f"{slurm_bin_path}/scontrol"
    
    def _run_command(self, cmd: List[str]) -> Dict[str, Any]:
        """Run a Slurm command and return parsed JSON.
        
        Args:
            cmd: Command and arguments to run
            
        Returns:
            Parsed JSON response
            
        Raises:
            SlurmError: If command fails or returns invalid JSON
        """
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=True,
                timeout=30
            )
            
            if not result.stdout.strip():
                raise SlurmError(f"Command {' '.join(cmd)} returned empty output")
            
            try:
                return json.loads(result.stdout)
            except json.JSONDecodeError as e:
                raise SlurmError(f"Invalid JSON from {' '.join(cmd)}: {e}")
                
        except subprocess.CalledProcessError as e:
            error_msg = e.stderr.strip() if e.stderr else "Unknown error"
            raise SlurmError(f"Command {' '.join(cmd)} failed: {error_msg}")
        except subprocess.TimeoutExpired:
            raise SlurmError(f"Command {' '.join(cmd)} timed out")
    
    def get_nodes(self, 
                  partition: Optional[str] = None,
                  nodelist: Optional[str] = None,
                  states: Optional[List[str]] = None,
                  all_partitions: bool = False) -> List[NodeInfo]:
        """Get node information from sinfo.
        
        Args:
            partition: Filter by partition name
            nodelist: Filter by node list (supports ranges)
            states: Filter by node states
            all_partitions: Include all partitions (hidden/unavailable)
            
        Returns:
            List of NodeInfo objects
        """
        cmd = [self.sinfo_cmd, "-N", "--json"]
        
        if all_partitions:
            cmd.append("--all")
        
        if partition:
            cmd.extend(["-p", partition])
        
        if nodelist:
            cmd.extend(["-n", nodelist])
        
        if states:
            cmd.extend(["--states", ",".join(states)])
        
        try:
            data = self._run_command(cmd)
            
            # Check for errors in response
            if data.get("errors"):
                error_msg = "; ".join(data["errors"])
                raise SlurmError(f"sinfo errors: {error_msg}")
            
            nodes = []
            for node_data in data.get("sinfo", []):
                try:
                    node = NodeInfo.from_sinfo_json(node_data)
                    if node.name:  # Only add nodes with valid names
                        nodes.append(node)
                except Exception as e:
                    # Log parsing error but continue with other nodes
                    print(f"Warning: Failed to parse node data: {e}")
                    continue
            
            return nodes
            
        except SlurmError:
            raise
        except Exception as e:
            raise SlurmError(f"Failed to get node information: {e}")
    
    def get_jobs(self,
                 users: Optional[List[str]] = None,
                 accounts: Optional[List[str]] = None,
                 partitions: Optional[List[str]] = None,
                 states: Optional[List[str]] = None,
                 job_ids: Optional[List[int]] = None) -> List[JobInfo]:
        """Get job information from squeue.
        
        Args:
            users: Filter by usernames
            accounts: Filter by account names
            partitions: Filter by partition names
            states: Filter by job states (default: RUNNING)
            job_ids: Filter by specific job IDs
            
        Returns:
            List of JobInfo objects
        """
        cmd = [self.squeue_cmd, "--json"]
        
        # Only filter by states if explicitly specified
        # If states is None, show all job states (don't add -t flag)
        if states is not None:
            cmd.extend(["-t", ",".join(states)])
        
        if users:
            cmd.extend(["-u", ",".join(users)])
        
        if accounts:
            for account in accounts:
                cmd.extend(["-A", account])
        
        if partitions:
            cmd.extend(["-p", ",".join(partitions)])
        
        if job_ids:
            cmd.extend(["-j", ",".join(map(str, job_ids))])
        
        try:
            data = self._run_command(cmd)
            
            # Check for errors in response
            if data.get("errors"):
                error_msg = "; ".join(data["errors"])
                raise SlurmError(f"squeue errors: {error_msg}")
            
            jobs = []
            for job_data in data.get("jobs", []):
                try:
                    job = JobInfo.from_squeue_json(job_data)
                    if job.job_id:  # Only add jobs with valid IDs
                        jobs.append(job)
                except Exception as e:
                    # Log parsing error but continue with other jobs
                    print(f"Warning: Failed to parse job data: {e}")
                    continue
            
            return jobs
            
        except SlurmError:
            raise
        except Exception as e:
            raise SlurmError(f"Failed to get job information: {e}")
    
    def expand_hostlist(self, hostlist: str) -> List[str]:
        """Expand a hostlist expression to individual hostnames.
        
        Args:
            hostlist: Hostlist expression (e.g., "node[001-010]")
            
        Returns:
            List of individual hostnames
        """
        if not hostlist.strip():
            return []
        
        # If no special characters, return as-is
        if "[" not in hostlist and "," not in hostlist:
            return [hostlist.strip()]
        
        cmd = [self.scontrol_cmd, "show", "hostnames", hostlist]
        
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                check=True,
                timeout=10
            )
            
            return [line.strip() for line in result.stdout.splitlines() if line.strip()]
            
        except subprocess.CalledProcessError as e:
            # If expansion fails, return original as single item
            print(f"Warning: Failed to expand hostlist '{hostlist}': {e}")
            return [hostlist]
        except subprocess.TimeoutExpired:
            print(f"Warning: Hostlist expansion timed out for '{hostlist}'")
            return [hostlist]
    
    def get_cluster_status(self,
                          partition: Optional[str] = None,
                          user: Optional[str] = None,
                          nodelist: Optional[str] = None) -> ClusterStatus:
        """Get complete cluster status including nodes and jobs.
        
        Args:
            partition: Filter by partition
            user: Filter jobs by user
            nodelist: Filter by node list
            
        Returns:
            ClusterStatus object with nodes and jobs
        """
        try:
            # Get nodes
            nodes = self.get_nodes(
                partition=partition,
                nodelist=nodelist
            )
            
            # Get jobs
            job_filter_args = {}
            if user:
                job_filter_args["users"] = [user]
            if partition:
                job_filter_args["partitions"] = [partition]
            
            jobs = self.get_jobs(**job_filter_args)
            
            return ClusterStatus(nodes=nodes, jobs=jobs)
            
        except Exception as e:
            raise SlurmError(f"Failed to get cluster status: {e}")
    
    def test_connection(self) -> bool:
        """Test if Slurm commands are available and working.
        
        Returns:
            True if Slurm is accessible, False otherwise
        """
        try:
            # Test with a simple sinfo command
            subprocess.run(
                [self.sinfo_cmd, "--version"],
                capture_output=True,
                check=True,
                timeout=5
            )
            return True
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
            return False


# Global instance for easy access
slurm = SlurmInterface()