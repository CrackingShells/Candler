"""
Containerized MDLM Diffusion Model Training on GPU - GTPyhop Domain.

Domain: cont-train
Version: 1.0.0
MCP Servers: rikyu-hpc

Actions (11):
    - a_check_facility_info: Get facility configuration and resource availability (rikyu-hpc)
    - a_get_resource_status: Check GPU partition occupancy and node status (rikyu-hpc)
    - a_create_job_directory: Create training job workspace directory (rikyu-hpc)
    - a_prepare_training_script: Prepare containerized training job script (rikyu-hpc)
    - a_submit_training_job: Submit training job to GPU scheduler (rikyu-hpc)
    - a_monitor_job_status: Poll job status during execution (rikyu-hpc)
    - a_retrieve_job_logs: Fetch stdout/stderr from running/completed job (rikyu-hpc)
    - a_verify_output_artifacts: Check for training output files and metrics (rikyu-hpc)
    - a_download_model_checkpoint: Download trained model to local storage (rikyu-hpc)
    - a_cleanup_job_workspace: Remove temporary training files (rikyu-hpc)
    - a_check_drained_nodes: Identify unavailable compute resources (rikyu-hpc)

Methods (2):
    - m_containerized_training_workflow: Top-level orchestration for model training pipeline
    - m_monitor_and_retrieve_logs: Sub-workflow for status polling and log retrieval
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
# Step 1: a_check_facility_info
#  (P) model_name: str [DATA] - name/identifier for the training job
#  (E) facility_info: dict [DATA] - cluster configuration and capabilities
#  (E) facility_loaded: bool [ENABLER] - gates resource status check
#
# Step 2: a_get_resource_status
#  (P) facility_loaded: bool [ENABLER]
#  (E) resource_status: list [DATA] - current GPU partition occupancy
#  (E) resource_checked: bool [ENABLER] - gates job directory creation
#
# Step 3: a_create_job_directory
#  (P) resource_checked: bool [ENABLER]
#  (E) job_path: str [DATA] - created workspace directory path
#  (E) job_dir_ready: bool [ENABLER] - gates training script preparation
#
# Step 4: a_prepare_training_script
#  (P) job_dir_ready: bool [ENABLER]
#  (E) training_script_path: str [DATA] - path to prepared job script
#  (E) script_ready: bool [ENABLER] - gates job submission
#
# Step 5: a_submit_training_job
#  (P) script_ready: bool [ENABLER]
#  (E) job_id: str [DATA] - Slurm job identifier
#  (E) job_submitted: bool [ENABLER] - gates job monitoring
#
# Step 6: a_monitor_job_status (looped)
#  (P) job_submitted: bool [ENABLER]
#  (E) job_status: dict [DATA] - current job state from Slurm
#  (E) job_completed: bool [ENABLER] - gates log retrieval
#
# Step 7: a_retrieve_job_logs
#  (P) job_completed: bool [ENABLER]
#  (E) stdout_content: str [DATA] - job output log
#  (E) logs_retrieved: bool [ENABLER] - gates artifact verification
#
# Step 8: a_verify_output_artifacts
#  (P) logs_retrieved: bool [ENABLER]
#  (E) artifacts_verified: dict [DATA] - validation results
#  (E) artifacts_ready: bool [ENABLER] - gates checkpoint download
#
# Step 9: a_download_model_checkpoint
#  (P) artifacts_ready: bool [ENABLER]
#  (E) checkpoint_path: str [DATA] - local path of downloaded model
#  (E) download_complete: bool [ENABLER] - gates workspace cleanup
#
# Step 10: a_cleanup_job_workspace
#  (P) download_complete: bool [ENABLER]
#  (E) cleanup_status: str [DATA] - completion message
#  (E) workflow_complete: bool [ENABLER] - terminal state
#
# Step 11: a_check_drained_nodes
#  (P) facility_loaded: bool [ENABLER]
#  (E) drained_nodes: list [DATA] - unavailable node information
# =============================================================================


# =============================================================================
# BEGIN: Actions
# =============================================================================

def a_check_facility_info(state: State, model_name: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_facility

    Action signature:
        a_check_facility_info(state, model_name)

    Action parameters:
        model_name: Identifier for the training job (e.g., 'mdlm_v1_phase2')

    Action purpose:
        Retrieve RIKYU facility configuration: GPU partition, node/CPU/memory table,
        storage tiers, available modules, and Spack packages.

    Preconditions:
        - model_name must be a non-empty string

    Effects:
        - Facility configuration loaded (state.facility_info) [DATA]
        - Facility load completed flag (state.facility_loaded) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(model_name, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not model_name.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions - this is the first action in the pipeline
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store the job identifier
    state.model_name = model_name

    # [DATA] Facility configuration placeholder (populated by MCP server at runtime)
    state.facility_info = {
        "model_name": model_name,
        "source": "rikyu-hpc:get_facility",
        "partitions": [],
        "gpu_counts": {},
        "storage": {},
        "modules": []
    }

    # [ENABLER] Gates a_get_resource_status
    state.facility_loaded = True
    # END: Effects

    return state


def a_check_drained_nodes(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_drained_nodes

    Action signature:
        a_check_drained_nodes(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Check which cluster nodes are drained or down and identify the reasons,
        helping diagnose scheduling delays or resource unavailability.

    Preconditions:
        - Facility info must be loaded (state.facility_loaded)

    Effects:
        - Drained nodes information (state.drained_nodes) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_loaded') and state.facility_loaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Drained nodes information (populated by MCP server at runtime)
    state.drained_nodes = []
    # END: Effects

    return state


def a_get_resource_status(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_resources

    Action signature:
        a_get_resource_status(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Query live GPU partition occupancy via sinfo: allocated/idle/other/total
        node counts to determine scheduling likelihood.

    Preconditions:
        - Facility info must be loaded (state.facility_loaded)

    Effects:
        - Current resource status (state.resource_status) [DATA]
        - Resource status check completed flag (state.resource_checked) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_loaded') and state.facility_loaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Resource status placeholder (populated by MCP server at runtime)
    state.resource_status = {
        "partition": "gpu",
        "nodes_allocated": 0,
        "nodes_idle": 0,
        "nodes_other": 0,
        "nodes_total": 0
    }

    # [ENABLER] Gates a_create_job_directory
    state.resource_checked = True
    # END: Effects

    return state


def a_create_job_directory(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_mkdir

    Action signature:
        a_create_job_directory(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Create a dedicated workspace directory for the training job on the cluster,
        including parent directories as needed.

    Preconditions:
        - Resource status must be checked (state.resource_checked)

    Effects:
        - Job workspace path created (state.job_path) [DATA]
        - Job directory ready flag (state.job_dir_ready) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'resource_checked') and state.resource_checked):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Job workspace path placeholder
    if hasattr(state, 'model_name') and state.model_name:
        state.job_path = f"agent/jobs/{state.model_name.replace(' ', '_')}"
    else:
        state.job_path = "agent/jobs/training_job"

    # [ENABLER] Gates a_prepare_training_script
    state.job_dir_ready = True
    # END: Effects

    return state


def a_prepare_training_script(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_view (for reading container templates)

    Action signature:
        a_prepare_training_script(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Prepare the containerized training job submission script (sbatch format)
        with appropriate resource requests, container image reference, and training
        command for the specialized MDLM diffusion model.

    Preconditions:
        - Job directory must be ready (state.job_dir_ready)

    Effects:
        - Training script path created (state.training_script_path) [DATA]
        - Script ready flag (state.script_ready) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_dir_ready') and state.job_dir_ready):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Training script path placeholder
    if hasattr(state, 'job_path') and state.job_path:
        state.training_script_path = f"{state.job_path}/train.sbatch"
    else:
        state.training_script_path = "agent/jobs/training_job/train.sbatch"

    # [ENABLER] Gates a_submit_training_job
    state.script_ready = True
    # END: Effects

    return state


def a_submit_training_job(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:submit_job

    Action signature:
        a_submit_training_job(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Submit the training job to RIKYU's Slurm scheduler with appropriate
        GPU resource requests and job configuration.

    Preconditions:
        - Training script must be ready (state.script_ready)

    Effects:
        - Job ID from Slurm (state.job_id) [DATA]
        - Job submission completed flag (state.job_submitted) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'script_ready') and state.script_ready):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Job ID placeholder (populated by MCP server at runtime)
    state.job_id = f"job_id_for_{state.model_name.replace(' ', '_')}"

    # [ENABLER] Gates a_monitor_job_status
    state.job_submitted = True
    # END: Effects

    return state


def a_monitor_job_status(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_job_status

    Action signature:
        a_monitor_job_status(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Poll job status from Slurm scheduler to check if training is complete
        or still running. Supports repeated polling in the workflow.

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must be available (state.job_id)

    Effects:
        - Current job status (state.job_status) [DATA]
        - Job completion flag (state.job_completed) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Job status placeholder (populated by MCP server at runtime)
    state.job_status = {
        "job_id": state.job_id if hasattr(state, 'job_id') else "unknown",
        "state": "RUNNING",
        "elapsed": 0,
        "time_limit": 3600
    }

    # [ENABLER] Gates a_retrieve_job_logs (once job completes)
    state.job_completed = True
    # END: Effects

    return state


def a_retrieve_job_logs(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_tail

    Action signature:
        a_retrieve_job_logs(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Fetch stdout/stderr from the training job's output files to verify
        training progress and identify any errors or warnings.

    Preconditions:
        - Job must be completed (state.job_completed)

    Effects:
        - Job standard output content (state.stdout_content) [DATA]
        - Logs retrieval completed flag (state.logs_retrieved) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Stdout content placeholder (populated by MCP server at runtime)
    state.stdout_content = ""

    # [ENABLER] Gates a_verify_output_artifacts
    state.logs_retrieved = True
    # END: Effects

    return state


def a_verify_output_artifacts(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_ls

    Action signature:
        a_verify_output_artifacts(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Check the job workspace for expected training output files
        (model checkpoints, metrics, logs) to verify successful completion.

    Preconditions:
        - Logs must be retrieved (state.logs_retrieved)

    Effects:
        - Artifact verification results (state.artifacts_verified) [DATA]
        - Artifacts ready for download flag (state.artifacts_ready) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'logs_retrieved') and state.logs_retrieved):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Artifact verification results placeholder
    state.artifacts_verified = {
        "job_path": state.job_path if hasattr(state, 'job_path') else "",
        "checkpoint_found": False,
        "metrics_found": False,
        "logs_found": False
    }

    # [ENABLER] Gates a_download_model_checkpoint
    state.artifacts_ready = True
    # END: Effects

    return state


def a_download_model_checkpoint(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_download

    Action signature:
        a_download_model_checkpoint(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Download the trained model checkpoint from the cluster to local storage
        for use in downstream inference or fine-tuning tasks.

    Preconditions:
        - Artifacts must be ready (state.artifacts_ready)

    Effects:
        - Local checkpoint file path (state.checkpoint_path) [DATA]
        - Download completion flag (state.download_complete) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'artifacts_ready') and state.artifacts_ready):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Local checkpoint path placeholder
    if hasattr(state, 'model_name') and state.model_name:
        state.checkpoint_path = f"./checkpoints/{state.model_name.replace(' ', '_')}_model.pt"
    else:
        state.checkpoint_path = "./checkpoints/trained_model.pt"

    # [ENABLER] Gates a_cleanup_job_workspace
    state.download_complete = True
    # END: Effects

    return state


def a_cleanup_job_workspace(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_mv

    Action signature:
        a_cleanup_job_workspace(state)

    Action parameters:
        None (state-driven)

    Action purpose:
        Remove temporary training files and job workspace from the cluster
        to free up storage and maintain a clean job submission directory.

    Preconditions:
        - Download must be complete (state.download_complete)

    Effects:
        - Cleanup completion status (state.cleanup_status) [DATA]
        - Workflow completion flag (state.workflow_complete) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    # None needed
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'download_complete') and state.download_complete):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Cleanup completion status
    state.cleanup_status = "Job workspace cleaned successfully"

    # [ENABLER] Terminal state - marks workflow completion
    state.workflow_complete = True
    # END: Effects

    return state

# =============================================================================
# END: Actions
# =============================================================================


# =============================================================================
# BEGIN: Methods
# =============================================================================

def m_containerized_training_workflow(state: State, model_name: str) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_containerized_training_workflow(state, model_name)

    Method parameters:
        model_name: Identifier for the training job (e.g., 'mdlm_v1_phase2')

    Method purpose:
        Top-level orchestration for complete containerized MDLM diffusion model
        training pipeline on RIKYU GPU cluster, from facility check through
        model checkpoint download and workspace cleanup.

    Preconditions:
        - model_name is a non-empty string

    Task decomposition:
        - a_check_facility_info: Load facility configuration
        - a_check_drained_nodes: Identify unavailable nodes
        - a_get_resource_status: Check GPU partition occupancy
        - a_create_job_directory: Create training workspace
        - a_prepare_training_script: Prepare containerized job script
        - a_submit_training_job: Submit to Slurm scheduler
        - monitor_and_retrieve_logs: Sub-workflow for status polling and log retrieval
        - a_verify_output_artifacts: Check training output
        - a_download_model_checkpoint: Download trained model
        - a_cleanup_job_workspace: Remove temporary files

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(model_name, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not model_name.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions - this is the entry point method
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_check_facility_info", model_name),
        ("a_check_drained_nodes",),
        ("a_get_resource_status",),
        ("a_create_job_directory",),
        ("a_prepare_training_script",),
        ("a_submit_training_job",),
        ("monitor_and_retrieve_logs",),
        ("a_verify_output_artifacts",),
        ("a_download_model_checkpoint",),
        ("a_cleanup_job_workspace",),
    ]
    # END: Task Decomposition


def m_monitor_and_retrieve_logs(state: State) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_monitor_and_retrieve_logs(state)

    Method parameters:
        None (uses state.job_submitted, state.job_id)

    Method auxiliary parameters:
        job_id: str (inferred from state.job_id)

    Method purpose:
        Sub-workflow for monitoring job execution status and retrieving
        job output logs once training completes.

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must exist (state.job_id)

    Task decomposition:
        - a_monitor_job_status: Poll job status until completion

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Auxiliary Parameter Inference
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: Auxiliary Parameter Inference

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_monitor_job_status",),
        ("a_retrieve_job_logs",),
    ]
    # END: Task Decomposition

# =============================================================================
# END: Methods
# =============================================================================


# =============================================================================
# DECLARE ACTIONS TO DOMAIN
# =============================================================================
declare_actions(
    a_check_facility_info,
    a_check_drained_nodes,
    a_get_resource_status,
    a_create_job_directory,
    a_prepare_training_script,
    a_submit_training_job,
    a_monitor_job_status,
    a_retrieve_job_logs,
    a_verify_output_artifacts,
    a_download_model_checkpoint,
    a_cleanup_job_workspace,
)


# =============================================================================
# DECLARE METHODS TO DOMAIN
# =============================================================================
declare_task_methods("containerized_training_workflow", m_containerized_training_workflow)
declare_task_methods("monitor_and_retrieve_logs", m_monitor_and_retrieve_logs)
