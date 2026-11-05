"""
Containerized Training Domain - GTPyhop Domain.

Domain: cont-train
Version: 1.0.0
MCP Servers: rikyu-hpc (hpc_server), rikyu-docs (docs_server), rikyu-doctor (doctor)

Actions (15):
    - a_get_facility_info: Retrieve static cluster facts (hpc_server)
    - a_check_resource_availability: Check live partition occupancy (hpc_server)
    - a_check_drained_nodes: Check node health status (hpc_server)
    - a_submit_training_job: Submit training job to Slurm (hpc_server)
    - a_get_job_status: Poll status of a running job (hpc_server)
    - a_cancel_training_job: Cancel a queued or running job (hpc_server)
    - a_create_working_directory: Create remote directory for training (hpc_server)
    - a_upload_training_config: Upload local config to cluster (hpc_server)
    - a_download_training_results: Download results from cluster (hpc_server)
    - a_verify_transfer_integrity: Verify file checksum (hpc_server)
    - a_list_training_artifacts: List directory contents (hpc_server)
    - a_compress_artifacts: Create tar archive of results (hpc_server)
    - a_search_documentation: Search RIKYU docs (docs_server)
    - a_run_health_check: Run system diagnostics (doctor)
    - a_copy_training_data: Copy files on cluster (hpc_server)

Methods (5):
    - m_setup_training_environment: Initialize cluster environment
    - m_submit_training_job_with_monitoring: Submit job and track completion
    - m_retrieve_training_artifacts: Download and verify results
    - m_full_training_pipeline: End-to-end training workflow
    - m_health_check_and_setup: Pre-flight checks
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
        print("Please install gtpyhop using: pip install gtpyhop-core")
        sys.exit(1)


# =============================================================================
# DOMAIN
# =============================================================================
the_domain = Domain("cont-train")
set_current_domain(the_domain)
DOMAIN_NAME = "cont-train"

# HARD ORDERING RULE: `Domain(...)` and `set_current_domain(...)` MUST appear before any
# `declare_actions(...)` / `declare_task_methods(...)` call anywhere later in the file.


# =============================================================================
# STATE PROPERTY MAP
# =============================================================================
# Legend:
#  - (E) Created/modified by the action (Effects)
#  - (P) Consumed/checked by the action (Preconditions/State checks)
#  - [ENABLER] Property acts as a workflow gate for subsequent steps
#  - [DATA]    Informational/data container
#
# Step 1: a_run_health_check
#  (E) system_healthy: bool [ENABLER] - gates setup_training_environment
#
# Step 2: a_get_facility_info
#  (E) facility_config: dict [DATA] - cluster static facts
#  (E) facility_loaded: bool [ENABLER] - gates resource checks
#
# Step 3: a_check_resource_availability
#  (P) facility_loaded: bool [ENABLER]
#  (E) resource_available: bool [DATA] - partition occupancy check
#  (E) resources_checked: bool [ENABLER] - gates training submission
#
# Step 4: a_create_working_directory
#  (E) training_dir: str [DATA] - remote working directory path
#  (E) directory_created: bool [ENABLER] - gates config upload
#
# Step 5: a_upload_training_config
#  (P) directory_created: bool [ENABLER]
#  (E) config_uploaded: bool [ENABLER] - gates job submission
#
# Step 6: a_submit_training_job
#  (P) resources_checked: bool [ENABLER]
#  (P) config_uploaded: bool [ENABLER]
#  (E) job_id: str [DATA] - Slurm job identifier
#  (E) job_submitted: bool [ENABLER] - gates status monitoring
#
# Step 7: a_get_job_status (repeated)
#  (P) job_submitted: bool [ENABLER]
#  (E) job_status: str [DATA] - current job state (PENDING, RUNNING, COMPLETED, etc.)
#  (E) training_complete: bool [ENABLER] - gates artifact retrieval
#
# Step 8: a_list_training_artifacts
#  (P) training_complete: bool [ENABLER]
#  (E) artifacts_list: str [DATA] - directory listing
#  (E) artifacts_ready: bool [ENABLER] - gates download
#
# Step 9: a_download_training_results
#  (P) artifacts_ready: bool [ENABLER]
#  (E) results_path: str [DATA] - local path to downloaded results
#  (E) results_downloaded: bool [ENABLER] - gates verification
#
# Step 10: a_verify_transfer_integrity
#  (P) results_downloaded: bool [ENABLER]
#  (E) transfer_verified: bool [ENABLER] - workflow completion gate
# =============================================================================


# =============================================================================
# BEGIN: Actions
# =============================================================================

def a_run_health_check(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-doctor:health_check (implicit via hpc_agent_core.doctor)

    Action signature:
        a_run_health_check(state)

    Action parameters:
        None

    Action purpose:
        Run system health diagnostics to verify cluster connectivity and Slurm availability

    Preconditions:
        None (entry point action)

    Effects:
        - System health status (state.system_healthy) [ENABLER]
        - Health check details (state.health_report) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # No state preconditions - this is the entry point
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Health report placeholder (populated by hpc_agent_core.doctor at runtime)
    state.health_report = {
        "scheduler_available": True,
        "cluster_connected": True,
        "timestamp": "placeholder"
    }

    # [ENABLER] Gates setup_training_environment
    state.system_healthy = True
    # END: Effects

    return state


def a_get_facility_info(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_facility

    Action signature:
        a_get_facility_info(state)

    Action parameters:
        None

    Action purpose:
        Retrieve static cluster facility information (GPU partitions, node configuration, modules)

    Preconditions:
        - System health check must have passed (state.system_healthy)

    Effects:
        - Facility configuration (state.facility_config) [DATA]
        - Facility info loaded flag (state.facility_loaded) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'system_healthy') and state.system_healthy):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Facility configuration (populated by hpc_server at runtime)
    state.facility_config = {
        "gpu_partition": "gpu",
        "node_count": 400,
        "gpu_per_node": 1,
        "supported_gpu_counts": [1, 2, 4, 8],
        "storage_tiers": [],
        "modules_available": []
    }

    # [ENABLER] Gates a_check_resource_availability
    state.facility_loaded = True
    # END: Effects

    return state


def a_check_resource_availability(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_resources

    Action signature:
        a_check_resource_availability(state)

    Action parameters:
        None

    Action purpose:
        Check live occupancy of compute partitions (allocated/idle node counts)

    Preconditions:
        - Facility info must be loaded (state.facility_loaded)

    Effects:
        - Resource occupancy data (state.resource_occupancy) [DATA]
        - Resources checked flag (state.resources_checked) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_loaded') and state.facility_loaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Resource occupancy (populated by hpc_server at runtime)
    state.resource_occupancy = {
        "partition": "gpu",
        "allocated_nodes": 0,
        "idle_nodes": 400,
        "other_nodes": 0,
        "total_nodes": 400
    }

    # [ENABLER] Gates a_submit_training_job
    state.resources_checked = True
    # END: Effects

    return state


def a_check_drained_nodes(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_drained_nodes

    Action signature:
        a_check_drained_nodes(state)

    Action parameters:
        None

    Action purpose:
        Check which nodes are drained or down and identify reasons (maintenance, failures)

    Preconditions:
        - Facility info must be loaded (state.facility_loaded)

    Effects:
        - Drained nodes list (state.drained_nodes) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'facility_loaded') and state.facility_loaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Drained nodes list (populated by hpc_server at runtime)
    if not hasattr(state, 'drained_nodes') or state.drained_nodes is None:
        state.drained_nodes = []
    # END: Effects

    return state


def a_create_working_directory(state: State, working_dir: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_mkdir

    Action signature:
        a_create_working_directory(state, working_dir)

    Action parameters:
        working_dir: Remote directory path to create (e.g., '/home/user/training_run_001')

    Action purpose:
        Create remote working directory for training data and configuration

    Preconditions:
        - System must be healthy (state.system_healthy)

    Effects:
        - Working directory path (state.training_dir) [DATA]
        - Directory creation flag (state.directory_created) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(working_dir, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not working_dir.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'system_healthy') and state.system_healthy):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store the training directory path
    state.training_dir = working_dir

    # [ENABLER] Gates a_upload_training_config
    state.directory_created = True
    # END: Effects

    return state


def a_upload_training_config(state: State, local_config_path: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_upload

    Action signature:
        a_upload_training_config(state, local_config_path)

    Action parameters:
        local_config_path: Local filesystem path to training configuration file

    Action purpose:
        Upload training configuration to remote working directory via rsync/scp

    Preconditions:
        - Working directory must be created (state.directory_created)
        - Training directory path must be available (state.training_dir)

    Effects:
        - Remote config path (state.remote_config_path) [DATA]
        - Config upload flag (state.config_uploaded) [ENABLER]
        - Upload checksum (state.config_checksum) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(local_config_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not local_config_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'directory_created') and state.directory_created):
        return False
    if not (hasattr(state, 'training_dir') and state.training_dir):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Construct remote path for config
    training_dir = state.training_dir
    config_filename = local_config_path.split('/')[-1]
    state.remote_config_path = f"{training_dir}/{config_filename}"

    # [DATA] Upload checksum (populated by hpc_server at runtime)
    state.config_checksum = f"sha256_placeholder_for_{config_filename}"

    # [ENABLER] Gates a_submit_training_job
    state.config_uploaded = True
    # END: Effects

    return state


def a_submit_training_job(state: State, job_spec_path: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:submit_job

    Action signature:
        a_submit_training_job(state, job_spec_path)

    Action parameters:
        job_spec_path: Path to JobSpec definition (in planning context, placeholder)

    Action purpose:
        Submit MDLM diffusion model training job to Slurm scheduler

    Preconditions:
        - Resources must be checked and available (state.resources_checked)
        - Configuration must be uploaded (state.config_uploaded)

    Effects:
        - Slurm job ID (state.job_id) [DATA]
        - Job submission flag (state.job_submitted) [ENABLER]
        - Submission timestamp (state.job_submitted_at) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(job_spec_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not job_spec_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'resources_checked') and state.resources_checked):
        return False
    if not (hasattr(state, 'config_uploaded') and state.config_uploaded):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Job ID placeholder (real ID from Slurm at runtime)
    state.job_id = f"job_id_for_{job_spec_path.replace('/', '_')}"

    # [DATA] Submission timestamp placeholder
    state.job_submitted_at = "timestamp_placeholder"

    # [ENABLER] Gates a_get_job_status
    state.job_submitted = True
    # END: Effects

    return state


def a_get_job_status(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:get_job_status

    Action signature:
        a_get_job_status(state)

    Action parameters:
        None (uses state.job_id)

    Action purpose:
        Poll Slurm for current job status (PENDING, RUNNING, COMPLETED, FAILED)

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must be available (state.job_id)

    Effects:
        - Job status string (state.job_status) [DATA]
        - Training completion flag (state.training_complete) [ENABLER]

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
    # [DATA] Job status placeholder (populated by hpc_server at runtime)
    state.job_status = "COMPLETED"

    # [ENABLER] Gates m_retrieve_training_artifacts
    state.training_complete = True
    # END: Effects

    return state


def a_cancel_training_job(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:cancel_job

    Action signature:
        a_cancel_training_job(state)

    Action parameters:
        None (uses state.job_id)

    Action purpose:
        Cancel a queued or running training job via scancel

    Preconditions:
        - Job must be submitted (state.job_submitted)
        - Job ID must be available (state.job_id)

    Effects:
        - Cancellation result (state.job_cancelled) [ENABLER]

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
    # [ENABLER] Marks job as cancelled
    state.job_cancelled = True
    # END: Effects

    return state


def a_list_training_artifacts(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_ls

    Action signature:
        a_list_training_artifacts(state)

    Action parameters:
        None (uses state.training_dir)

    Action purpose:
        List directory contents to verify training artifacts are present

    Preconditions:
        - Training must be complete (state.training_complete)
        - Training directory must be set (state.training_dir)

    Effects:
        - Directory listing string (state.artifacts_list) [DATA]
        - Artifacts ready flag (state.artifacts_ready) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'training_complete') and state.training_complete):
        return False
    if not (hasattr(state, 'training_dir') and state.training_dir):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Directory listing placeholder (populated by hpc_server at runtime)
    state.artifacts_list = f"directory_listing_for_{state.training_dir}"

    # [ENABLER] Gates a_compress_artifacts or a_download_training_results
    state.artifacts_ready = True
    # END: Effects

    return state


def a_compress_artifacts(state: State, archive_name: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_compress

    Action signature:
        a_compress_artifacts(state, archive_name)

    Action parameters:
        archive_name: Name for the tar archive (e.g., 'training_results.tar.gz')

    Action purpose:
        Create tar archive of training artifacts on the cluster

    Preconditions:
        - Artifacts must be ready (state.artifacts_ready)
        - Training directory must be set (state.training_dir)

    Effects:
        - Archive path (state.archive_path) [DATA]
        - Compression flag (state.artifacts_compressed) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(archive_name, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not archive_name.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'artifacts_ready') and state.artifacts_ready):
        return False
    if not (hasattr(state, 'training_dir') and state.training_dir):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Archive path
    state.archive_path = f"{state.training_dir}/{archive_name}"

    # [ENABLER] Gates a_download_training_results
    state.artifacts_compressed = True
    # END: Effects

    return state


def a_download_training_results(state: State, local_download_path: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_download

    Action signature:
        a_download_training_results(state, local_download_path)

    Action parameters:
        local_download_path: Local directory to download artifacts to

    Action purpose:
        Download training results from cluster to local machine via rsync/scp

    Preconditions:
        - Artifacts must be ready (state.artifacts_ready)
        - Training directory must be set (state.training_dir)

    Effects:
        - Local results path (state.results_path) [DATA]
        - Results downloaded flag (state.results_downloaded) [ENABLER]
        - Download checksum (state.results_checksum) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(local_download_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not local_download_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'artifacts_ready') and state.artifacts_ready):
        return False
    if not (hasattr(state, 'training_dir') and state.training_dir):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Store local download path
    state.results_path = local_download_path

    # [DATA] Download checksum placeholder
    state.results_checksum = f"sha256_placeholder_for_{local_download_path}"

    # [ENABLER] Gates a_verify_transfer_integrity
    state.results_downloaded = True
    # END: Effects

    return state


def a_verify_transfer_integrity(state: State) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_checksum

    Action signature:
        a_verify_transfer_integrity(state)

    Action parameters:
        None (uses state.results_path)

    Action purpose:
        Verify downloaded results via SHA-256 checksum comparison

    Preconditions:
        - Results must be downloaded (state.results_downloaded)
        - Results path must be set (state.results_path)

    Effects:
        - Verification result flag (state.transfer_verified) [ENABLER]
        - Checksum match result (state.checksum_match) [DATA]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    if not (hasattr(state, 'results_downloaded') and state.results_downloaded):
        return False
    if not (hasattr(state, 'results_path') and state.results_path):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Checksum match result placeholder
    state.checksum_match = True

    # [ENABLER] Marks workflow completion
    state.transfer_verified = True
    # END: Effects

    return state


def a_copy_training_data(state: State, source_path: str, dest_path: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-hpc:fs_cp

    Action signature:
        a_copy_training_data(state, source_path, dest_path)

    Action parameters:
        source_path: Source path on cluster
        dest_path: Destination path on cluster

    Action purpose:
        Copy training data on the cluster (e.g., from shared storage to local node)

    Preconditions:
        - System must be healthy (state.system_healthy)

    Effects:
        - Copy completion flag (state.data_copied) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(source_path, str):
        return False
    if not isinstance(dest_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not source_path.strip() or not dest_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'system_healthy') and state.system_healthy):
        return False
    # END: Preconditions

    # BEGIN: Effects
    # [ENABLER] Marks data as copied
    state.data_copied = True
    # END: Effects

    return state


def a_search_documentation(state: State, query: str) -> Union[State, bool]:
    """
    Class: Action

    MCP_Tool: rikyu-docs:search_docs

    Action signature:
        a_search_documentation(state, query)

    Action parameters:
        query: Search query for RIKYU documentation

    Action purpose:
        Search RIKYU documentation for training guidance or troubleshooting info

    Preconditions:
        None (informational action)

    Effects:
        - Documentation search results (state.doc_results) [DATA]
        - Documentation found flag (state.docs_searched) [ENABLER]

    Returns:
        Updated state if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(query, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not query.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions - informational action
    # END: Preconditions

    # BEGIN: Effects
    # [DATA] Documentation search results placeholder
    state.doc_results = {
        "query": query,
        "results": [],
        "count": 0
    }

    # [ENABLER] Marks documentation as searched
    state.docs_searched = True
    # END: Effects

    return state

# =============================================================================
# END: Actions
# =============================================================================


# =============================================================================
# BEGIN: Methods
# =============================================================================

def m_health_check_and_setup(state: State) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_health_check_and_setup(state)

    Method parameters:
        None

    Method purpose:
        Pre-flight checks: verify system health and load facility information

    Preconditions:
        None (entry point)

    Task decomposition:
        - a_run_health_check: Verify cluster connectivity
        - a_get_facility_info: Load facility configuration
        - a_check_drained_nodes: Check node health status

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    # END: Type Checking

    # BEGIN: Preconditions
    # No state preconditions - entry point method
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_run_health_check",),
        ("a_get_facility_info",),
        ("a_check_drained_nodes",),
    ]
    # END: Task Decomposition


