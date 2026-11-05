"""
Problem definitions for Containerization Training Domain.
-- Generated 2026-08-06

This file defines initial states for cont-train workflows.
The workflow demonstrates coordination with the RIKYU HPC MCP server:
  - rikyu-hpc: GPU resource management, job scheduling, file transfer

Scenarios:
  - baseline_mdlm_training: Single-GPU training of baseline MDLM model -> ~7 actions
  - distributed_mdlm_training: Multi-GPU distributed training of MDLM model -> ~7 actions
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

def h_create_initial_state(name: str, model_name: str = "", gpu_count: int = 1) -> State:
    """
    Helper function to create initial state for containerized training.

    Args:
        name: State name identifier
        model_name: Name of the MDLM diffusion model
        gpu_count: Number of GPUs to request

    Returns:
        Configured gtpyhop.State object with all required properties
    """
    state = State(name)

    # Domain-specific input
    state.model_name = model_name
    state.gpu_count = gpu_count

    # DATA containers (all start empty)
    state.facility_config = {}
    state.resource_status = {}
    state.working_dir = ""
    state.job_id = ""
    state.job_status = {}

    # ENABLER flags (all start False)
    state.facility_retrieved = False
    state.resource_checked = False
    state.environment_ready = False
    state.code_uploaded = False
    state.job_submitted = False
    state.job_completed = False
    state.trained_model_retrieved = False

    # Execution tracking
    state.servers_called = []
    state.api_calls = 0

    return state


# =============================================================================
# SCENARIOS
# =============================================================================

problems: Dict[str, Tuple[State, List[Tuple], str]] = {}

# BEGIN: Domain: cont-train

# BEGIN: Scenario: baseline_mdlm_training
# Configuration
_model = "mdlm-baseline"
_gpus = 1

# State
initial_state_baseline = h_create_initial_state('baseline_mdlm_training', _model, _gpus)

# Problem
problems['baseline_mdlm_training'] = (
    initial_state_baseline,
    [('containerized_training_workflow', _model, _gpus)],
    f"""Baseline MDLM Diffusion Model Training

Train a baseline MDLM diffusion model on a single GPU. This scenario verifies the complete
containerized training pipeline: facility discovery, resource checking, environment setup,
code upload, job submission, monitoring, and model retrieval. Demonstrates end-to-end
orchestration for containerized model training on RIKYU's GPU partition."""
)
# END: Scenario

# BEGIN: Scenario: distributed_mdlm_training
# Configuration
_model = "mdlm-distributed"
_gpus = 4

# State
initial_state_distributed = h_create_initial_state('distributed_mdlm_training', _model, _gpus)

# Problem
problems['distributed_mdlm_training'] = (
    initial_state_distributed,
    [('containerized_training_workflow', _model, _gpus)],
    f"""Distributed MDLM Diffusion Model Training

Train an MDLM diffusion model across 4 GPUs using distributed training. This scenario tests
multi-GPU resource allocation, parallel job execution, and coordination across multiple
compute nodes. Verifies that the containerization training pipeline scales to distributed
settings with proper GPU request validation and resource availability checks."""
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
