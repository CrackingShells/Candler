"""
Containerization Training Domain - GTPyhop Domain for MDLM Diffusion Model Training.

Domain: cont-train
Version: 1.0.0
MCP Servers: rikyu-hpc (RIKYU HPC cluster interface)

Actions (7):
    - a_get_facility_info: Get facility capabilities (rikyu-hpc)
    - a_check_resource_availability: Check GPU resource occupancy (rikyu-hpc)
    - a_prepare_training_environment: Setup directories and permissions (rikyu-hpc)
    - a_upload_training_code: Upload model and training scripts to cluster (rikyu-hpc)
    - a_submit_training_job: Submit containerized training job to Slurm scheduler (rikyu-hpc)
    - a_monitor_job_progress: Poll job status and retrieve logs (rikyu-hpc)
    - a_retrieve_trained_model: Download trained model and artifacts (rikyu-hpc)

Methods (2):
    - m_containerized_training_workflow: Top-level orchestration for containerized training
    - m_training_completion: Handle job monitoring and artifact retrieval
"""

import sys
import os
from typing import Optional, Union, List, Tuple, Dict, Any

# Secure GTPyhop import strategy
try:
    import gtpyhop
    from gtpyhop import Domain, State, set_current_domain, declare_actions, declare_task_methods
except ImportError:
    try:
        sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
        import gtpyhop
        from gtpyhop import Domain, State, set_current_domain, declare_actions, declare_task_methods
    except ImportError as e:
        print(f"Error: Could not import gtpyhop: {e}")
        print("Please install gtpyhop using: pip install gtpyhop")
        sys.exit(1)


# =============================================================================
# DOMAIN
# =============================================================================
the_domain = Domain("cont-train")
set_current_domain(the_domain)
DOMAIN_NAME = "cont-train"


# =============================================================================
# STATE PROPERTY MAP
# =============================================================================
# Legend:
#  - (E) Created/modified by the action (Effects)
#  - (P) Consumed/checked by the action (Preconditions/State checks)
#  - [ENABLER] Property acts as a workflow gate for subsequent steps
#  - [DATA]    Informational/data container
#
# Step 1: a_get_facility_info
#  (E) facility_config: dict [DATA] - RIKYU facility facts (GPU partitions, modules)
#  (E) facility_retrieved: bool [ENABLER] - gates a_check_resource_availability
#
# Step 2: a_check_resource_availability
#  (P) facility_retrieved: bool [ENABLER]
#  (E) resource_status: dict [DATA] - live GPU occupancy/availability
#  (E) resource_checked: bool [ENABLER] - gates a_prepare_training_environment
#
# Step 3: a_prepare_training_environment
#  (P) resource_checked: bool [ENABLER]
#  (E) working_dir: str [DATA] - remote working directory path
#  (E) environment_ready: bool [ENABLER] - gates a_upload_training_code
#
# Step 4: a_upload_training_code
#  (P) environment_ready: bool [ENABLER]
#  (E) code_uploaded: bool [ENABLER] - gates a_submit_training_job
#
# Step 5: a_submit_training_job
#  (P) code_uploaded: bool [ENABLER]
#  (E) job_id: str [DATA] - Slurm job ID
#  (E) job_submitted: bool [ENABLER] - gates m_training_completion
#
# Step 6: m_training_completion (method)
#  (P) job_submitted: bool [ENABLER]
#  (E) job_completed: bool [ENABLER]
#  (E) trained_model_retrieved: bool [ENABLER]
#
# Substeps in m_training_completion:
#  - a_monitor_job_progress: polls job status, retrieves logs
#  - a_retrieve_trained_model: downloads artifacts
# =============================================================================


# =============================================================================
# BEGIN: Actions
# =============================================================================