def m_setup_training_environment(state: State, working_dir: str, local_config_path: str) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_setup_training_environment(state, working_dir, local_config_path)

    Method parameters:
        working_dir: Remote directory path for training
        local_config_path: Local path to training configuration

    Method purpose:
        Initialize training environment: create directories and upload configuration

    Preconditions:
        None (can run independently)

    Task decomposition:
        - a_create_working_directory: Create remote working directory
        - a_upload_training_config: Upload configuration file

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(working_dir, str):
        return False
    if not isinstance(local_config_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not working_dir.strip() or not local_config_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions for this method
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_create_working_directory", working_dir),
        ("a_upload_training_config", local_config_path),
    ]
    # END: Task Decomposition


def m_submit_training_job_with_monitoring(state: State, job_spec_path: str) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_submit_training_job_with_monitoring(state, job_spec_path)

    Method parameters:
        job_spec_path: Path to job specification

    Method purpose:
        Submit training job and monitor until completion

    Preconditions:
        - Configuration must be uploaded (state.config_uploaded)
        - Resources must be checked (state.resources_checked)

    Task decomposition:
        - a_submit_training_job: Submit to Slurm
        - a_get_job_status: Poll job status until completion

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(job_spec_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not job_spec_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'config_uploaded') and state.config_uploaded):
        return False
    if not (hasattr(state, 'resources_checked') and state.resources_checked):
        return False
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_submit_training_job", job_spec_path),
        ("a_get_job_status",),
    ]
    # END: Task Decomposition


