#!/bin/bash

# Test job submission script for cmon development
# Usage: ./submit_test_jobs.sh [number_of_jobs] [partition_type]
# Examples:
#   ./submit_test_jobs.sh 5         # 5 mixed jobs across all partitions
#   ./submit_test_jobs.sh 3 cpu     # 3 CPU jobs only
#   ./submit_test_jobs.sh 2 gpu     # 2 GPU jobs only
#   ./submit_test_jobs.sh 4 fat     # 4 Fat jobs only

set -e

# Default values
NUM_JOBS=${1:-5}
PARTITION_TYPE=${2:-mixed}

# Creative short job names for testing dynamic column width
JOB_NAMES=(
    "spark"
    "nova"
    "zen"
    "flux"
    "echo"
    "wave"
    "bolt"
    "fire"
    "ice"
    "wind"
    "star"
    "moon"
    "sun"
    "ray"
    "glow"
    "dash"
    "rush"
    "flow"
    "peak"
    "edge"
)

# Longer creative names for additional testing
LONG_JOB_NAMES=(
    "neural_network_training"
    "molecular_dynamics_sim"
    "climate_model_forecast"
    "protein_folding_analysis"
    "quantum_computation_test"
    "image_processing_pipeline"
    "deep_learning_experiment"
    "bioinformatics_workflow"
    "computational_fluid_dynamics"
    "monte_carlo_simulation"
)

# Function to get a random job name
get_job_name() {
    local index=$((RANDOM % ${#JOB_NAMES[@]}))
    echo "${JOB_NAMES[$index]}"
}

# Function to get a random long job name
get_long_job_name() {
    local index=$((RANDOM % ${#LONG_JOB_NAMES[@]}))
    echo "${LONG_JOB_NAMES[$index]}"
}

# Function to submit a job with error/output redirection
submit_job() {
    local job_name="$1"
    local script="$2"
    local partition="$3"
    
    echo "Submitting $partition job: $job_name"
    
    if [[ "$partition" == "cpu" ]]; then
        sbatch -J "$job_name" -o /dev/null -e /dev/null ~/sleep.sh
    elif [[ "$partition" == "gpu" ]]; then
        sbatch -J "$job_name" -o /dev/null -e /dev/null ~/sleep_gpu.sh
    elif [[ "$partition" == "fat" ]]; then
        sbatch -J "$job_name" -o /dev/null -e /dev/null ~/sleep_fat.sh
    else
        echo "Unknown partition type: $partition"
        return 1
    fi
}

echo "=== Test Job Submission Script ==="
echo "Submitting $NUM_JOBS jobs (type: $PARTITION_TYPE)"
echo "Output and error files redirected to /dev/null"
echo

# Validate partition type
case $PARTITION_TYPE in
    cpu|gpu|fat|mixed)
        ;;
    *)
        echo "Error: Invalid partition type '$PARTITION_TYPE'"
        echo "Valid types: cpu, gpu, fat, mixed"
        exit 1
        ;;
esac

# Submit jobs
for i in $(seq 1 $NUM_JOBS); do
    # Mix of short and long names for variety
    if [[ $((RANDOM % 3)) == 0 ]]; then
        job_name=$(get_long_job_name)
    else
        job_name=$(get_job_name)
    fi
    
    # Add some variety with numbered jobs
    if [[ $((RANDOM % 4)) == 0 ]]; then
        job_name="${job_name}_${i}"
    fi
    
    # Determine partition
    case $PARTITION_TYPE in
        mixed)
            # Randomly choose partition for mixed mode
            partitions=("cpu" "gpu" "fat")
            partition_choice=$((RANDOM % 3))
            partition=${partitions[$partition_choice]}
            ;;
        *)
            partition=$PARTITION_TYPE
            ;;
    esac
    
    # Submit the job
    if submit_job "$job_name" "sleep.sh" "$partition"; then
        echo "  ✓ Job submitted successfully"
    else
        echo "  ✗ Failed to submit job"
    fi
    
    # Small delay to avoid overwhelming the scheduler
    sleep 0.1
done

echo
echo "=== Summary ==="
echo "Submitted $NUM_JOBS jobs of type: $PARTITION_TYPE"
echo
echo "To view the jobs:"
echo "  uv run cmon jobs --all    # All jobs including pending"
echo "  uv run cmon jobs          # Running jobs only"
echo "  squeue --me               # Raw squeue output"
echo
echo "To test different layouts:"
echo "  uv run cmon jobs --compact        # Narrow layout"
echo "  uv run cmon jobs --columns=Name   # Name column only"
echo
echo "The dynamic Name column should adapt to the longest job name in the display!"