def a_get_facility_info(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_facility

    Action signature:
        a_get_facility_info(state)

    Action parameters:
        None (queries facility info from cluster)

    Action purpose:
        Retrieve RIKYU facility configuration: GPU partitions, node specs, storage tiers, modules

    Preconditions:
        - None (first action - entry point)

    Effects:
        - Facility config stored (state.facility_config) [DATA]
        - Facility retrieval flag (state.facility_retrieved) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # No state preconditions - this is the first action
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store facility configuration (structure populated by MCP at runtime)
    state.facility_config = {
        "facility": "rikyu",
        "partitions": [],  # Populated by MCP server
        "modules": [],     # Populated by MCP server
        "storage_tiers": [] # Populated by MCP server
    }

    # [ENABLER] Gates a_check_resource_availability
    state.facility_retrieved = True
    # END: Effects

    return state


def a_check_resource_availability(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_resources

    Action signature:
        a_check_resource_availability(state)

    Action parameters:
        None (queries live resource status)

    Action purpose:
        Check live GPU partition occupancy and availability status

    Preconditions:
        - Facility info must be retrieved (state.facility_retrieved)

    Effects:
        - Resource status stored (state.resource_status) [DATA]
        - Resource availability flag (state.resource_checked) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_retrieved') and state.facility_retrieved):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store resource occupancy info (populated by MCP at runtime)
    state.resource_status = {
        "partition": "gpu",
        "allocated": 0,  # Populated by MCP
        "idle": 0,       # Populated by MCP
        "total": 0       # Populated by MCP
    }

    # [ENABLER] Gates a_prepare_training_environment
    state.resource_checked = True
    # END: Effects

    return state


def a_prepare_training_environment(state: State, project_name: str = "training") -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_mkdir, rikyu-hpc:fs_chmod

    Action signature:
        a_prepare_training_environment(state, project_name)

    Action parameters:
        project_name: Name identifier for training project (default: "training")

    Action purpose:
        Create remote working directory and set appropriate permissions for training

    Preconditions:
        - Resource availability must be checked (state.resource_checked)

    Effects:
        - Working directory path stored (state.working_dir) [DATA]
        - Environment readiness flag (state.environment_ready) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(project_name, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not project_name.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'resource_checked') and state.resource_checked):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Set working directory path (created by MCP at runtime)
    state.working_dir = f"~/agent/training/{project_name}"

    # [ENABLER] Gates a_upload_training_code
    state.environment_ready = True
    # END: Effects

    return state


def a_upload_training_code(state: State, code_archive: str = "model.tar.gz") -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_upload

    Action signature:
        a_upload_training_code(state, code_archive)

    Action parameters:
        code_archive: Local path to training code/model archive

    Action purpose:
        Upload containerized model and training scripts to the cluster

    Preconditions:
        - Training environment must be prepared (state.environment_ready)

    Effects:
        - Code upload completion flag (state.code_uploaded) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(code_archive, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not code_archive.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'environment_ready') and state.environment_ready):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [ENABLER] Gates a_submit_training_job
    state.code_uploaded = True
    # END: Effects

    return state