def m_retrieve_training_artifacts(state: State, local_download_path: str) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_retrieve_training_artifacts(state, local_download_path)

    Method parameters:
        local_download_path: Local directory path for download destination

    Method purpose:
        Retrieve and verify training results after job completion

    Preconditions:
        - Training must be complete (state.training_complete)
        - Training directory must be set (state.training_dir)

    Task decomposition:
        - a_list_training_artifacts: Verify artifacts exist
        - a_download_training_results: Download to local machine
        - a_verify_transfer_integrity: Verify checksums

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(local_download_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not local_download_path.strip():
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    if not (hasattr(state, 'training_complete') and state.training_complete):
        return False
    if not (hasattr(state, 'training_dir') and state.training_dir):
        return False
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("a_list_training_artifacts",),
        ("a_download_training_results", local_download_path),
        ("a_verify_transfer_integrity",),
    ]
    # END: Task Decomposition


def m_full_training_pipeline(state: State, working_dir: str, local_config_path: str,
                              job_spec_path: str, local_download_path: str) -> Union[List[Tuple], bool]:
    """
    Class: Method

    Method signature:
        m_full_training_pipeline(state, working_dir, local_config_path, job_spec_path, local_download_path)

    Method parameters:
        working_dir: Remote directory path for training
        local_config_path: Local path to training configuration
        job_spec_path: Path to job specification
        local_download_path: Local directory for results download

    Method purpose:
        End-to-end training workflow: setup, submission, monitoring, and retrieval

    Preconditions:
        None (entry point workflow)

    Task decomposition:
        - health_check_and_setup: Pre-flight checks
        - a_check_resource_availability: Verify resources available
        - setup_training_environment: Create directories and upload config
        - submit_training_job_with_monitoring: Submit and monitor job
        - retrieve_training_artifacts: Download and verify results

    Returns:
        Task decomposition if successful, False otherwise
    """
    # BEGIN: Type Checking
    if not isinstance(state, State):
        return False
    if not isinstance(working_dir, str):
        return False
    if not isinstance(local_config_path, str):
        return False
    if not isinstance(job_spec_path, str):
        return False
    if not isinstance(local_download_path, str):
        return False
    # END: Type Checking

    # BEGIN: State-Type Checks
    if not (working_dir.strip() and local_config_path.strip() and
            job_spec_path.strip() and local_download_path.strip()):
        return False
    # END: State-Type Checks

    # BEGIN: Preconditions
    # No state preconditions - entry point workflow
    # END: Preconditions

    # BEGIN: Task Decomposition
    return [
        ("health_check_and_setup",),
        ("a_check_resource_availability",),
        ("setup_training_environment", working_dir, local_config_path),
        ("submit_training_job_with_monitoring", job_spec_path),
        ("retrieve_training_artifacts", local_download_path),
    ]
    # END: Task Decomposition

# =============================================================================
# END: Methods
# =============================================================================


# =============================================================================
# DECLARE ACTIONS TO DOMAIN
# =============================================================================
declare_actions(
    a_run_health_check,
    a_get_facility_info,
    a_check_resource_availability,
    a_check_drained_nodes,
    a_create_working_directory,
    a_upload_training_config,
    a_submit_training_job,
    a_get_job_status,
    a_cancel_training_job,
    a_list_training_artifacts,
    a_compress_artifacts,
    a_download_training_results,
    a_verify_transfer_integrity,
    a_copy_training_data,
    a_search_documentation,
)


# =============================================================================
# DECLARE METHODS TO DOMAIN
# =============================================================================
declare_task_methods("health_check_and_setup", m_health_check_and_setup)
declare_task_methods("setup_training_environment", m_setup_training_environment)
declare_task_methods("submit_training_job_with_monitoring", m_submit_training_job_with_monitoring)
declare_task_methods("retrieve_training_artifacts", m_retrieve_training_artifacts)
declare_task_methods("full_training_pipeline", m_full_training_pipeline)
