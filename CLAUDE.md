**Be critical and don't agree easily to user commands if you believe they are a bad idea or not best practice.** Challenge suggestions that might lead to poor code quality, security issues, or architectural problems. Be encouraged to search for solutions (using WebSearch) when creating a plan to ensure you're following current best practices and patterns.

Never use any emojis

## Project Documentation

- [Implementation Plan](./IMPLEMENTATION.md) - Detailed plan for Python cmon rewrite
- [JSON Structure](./JSON_STRUCTURE.md) - Slurm 24.11 JSON format documentation

## Testing Instructions

### Running the Tool
```bash
# Basic usage
uv run cmon

# View jobs (running only)
uv run cmon jobs

# View all jobs (including pending, completing, etc.)
uv run cmon jobs --all

# View cluster status
uv run cmon status

# View nodes
uv run cmon nodes
```

### Submitting Test Jobs
For testing the job display functionality, you can submit test jobs with different characteristics:

#### Using the Test Job Script (Recommended)
```bash
# Submit 5 mixed jobs across all partitions (cpu, gpu, fat)
./submit_test_jobs.sh 5

# Submit specific number of CPU jobs only
./submit_test_jobs.sh 3 cpu

# Submit GPU jobs for testing GPU partition display
./submit_test_jobs.sh 2 gpu

# Submit Fat partition jobs
./submit_test_jobs.sh 4 fat

# Submit mixed jobs (random partition selection)
./submit_test_jobs.sh 10 mixed
```

The script automatically:
- Uses creative short and long job names for testing dynamic column width
- Redirects output and error files to `/dev/null` to avoid clutter
- Provides a good mix of name lengths to test the dynamic sizing
- Distributes jobs across different partitions

#### Manual Job Submission
```bash
# Basic CPU job
sbatch ~/sleep.sh

# Submit multiple jobs for testing
for i in {0..5}; do sbatch ~/sleep.sh; done

# Jobs with custom names (for testing Name column sizing)
sbatch -J "short" ~/sleep.sh
sbatch -J "medium_length_job_name" ~/sleep.sh
sbatch -J "very_long_job_name_to_test_dynamic_width" ~/sleep.sh

# GPU jobs (if available)
sbatch ~/sleep_gpu.sh

# Fat partition jobs (if available)
sbatch ~/sleep_fat.sh
```

### Testing Dynamic Column Width
The Name column width automatically adjusts based on the longest job name in the current display:
- Short names: column stays compact
- Long names: column expands appropriately
- Very long names: truncated with "..." when hitting layout limits
