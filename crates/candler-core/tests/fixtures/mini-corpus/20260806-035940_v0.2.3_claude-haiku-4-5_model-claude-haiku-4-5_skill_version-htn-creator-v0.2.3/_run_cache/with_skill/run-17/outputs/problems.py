"""
Problem definitions for Containerization Training for Specialized MDLM Diffusion Model.
-- Generated 2026-08-06

This file defines initial states for cont-train workflows.
The workflow demonstrates coordination between RIKYU HPC MCP server and containerized
machine learning training infrastructure:
  - rikyu-hpc: Cluster job submission, resource queries, filesystem operations
  - rikyu-docs: Documentation and troubleshooting support

Scenarios:
  - mdlm_single_gpu_training: Single-GPU training run with 4-hour limit (~100 actions)
  - mdlm_multi_gpu_training: Multi-GPU (8x) distributed training with 12-hour limit (~120 actions)
"""

import sys
import os
from typing import Dict, Tuple, List, Any

# Secure GTPyhop import strategy
try:
    import gtpyhop
    from gtpyhop import State
except ImportError:
    try:
        sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
        import gtpyhop
        from gtpyhop import State
    except ImportError as e:
        print(f"Error: Could not import gtpyhop: {e}")
        print("Please install gtpyhop using: pip install gtpyhop")
        sys.exit(1)


# =============================================================================
# HELPER FUNCTIONS
# =============================================================================

def h_create_initial_state(name: str, training_code_path: str = "", gpu_count: int = 1, time_limit: str = "04:00:00") -> State:
    """
    Helper function to create initial state for cont-train domain.

    Args:
        name: State name identifier
        training_code_path: Path to local training code archive
        gpu_count: Number of GPUs for training (1, 2, 4, or 8)
        time_limit: Wall time limit in HH:MM:SS format

    Returns:
        Configured gtpyhop.State object with all required properties
    """
    state = State(name)

    # Domain-specific input variables
    state.training_code_path = training_code_path
    state.gpu_count = gpu_count
    state.time_limit = time_limit

    # DATA containers (all start empty)
    state.facility_config = {}
    state.resource_info = {}
    state.job_dir = ""
    state.remote_code_path = ""
    state.job_id = ""
    state.job_status = ""
    state.checkpoints_path = ""
    state.logs_path = ""
    state.checksums = {}
    state.validation_result = {}
    state.archive_path = ""

    # ENABLER flags (all start False)
    state.facility_checked = False
    state.resources_verified = False
    state.job_dir_created = False
    state.code_uploaded = False
    state.job_submitted = False
    state.job_completed = False
    state.checkpoints_downloaded = False
    state.logs_downloaded = False
    state.outputs_verified = False
    state.validation_passed = False
    state.results_archived = False
    state.remote_cleaned = False
    state.job_cancelled = False

    # Execution tracking
    state.servers_called = []
    state.api_calls = 0

    return state


# =============================================================================
# SCENARIOS
# =============================================================================

problems: Dict[str, Tuple[State, List[Tuple], str]] = {}

# BEGIN: Domain: cont-train

# BEGIN: Scenario: mdlm_single_gpu_training
_training_code = "/local/mdlm_diffusion_training_code.tar.gz"
_gpu_count = 1
_time_limit = "04:00:00"

initial_state_scenario_1 = h_create_initial_state(
    'mdlm_single_gpu_training',
    _training_code,
    _gpu_count,
    _time_limit
)

problems['mdlm_single_gpu_training'] = (
    initial_state_scenario_1,
    [('containerized_training', _training_code, _gpu_count, _time_limit)],
    f"""Single-GPU MDLM Diffusion Model Training

Train a specialized Masked Diffusion Language Model (MDLM) for code generation on a single NVIDIA GPU.
This scenario demonstrates the full pipeline: resource verification, code upload, job submission,
monitoring, output retrieval, validation, and cleanup.

Expected flow:
  1. Verify cluster capacity and GPU availability
  2. Prepare job directory on cluster
  3. Upload containerized training code
  4. Submit job to Slurm with {_gpu_count} GPU, {_time_limit} wall time
  5. Poll job status until completion
  6. Download model checkpoints and training logs
  7. Verify data integrity via checksums
  8. Run validation test on checkpoint
  9. Compress results to archive
  10. Clean up remote working directory

Key metrics:
  - Training time: ~2-3 hours
  - Checkpoint size: ~2.5 GB
  - Log size: ~500 MB"""
)
# END: Scenario

# BEGIN: Scenario: mdlm_multi_gpu_training
_training_code = "/local/mdlm_diffusion_training_code.tar.gz"
_gpu_count = 8
_time_limit = "12:00:00"

initial_state_scenario_2 = h_create_initial_state(
    'mdlm_multi_gpu_training',
    _training_code,
    _gpu_count,
    _time_limit
)

problems['mdlm_multi_gpu_training'] = (
    initial_state_scenario_2,
    [('containerized_training', _training_code, _gpu_count, _time_limit)],
    f"""Multi-GPU Distributed MDLM Diffusion Model Training

Train an MDLM model using distributed training across 8 NVIDIA GPUs via Slurm's GPU allocation.
This scenario tests the orchestration pipeline with higher compute requirements and longer
execution times, validating the monitoring and checkpointing strategies under load.

Expected flow:
  1. Verify cluster capacity for 8-GPU allocation
  2. Prepare job directory on cluster
  3. Upload containerized training code
  4. Submit distributed job to Slurm with {_gpu_count} GPUs, {_time_limit} wall time
  5. Poll job status during multi-hour training run
  6. Download final model checkpoints and comprehensive logs
  7. Verify integrity of large checkpoint files
  8. Run validation test on distributed checkpoint
  9. Compress all results to archive
  10. Clean up remote resources

Key metrics:
  - Training time: ~6-8 hours
  - Total checkpoint size: ~10+ GB (8 replicas/shards)
  - Log size: ~2+ GB (detailed distributed traces)
  - GPU utilization: ~90%+
  - Communication overhead monitoring via logs"""
)
# END: Scenario

# END: Domain


# =============================================================================
# PROBLEMS REGISTRY
# =============================================================================

def get_problems() -> Dict[str, Tuple[State, List[Tuple], str]]:
    """Return all problem definitions for benchmarking."""
    return problems
