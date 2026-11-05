"""
Problem definitions for Containerized MDLM Diffusion Model Training.
-- Generated 2026-08-06

This file defines initial states for cont-train workflows.
The workflow demonstrates coordination with the RIKYU HPC cluster:
  - rikyu-hpc: GPU job submission, monitoring, filesystem operations, and resource management

Scenarios:
  - basic_training: Standard training run for MDLM model on 4 GPUs -> ~10 actions
  - extended_training: Extended training with checkpoint monitoring on 8 GPUs -> ~10 actions
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

def h_create_initial_state(name: str, model_name: str = "") -> State:
    """
    Helper function to create initial state for cont-train.

    Args:
        name: State name identifier
        model_name: Model identifier for the training job (optional)

    Returns:
        Configured gtpyhop.State object with all required properties
    """
    state = State(name)

    # Domain-specific input
    state.model_name = model_name if model_name else f"mdlm_{name}"

    # DATA containers (all start empty)
    state.facility_info = {}
    state.drained_nodes = []
    state.resource_status = {}
    state.job_path = ""
    state.training_script_path = ""
    state.job_id = ""
    state.job_status = {}
    state.stdout_content = ""
    state.artifacts_verified = {}
    state.checkpoint_path = ""
    state.cleanup_status = ""

    # ENABLER flags (all start False)
    state.facility_loaded = False
    state.resource_checked = False
    state.job_dir_ready = False
    state.script_ready = False
    state.job_submitted = False
    state.job_completed = False
    state.logs_retrieved = False
    state.artifacts_ready = False
    state.download_complete = False
    state.workflow_complete = False

    # Execution tracking
    state.servers_called = []
    state.api_calls = 0

    return state


# =============================================================================
# SCENARIOS
# =============================================================================

problems: Dict[str, Tuple[State, List[Tuple], str]] = {}

# BEGIN: Domain: cont-train

# BEGIN: Scenario: basic_training
# Configuration
_model_name = "mdlm_v1_phase2_4gpu"

# State
initial_state_basic = h_create_initial_state('basic_training', _model_name)

# Problem
problems['basic_training'] = (
    initial_state_basic,
    [('containerized_training_workflow', _model_name)],
    f"""Basic MDLM Training on 4 GPUs

This scenario demonstrates a standard containerized training run for a specialized
MDLM diffusion model on RIKYU's GPU cluster. The workflow submits a 4-GPU job,
monitors its execution, retrieves logs, verifies output artifacts, downloads the
trained checkpoint, and cleans up the temporary workspace.

Expected pipeline:
  1. Load facility configuration and check resource availability
  2. Identify drained nodes to avoid scheduling issues
  3. Query GPU partition occupancy
  4. Create job workspace directory
  5. Prepare containerized training sbatch script
  6. Submit job to Slurm scheduler
  7. Monitor job status until completion
  8. Retrieve stdout/stderr logs for verification
  9. Verify expected output artifacts exist
 10. Download trained model checkpoint to local storage
 11. Clean up temporary job workspace"""
)
# END: Scenario

# BEGIN: Scenario: extended_training
# Configuration
_model_name = "mdlm_v2_extended_8gpu"

# State
initial_state_extended = h_create_initial_state('extended_training', _model_name)

# Problem
problems['extended_training'] = (
    initial_state_extended,
    [('containerized_training_workflow', _model_name)],
    f"""Extended MDLM Training with Checkpoint Monitoring (8 GPUs)

This scenario represents a longer, more resource-intensive training run that
leverages 8 GPUs for accelerated diffusion model training. The workflow
coordinates with RIKYU's HPC infrastructure to manage larger resource
allocation, monitor ongoing execution, ensure successful completion,
and safely retrieve the trained model checkpoint.

Differences from basic_training:
  - Larger GPU allocation (8 vs 4)
  - Extended time limits for convergence
  - More comprehensive artifact verification
  - Higher computational requirements

Expected pipeline (same structure as basic_training, adapted for 8-GPU resources):
  1. Load facility configuration and check availability of 8 GPUs
  2. Check for unavailable compute nodes
  3. Monitor GPU partition occupancy for scheduling window
  4. Create workspace for extended training
  5. Prepare containerized training script with 8-GPU resource request
  6. Submit job with extended time allocation
  7. Monitor job status throughout extended execution
  8. Retrieve detailed training logs and metrics
  9. Verify comprehensive artifact set (checkpoints, metrics, logs)
 10. Download final model checkpoint
 11. Archive and clean up job workspace"""
)
# END: Scenario

# END: Domain


def get_problems() -> Dict[str, Tuple[State, List[Tuple], str]]:
    """
    Return all problem definitions for benchmarking.

    Returns:
        Dictionary mapping problem IDs to (state, tasks, description) tuples.
    """
    return problems