def a_submit_training_job(state: State, gpu_count: int = 1, wall_time: str = "01:00:00") -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:submit_job

    Action signature:
        a_submit_training_job(state, gpu_count, wall_time)

    Action parameters:
        gpu_count: Number of GPUs to request (must be in RIKYU's supported list)
        wall_time: Maximum wall time for the job (HH:MM:SS format)

    Action purpose:
        Submit the containerized training job to RIKYU's Slurm scheduler

    Preconditions:
        - Training code must be uploaded (state.code_uploaded)

    Effects:
        - Job ID stored (state.job_id) [DATA]
        - Job submission flag (state.job_submitted) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(gpu_count, int):
        return False
    if not isinstance(wall_time, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if gpu_count < 1:
        return False
    if not wall_time.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'code_uploaded') and state.code_uploaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store job ID (returned by MCP at runtime)
    state.job_id = f"job_id_for_{gpu_count}_gpus"

    # [ENABLER] Gates m_training_completion
    state.job_submitted = True
    # END: Effects

    return state


def a_monitor_job_progress(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_job_status, rikyu-hpc:fs_tail

    Action signature:
        a_monitor_job_progress(state)

    Action parameters:
        None (uses state.job_id)

    Action purpose:
        Poll job status and retrieve training progress logs

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must be available (state.job_id)

    Effects:
        - Job status stored (state.job_status) [DATA]
        - Job completion flag (state.job_completed) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store job status (populated by MCP at runtime)
    state.job_status = {
        "job_id": state.job_id,
        "state": "RUNNING",  # Populated by MCP
        "progress": 0,       # Populated by MCP
        "logs": []          # Populated by MCP
    }

    # [ENABLER] Gates a_retrieve_trained_model
    state.job_completed = True
    # END: Effects

    return state


def a_retrieve_trained_model(state: State, local_output: str = ".") -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_download, rikyu-hpc:fs_checksum

    Action signature:
        a_retrieve_trained_model(state, local_output)

    Action parameters:
        local_output: Local directory where trained model will be downloaded

    Action purpose:
        Download trained model and artifacts from the cluster with verification

    Preconditions:
        - Job must be completed (state.job_completed)

    Effects:
        - Model retrieval completion flag (state.trained_model_retrieved) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(local_output, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not local_output.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [ENABLER] Marks successful model retrieval
    state.trained_model_retrieved = True
    # END: Effects

    return state

# =============================================================================
# END: Actions
# =============================================================================


# =============================================================================
# BEGIN: Methods
# =============================================================================

def m_containerized_training_workflow(state: State, model_name: str, gpu_count: int = 1) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_containerized_training_workflow(state, model_name, gpu_count)

    Method parameters:
        model_name: Name of the MDLM diffusion model to train
        gpu_count: Number of GPUs to request for training

    Method purpose:
        Top-level orchestration for containerized MDLM diffusion model training on RIKYU

    Preconditions:
        - model_name is a non-empty string
        - gpu_count is a positive integer

    Task decomposition:
        - a_get_facility_info: Query RIKYU facility configuration
        - a_check_resource_availability: Check GPU availability
        - a_prepare_training_environment: Setup working directory
        - a_upload_training_code: Upload model and scripts
        - a_submit_training_job: Submit training job to Slurm
        - training_completion: Monitor and retrieve results

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(model_name, str):
        return False
    if not isinstance(gpu_count, int):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not model_name.strip():
        return False
    if gpu_count < 1:
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions - this is the entry point method
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_get_facility_info",),
        ("a_check_resource_availability",),
        ("a_prepare_training_environment", model_name),
        ("a_upload_training_code", f"model_{model_name}.tar.gz"),
        ("a_submit_training_job", gpu_count),
        ("training_completion",),
    ]
    # END: Task Decomposition


def m_training_completion(state: State) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_training_completion(state)

    Method parameters:
        None (uses state.job_id and state.working_dir)

    Method purpose:
        Handle job monitoring and trained model artifact retrieval

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must be available (state.job_id)

    Task decomposition:
        - a_monitor_job_progress: Poll job status until completion
        - a_retrieve_trained_model: Download trained model and logs

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_monitor_job_progress",),
        ("a_retrieve_trained_model", "${context.working_dir}"),
    ]
    # END: Task Decomposition

# =============================================================================
# END: Methods
# =============================================================================


# =============================================================================
# DECLARE ACTIONS TO DOMAIN
# =============================================================================
declare_actions(
    a_get_facility_info,
    a_check_resource_availability,
    a_prepare_training_environment,
    a_upload_training_code,
    a_submit_training_job,
    a_monitor_job_progress,
    a_retrieve_trained_model,
)


# =============================================================================
# DECLARE METHODS TO DOMAIN
# =============================================================================
declare_task_methods("containerized_training_workflow", m_containerized_training_workflow)
declare_task_methods("training_completion", m_training_completion)
