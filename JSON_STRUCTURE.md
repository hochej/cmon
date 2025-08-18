# Slurm 24.11 JSON Structure Documentation

This document details the JSON output formats from Slurm 24.11 commands, which are used by the Python pestat implementation.

## Table of Contents
- [sinfo JSON Structure](#sinfo-json-structure)
- [squeue JSON Structure](#squeue-json-structure)
- [Key Findings](#key-findings)
- [Usage Examples](#usage-examples)

## sinfo JSON Structure

### Top-level Structure
```json
{
  "errors": [],
  "meta": {},
  "sinfo": [...],
  "warnings": []
}
```

### Node Information (sinfo array element)
Each element in the `sinfo` array represents a node with the following structure:

```json
{
  "port": 6818,
  "cluster": "onehpc",
  "partition": {
    "name": "cpu",
    "nodes": {
      "allowed_allocation": "",
      "configured": "demu4xcpu[001-050]",
      "total": 50
    },
    ...
  },
  "node": {
    "state": ["IDLE", "DRAIN"]  // Array of state flags
  },
  "nodes": {
    "allocated": 0,
    "idle": 0,
    "other": 1,
    "total": 1,
    "hostnames": [],
    "addresses": [],
    "nodes": ["demu4xcpu001"]  // Actual node name(s)
  },
  "cpus": {
    "allocated": 0,
    "idle": 256,
    "other": 0,
    "total": 256,
    "minimum": 256,
    "maximum": 256,
    "load": {
      "minimum": 1,      // CPU load * 100 (1 = 0.01)
      "maximum": 1
    },
    "per_node": {
      "max": {
        "set": false,
        "infinite": true,
        "number": 0
      }
    }
  },
  "memory": {
    "minimum": 1547680,  // Total memory in MB
    "maximum": 1547680,
    "free": {
      "minimum": {
        "set": true,
        "infinite": false,
        "number": 1534954  // Free memory in MB
      },
      "maximum": {
        "set": true,
        "infinite": false,
        "number": 1534954
      }
    },
    "allocated": 0  // Allocated memory in MB
  },
  "gres": {
    "total": "tmpsize:7T,gpu:l40s:4(S:0-1)",
    "used": "gpu:l40s:1(IDX:0),tmpsize:10737418240"
  },
  "sockets": {
    "minimum": 2,
    "maximum": 2
  },
  "cores": {
    "minimum": 64,
    "maximum": 64
  },
  "threads": {
    "minimum": 2,
    "maximum": 2
  },
  "features": {
    "total": "",
    "active": ""
  },
  "reason": "",
  "reservation": "",
  "weight": {
    "minimum": 1,
    "maximum": 1
  }
}
```

### Important Fields for pestat

| Field Path | Description | Example |
|------------|-------------|---------|
| `nodes.nodes[0]` | Node hostname | "demu4xcpu001" |
| `node.state` | Array of node states | ["IDLE", "DRAIN"] |
| `partition.name` | Partition name | "cpu" |
| `cpus.allocated` | Allocated CPU cores | 2 |
| `cpus.total` | Total CPU cores | 256 |
| `cpus.load.minimum` | CPU load (×100) | 234 (= 2.34) |
| `memory.minimum` | Total memory (MB) | 1547680 |
| `memory.free.minimum.number` | Free memory (MB) | 1534954 |
| `memory.allocated` | Allocated memory (MB) | 4096 |
| `gres.total` | Total GRES resources | "gpu:l40s:4" |
| `gres.used` | Used GRES resources | "gpu:l40s:1" |
| `threads.minimum` | Threads per core | 2 |

## squeue JSON Structure

### Top-level Structure
```json
{
  "errors": [],
  "meta": {},
  "jobs": [...],
  "warnings": []
}
```

### Job Information (jobs array element)
Each element in the `jobs` array represents a running job:

```json
{
  "account": "admin",
  "job_id": 525,
  "name": "sleep_gpu_$RANDOM",
  "user_name": "m318358",
  "group_name": "merckdefault",
  "partition": "gpu",
  "job_state": ["RUNNING"],
  "nodes": "demu4xgpu001",
  "tres_alloc_str": "cpu=2,mem=4G,node=1,billing=2,gres/gpu=1,gres/gpu:l40s=1,gres/tmpsize=10737418240",
  "start_time": {
    "set": true,
    "infinite": false,
    "number": 1755290346  // Unix timestamp
  },
  "end_time": {
    "set": true,
    "infinite": false,
    "number": 1755290406  // Unix timestamp
  },
  "time_limit": {
    "set": true,
    "infinite": false,
    "number": 60  // Minutes
  },
  "batch_host": "demu4xgpu001",
  "cpus_per_task": {
    "set": true,
    "infinite": false,
    "number": 1
  },
  "tasks": {
    "set": true,
    "infinite": false,
    "number": 1
  },
  "array_job_id": {
    "set": true,
    "infinite": false,
    "number": 0  // Non-zero for array jobs
  },
  "qos": "admin",
  "flags": [
    "ACCRUE_COUNT_CLEARED",
    "JOB_WAS_RUNNING",
    "USING_DEFAULT_ACCOUNT"
  ]
}
```

### Important Fields for pestat

| Field Path | Description | Example |
|------------|-------------|---------|
| `job_id` | Job ID | 525 |
| `name` | Job name | "sleep_gpu" |
| `user_name` | Username | "m318358" |
| `group_name` | UNIX group | "merckdefault" |
| `account` | Slurm account | "admin" |
| `partition` | Job partition | "gpu" |
| `job_state` | Array of job states | ["RUNNING"] |
| `nodes` | Node list (may need expansion) | "demu4xgpu001" or "node[001-010]" |
| `tres_alloc_str` | Allocated resources | "cpu=2,mem=4G,gres/gpu=1" |
| `start_time.number` | Start timestamp | 1755290346 |
| `end_time.number` | End timestamp | 1755290406 |
| `array_job_id.number` | Array job ID (0 if not array) | 0 |

## Key Findings

### 1. Node States
- States are provided as an array of strings
- Common states: "IDLE", "MIXED", "ALLOCATED", "DOWN", "DRAIN"
- Multiple states can apply simultaneously: ["IDLE", "DRAIN"]

### 2. CPU Load Format
- CPU load is stored as an integer (×100)
- Example: `234` represents a load of `2.34`
- Need to divide by 100 for display

### 3. Memory Values
- All memory values are in MB
- Free memory is nested in a structure with `set`, `infinite`, and `number` fields
- Allocated memory is a direct integer value

### 4. GRES Information
- GRES (Generic Resources) stored as formatted strings
- Format: "resource:type:count(details)"
- Example: "gpu:l40s:4(S:0-1)" means 4 L40S GPUs on sockets 0-1
- Used GRES shows allocated resources with indices

### 5. Timestamp Format
- Times are Unix timestamps (seconds since epoch)
- Wrapped in objects with `set`, `infinite`, and `number` fields
- Time limits are in minutes (not seconds)

### 6. Node Name Expansion
- Node lists may be compressed: "node[001-010]"
- Use `scontrol show hostnames` for expansion
- Single nodes are provided as simple strings

### 7. TRES Allocation String
- Comma-separated key=value pairs
- Includes CPU, memory, node count, billing, GRES
- Memory can be in GB (4G) or MB format
- Parse required to extract GPU information

## Usage Examples

### Getting Node Information
```bash
# Get all nodes with details
sinfo -N --json

# Get specific partition
sinfo -N -p gpu --json

# Get specific states
sinfo -N -t idle,mixed --json
```

### Getting Job Information
```bash
# Get all running jobs
squeue --json -t RUNNING

# Get jobs for specific user
squeue --json -u username

# Get jobs in specific partition
squeue --json -p gpu
```

### Parsing with jq

#### Extract node names and states
```bash
sinfo -N --json | jq '.sinfo[] | {node: .nodes.nodes[0], state: .node.state}'
```

#### Extract job IDs and users
```bash
squeue --json | jq '.jobs[] | {id: .job_id, user: .user_name, nodes: .nodes}'
```

#### Get nodes with high CPU load
```bash
sinfo -N --json | jq '.sinfo[] | select(.cpus.load.minimum > 400) | .nodes.nodes[0]'
```

#### Count GPUs in use
```bash
squeue --json | jq '[.jobs[].tres_alloc_str | capture("gres/gpu=(?<gpu>[0-9]+)") | .gpu | tonumber] | add'
```

## Python Parsing Notes

### Handling Nested Values
```python
# For values with set/infinite/number structure
def extract_value(obj):
    if isinstance(obj, dict) and 'number' in obj:
        return obj['number'] if obj.get('set', False) else None
    return obj

# Example usage
free_memory = extract_value(node['memory']['free']['minimum'])
```

### Converting CPU Load
```python
# CPU load is stored as integer (×100)
cpu_load = node['cpus']['load']['minimum'] / 100.0
```

### Parsing TRES String
```python
def parse_tres(tres_str):
    """Parse TRES allocation string into dictionary"""
    result = {}
    for item in tres_str.split(','):
        if '=' in item:
            key, value = item.split('=', 1)
            result[key] = value
    return result

# Example
tres = parse_tres("cpu=2,mem=4G,gres/gpu=1")
# {'cpu': '2', 'mem': '4G', 'gres/gpu': '1'}
```

### State Checking
```python
def has_state(node, state):
    """Check if node has a specific state"""
    return state in node.get('node', {}).get('state', [])

# Example
is_idle = has_state(node_data, 'IDLE')
```

## Advantages of JSON Format

1. **Structured Data**: No regex or text parsing needed
2. **Type Information**: Numbers are actual numbers, not strings
3. **Consistent Format**: Same structure regardless of cluster configuration
4. **Complete Information**: All fields available, even if empty
5. **Array Support**: Multiple states/flags properly represented
6. **Error Handling**: Errors and warnings in separate fields
7. **Metadata**: Additional context in meta field

## Limitations and Workarounds

1. **Node Expansion**: Compressed node lists still need expansion
   - Solution: Use `scontrol show hostnames` or Python library

2. **CPU Load Format**: Stored as integer ×100
   - Solution: Simple division by 100

3. **Memory Units**: Always in MB, may need conversion for display
   - Solution: Format function for GB/TB display

4. **TRES Parsing**: Still a string format
   - Solution: Simple string parsing function

5. **Timestamp Format**: Unix timestamps need conversion
   - Solution: Use datetime.fromtimestamp()