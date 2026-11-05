"""
Containerization Training for Specialized MDLM Diffusion Model - GTPyhop Domain.

Domain: cont-train
Version: 1.0.0
MCP Servers: rikyu-hpc, rikyu-docs

Actions (13):
    - a_check_facility: Get facility GPU/node/resource info (rikyu-hpc)
    - a_check_resources: Get live cluster resource occupancy (rikyu-hpc)
    - a_create_job_dir: Create working directory for training (rikyu-hpc)
    - a_upload_training_code: Upload containerized training code to cluster (rikyu-hpc)
    - a_submit_training_job: Submit training job to Slurm scheduler (rikyu-hpc)
    - a_poll_job_status: Poll status of running training job (rikyu-hpc)
    - a_download_checkpoints: Download model checkpoints from cluster (rikyu-hpc)
    - a_download_logs: Download training logs from cluster (rikyu-hpc)
    - a_verify_outputs: Verify checksum of downloaded artifacts (rikyu-hpc)
    - a_cancel_job: Cancel job if needed (rikyu-hpc)
    - a_cleanup_remote: Remove temporary files on cluster (rikyu-hpc)
    - a_compress_results: Create compressed archive of results (rikyu-hpc)
    - a_run_validation_test: Run quick validation on downloaded checkpoint (rikyu-hpc)

Methods (3):
    - m_containerized_training: Orchestrate full training pipeline
    - m_monitor_training: Monitor job and retrieve outputs
    - m_cleanup: Clean up local and remote artifacts
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
# Step 1: a_check_facility
#  (E) facility_config: dict [DATA] - cluster facility metadata
#  (E) facility_checked: bool [ENABLER] - gates resource check
#
# Step 2: a_check_resources
#  (P) facility_checked: bool [ENABLER]
#  (E) resource_info: dict [DATA] - live cluster resource occupancy
#  (E) resources_verified: bool [ENABLER] - gates job setup
#
# Step 3: a_create_job_dir
#  (P) resources_verified: bool [ENABLER]
#  (E) job_dir: str [DATA] - working directory path on cluster
#  (E) job_dir_created: bool [ENABLER] - gates code upload
#
# Step 4: a_upload_training_code
#  (P) job_dir_created: bool [ENABLER]
#  (P) training_code_path: str [DATA] - local path to training code
#  (E) remote_code_path: str [DATA] - path where code was uploaded
#  (E) code_uploaded: bool [ENABLER] - gates job submission
#
# Step 5: a_submit_training_job
#  (P) code_uploaded: bool [ENABLER]
#  (P) gpu_count: int [DATA] - number of GPUs requested
#  (P) time_limit: str [DATA] - wall time limit (HH:MM:SS)
#  (E) job_id: str [DATA] - submitted job ID from Slurm
#  (E) job_submitted: bool [ENABLER] - gates monitoring
#
# Step 6: a_poll_job_status
#  (P) job_submitted: bool [ENABLER]
#  (P) job_id: str [DATA]
#  (E) job_status: str [DATA] - current job state
#  (E) job_completed: bool [ENABLER] - gates output retrieval
#
# Step 7a: a_download_checkpoints
#  (P) job_completed: bool [ENABLER]
#  (P) job_dir: str [DATA]
#  (E) checkpoints_path: str [DATA] - local path to downloaded checkpoints
#  (E) checkpoints_downloaded: bool [ENABLER] - gates verification
#
# Step 7b: a_download_logs
#  (P) job_completed: bool [ENABLER]
#  (P) job_dir: str [DATA]
#  (E) logs_path: str [DATA] - local path to downloaded logs
#  (E) logs_downloaded: bool [ENABLER] - gates compression
#
# Step 8: a_verify_outputs
#  (P) checkpoints_downloaded: bool [ENABLER]
#  (P) logs_downloaded: bool [ENABLER]
#  (E) checksums: dict [DATA] - SHA-256 checksums of downloaded files
#  (E) outputs_verified: bool [ENABLER] - gates cleanup and validation
#
# Step 9: a_run_validation_test
#  (P) outputs_verified: bool [ENABLER]
#  (P) checkpoints_path: str [DATA]
#  (E) validation_result: dict [DATA] - validation metrics/results
#  (E) validation_passed: bool [ENABLER] - gates cleanup
#
# Step 10: a_compress_results
#  (P) validation_passed: bool [ENABLER]
#  (P) checkpoints_path: str [DATA]
#  (P) logs_path: str [DATA]
#  (E) archive_path: str [DATA] - path to compressed results
#  (E) results_archived: bool [ENABLER] - gates remote cleanup
#
# Step 11: a_cleanup_remote
#  (P) results_archived: bool [ENABLER]
#  (P) job_dir: str [DATA]
#  (E) remote_cleaned: bool [ENABLER] - marks final cleanup
#
# =============================================================================


# =============================================================================
# BEGIN: Actions
# =============================================================================

def a_check_facility(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: get_facility
    Action signature: a_check_facility(state)
    Action parameters: (none)
    Action purpose: Retrieve static facility configuration (GPU counts, partitions, storage tiers)
    Preconditions: None (always executable)
    Effects:
        - facility_config: dict - cluster metadata
        - facility_checked: bool - True (gates next step)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # No preconditions required for first action
    # END: Preconditions

    # BEGIN: Effects
    state.facility_config = {
        "partitions": ["gpu"],
        "gpu_counts": [1, 2, 4, 8],
        "storage_tiers": ["scratch", "home"],
        "loaded_from": "hpc_agent_core.middleware"
    }
    state.facility_checked = True
    # END: Effects

    return state


def a_check_resources(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: get_resources
    Action signature: a_check_resources(state)
    Action parameters: (none)
    Action purpose: Query live resource occupancy (node allocation, idle/allocated/other counts)
    Preconditions: facility_checked must be True
    Effects:
        - resource_info: dict - live resource occupancy snapshot
        - resources_verified: bool - True (gates job setup)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'facility_checked') and state.facility_checked):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_checked') and state.facility_checked):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.resource_info = {
        "partition": "gpu",
        "allocated": 150,
        "idle": 200,
        "other": 50,
        "total": 400
    }
    state.resources_verified = True
    # END: Effects

    return state


def a_create_job_dir(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_mkdir
    Action signature: a_create_job_dir(state)
    Action parameters: (none)
    Action purpose: Create working directory on cluster for training artifacts
    Preconditions: resources_verified must be True
    Effects:
        - job_dir: str - absolute path to created directory
        - job_dir_created: bool - True (gates code upload)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'resources_verified') and state.resources_verified):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'resources_verified') and state.resources_verified):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.job_dir = "agent/jobs/mdlm_training_job"
    state.job_dir_created = True
    # END: Effects

    return state


def a_upload_training_code(state: State, training_code_local: str) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_upload
    Action signature: a_upload_training_code(state, training_code_local)
    Action parameters: training_code_local (str) - path to local training code
    Action purpose: Upload containerized training code to cluster working directory
    Preconditions: job_dir_created must be True
    Effects:
        - remote_code_path: str - path where code was uploaded
        - code_uploaded: bool - True (gates job submission)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State) or not isinstance(training_code_local, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'job_dir_created') and state.job_dir_created):
        return False
    if not (hasattr(state, 'job_dir') and state.job_dir):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_dir_created') and state.job_dir_created):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.remote_code_path = f"{state.job_dir}/training_code.tar.gz"
    state.code_uploaded = True
    # END: Effects

    return state


def a_submit_training_job(state: State, gpu_count: int, time_limit: str) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: submit_job
    Action signature: a_submit_training_job(state, gpu_count, time_limit)
    Action parameters:
        - gpu_count (int) - number of GPUs for training
        - time_limit (str) - wall time limit (HH:MM:SS)
    Action purpose: Submit containerized training job to Slurm scheduler
    Preconditions: code_uploaded must be True, gpu_count in [1,2,4,8], time_limit format valid
    Effects:
        - job_id: str - Slurm job ID (placeholder)
        - job_submitted: bool - True (gates monitoring)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State) or not isinstance(gpu_count, int) or not isinstance(time_limit, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'code_uploaded') and state.code_uploaded):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'code_uploaded') and state.code_uploaded):
        return False
    if gpu_count not in [1, 2, 4, 8]:
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.gpu_count = gpu_count
    state.time_limit = time_limit
    state.job_id = f"job_id_for_{gpu_count}_gpus_{time_limit.replace(':', '_')}"
    state.job_submitted = True
    # END: Effects

    return state


def a_poll_job_status(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: get_job_status
    Action signature: a_poll_job_status(state)
    Action parameters: (none)
    Action purpose: Poll status of submitted training job
    Preconditions: job_submitted must be True
    Effects:
        - job_status: str - current job state (RUNNING, COMPLETED, FAILED, etc.)
        - job_completed: bool - True if status indicates completion
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.job_status = "COMPLETED"
    state.job_completed = True
    # END: Effects

    return state


def a_download_checkpoints(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_download
    Action signature: a_download_checkpoints(state)
    Action parameters: (none)
    Action purpose: Download model checkpoints from cluster to local machine
    Preconditions: job_completed must be True
    Effects:
        - checkpoints_path: str - local path to downloaded checkpoints
        - checkpoints_downloaded: bool - True (gates verification)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    if not (hasattr(state, 'job_dir') and state.job_dir):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.checkpoints_path = "./downloads/checkpoints"
    state.checkpoints_downloaded = True
    # END: Effects

    return state


def a_download_logs(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_download
    Action signature: a_download_logs(state)
    Action parameters: (none)
    Action purpose: Download training logs from cluster to local machine
    Preconditions: job_completed must be True
    Effects:
        - logs_path: str - local path to downloaded logs
        - logs_downloaded: bool - True (gates compression)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    if not (hasattr(state, 'job_dir') and state.job_dir):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_completed') and state.job_completed):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.logs_path = "./downloads/logs"
    state.logs_downloaded = True
    # END: Effects

    return state


def a_verify_outputs(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_checksum
    Action signature: a_verify_outputs(state)
    Action parameters: (none)
    Action purpose: Verify checksums of downloaded artifacts for integrity
    Preconditions: checkpoints_downloaded and logs_downloaded must both be True
    Effects:
        - checksums: dict - SHA-256 checksums of verified files
        - outputs_verified: bool - True (gates cleanup and validation)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'checkpoints_downloaded') and state.checkpoints_downloaded):
        return False
    if not (hasattr(state, 'logs_downloaded') and state.logs_downloaded):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'checkpoints_downloaded') and state.checkpoints_downloaded):
        return False
    if not (hasattr(state, 'logs_downloaded') and state.logs_downloaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.checksums = {
        "checkpoints": "sha256_placeholder_for_checkpoints",
        "logs": "sha256_placeholder_for_logs"
    }
    state.outputs_verified = True
    # END: Effects

    return state


def a_run_validation_test(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: run_command_on_cluster
    Action signature: a_run_validation_test(state)
    Action parameters: (none)
    Action purpose: Run quick validation/inference test on downloaded checkpoint
    Preconditions: outputs_verified must be True
    Effects:
        - validation_result: dict - validation metrics and test results
        - validation_passed: bool - True if validation successful (gates cleanup)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'outputs_verified') and state.outputs_verified):
        return False
    if not (hasattr(state, 'checkpoints_path') and state.checkpoints_path):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'outputs_verified') and state.outputs_verified):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.validation_result = {
        "test_samples": 100,
        "accuracy": 0.95,
        "inference_time_ms": 45.3,
        "status": "passed"
    }
    state.validation_passed = True
    # END: Effects

    return state


def a_compress_results(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: fs_compress
    Action signature: a_compress_results(state)
    Action parameters: (none)
    Action purpose: Create compressed archive of checkpoints and logs
    Preconditions: validation_passed must be True
    Effects:
        - archive_path: str - path to created compressed archive
        - results_archived: bool - True (gates remote cleanup)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'validation_passed') and state.validation_passed):
        return False
    if not (hasattr(state, 'checkpoints_path') and state.checkpoints_path):
        return False
    if not (hasattr(state, 'logs_path') and state.logs_path):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'validation_passed') and state.validation_passed):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.archive_path = "./results/mdlm_training_results.tar.gz"
    state.results_archived = True
    # END: Effects

    return state


def a_cleanup_remote(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: run_command_on_cluster
    Action signature: a_cleanup_remote(state)
    Action parameters: (none)
    Action purpose: Remove temporary files and working directory on cluster
    Preconditions: results_archived must be True
    Effects:
        - remote_cleaned: bool - True (marks pipeline completion)
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'results_archived') and state.results_archived):
        return False
    if not (hasattr(state, 'job_dir') and state.job_dir):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'results_archived') and state.results_archived):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.remote_cleaned = True
    # END: Effects

    return state


def a_cancel_job(state: State) -> Union[State, bool]:
    """
    Class: Action
    MCP_Tool: cancel_job
    Action signature: a_cancel_job(state)
    Action parameters: (none)
    Action purpose: Cancel training job if workflow fails or is interrupted
    Preconditions: job_submitted must be True, job_completed must be False
    Effects:
        - job_cancelled: bool - True
    Returns: state on success, False if precondition fails
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    if not (hasattr(state, 'job_id') and state.job_id):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'job_submitted') and state.job_submitted):
        return False
    # END: Preconditions

    # BEGIN: Effects
    state.job_cancelled = True
    # END: Effects

    return state

# =============================================================================
# END: Actions
# =============================================================================


# =============================================================================
# BEGIN: Methods
# =============================================================================

def m_containerized_training(state: State, training_code_path: str, gpu_count: int, time_limit: str) -> Union[List[Tuple], bool]:
    """
    Class: Method
    Method signature: m_containerized_training(state, training_code_path, gpu_count, time_limit)
    Method parameters:
        - training_code_path (str) - path to local training code
        - gpu_count (int) - number of GPUs (1, 2, 4, or 8)
        - time_limit (str) - wall time limit (HH:MM:SS format)
    Method purpose: Orchestrate complete containerized training pipeline
    Preconditions: (none for method)
    Task decomposition:
        1. Check facility configuration
        2. Verify resource availability
        3. Create job directory
        4. Upload training code
        5. Submit job with specified resources
        6. Monitor training progress
    Returns: Task tuple list or False
    """
    # BEGIN: Type Checking
    if not isinstance(state, State) or not isinstance(training_code_path, str):
        return False
    if not isinstance(gpu_count, int) or not isinstance(time_limit, str):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # No preconditions required for top-level method
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_check_facility",),
        ("a_check_resources",),
        ("a_create_job_dir",),
        ("a_upload_training_code", training_code_path),
        ("a_submit_training_job", gpu_count, time_limit),
        ("monitor_training",),
    ]
    # END: Task Decomposition


def m_monitor_training(state: State) -> Union[List[Tuple], bool]:
    """
    Class: Method
    Method signature: m_monitor_training(state)
    Method parameters: (none)
    Method purpose: Monitor job execution and retrieve outputs
    Preconditions: (none for method — checked by decomposition)
    Task decomposition:
        1. Poll job status
        2. Download checkpoints
        3. Download logs
        4. Verify output integrity
        5. Run validation test
    Returns: Task tuple list or False
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # Preconditions checked by previous decomposition in a_submit_training_job
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_poll_job_status",),
        ("a_download_checkpoints",),
        ("a_download_logs",),
        ("a_verify_outputs",),
        ("a_run_validation_test",),
        ("finalize_training",),
    ]
    # END: Task Decomposition


def m_finalize_training(state: State) -> Union[List[Tuple], bool]:
    """
    Class: Method
    Method signature: m_finalize_training(state)
    Method parameters: (none)
    Method purpose: Compress results and clean up cluster resources
    Preconditions: (none for method)
    Task decomposition:
        1. Compress training results
        2. Clean up remote job directory
    Returns: Task tuple list or False
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # Preconditions checked by previous decomposition
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_compress_results",),
        ("a_cleanup_remote",),
    ]
    # END: Task Decomposition

# =============================================================================
# END: Methods
# =============================================================================


# =============================================================================
# DECLARE ACTIONS TO DOMAIN
# =============================================================================
declare_actions(
    a_check_facility,
    a_check_resources,
    a_create_job_dir,
    a_upload_training_code,
    a_submit_training_job,
    a_poll_job_status,
    a_download_checkpoints,
    a_download_logs,
    a_verify_outputs,
    a_run_validation_test,
    a_compress_results,
    a_cleanup_remote,
    a_cancel_job,
)


# =============================================================================
# DECLARE METHODS TO DOMAIN
# =============================================================================
declare_task_methods("containerized_training", m_containerized_training)
declare_task_methods("monitor_training", m_monitor_training)
declare_task_methods("finalize_training", m_finalize_training)
