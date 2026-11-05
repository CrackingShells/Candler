"""Containerization training of a specialized MDLM diffusion model - GTPyhop Domain.

Domain: cont_train
Version: 1.0.0
MCP Servers: rikyu-hpc

Actions (5):
    - a_query_gpu_resources: Query GPU partition occupancy (rikyu-hpc)
    - a_prepare_training_environment: Upload training code/config (rikyu-hpc)
    - a_submit_training_job: Submit containerized job to Slurm (rikyu-hpc)
    - a_monitor_job_execution: Poll job status until completion (rikyu-hpc)
    - a_collect_training_results: Download results to local system (rikyu-hpc)

Methods (5):
    - m_ensure_gpu_available: Check GPU resource availability
    - m_setup_training_simple: Prepare training environment
    - m_launch_training: Launch containerized training job
    - m_await_training_completion: Monitor and collect results
    - m_full_training_workflow: Complete end-to-end orchestration
"""

import sys
import os
from typing import Optional, Union, List, Tuple, Dict, Any

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
the_domain = Domain("cont_train")
set_current_domain(the_domain)
DOMAIN_NAME = "cont_train"

# =============================================================================
# STATE PROPERTY MAP
# =============================================================================
# Step 1: a_query_gpu_resources
#  (E) partition_occupancy: dict [DATA] - GPU partition occupancy info
#
# Step 2: a_prepare_training_environment
#  (E) training_env_ready: bool [ENABLER] - gates a_submit_training_job
#  (E) work_dir: str [DATA] - staging directory path
#
# Step 3: a_submit_training_job
#  (P) training_env_ready: bool [ENABLER]
#  (E) job_id: str [DATA] - assigned job ID
#  (E) job_status: str [DATA] - initial status (QUEUED)
#
# Step 4: a_monitor_job_execution
#  (P) job_id: str [DATA]
#  (E) job_status: str [DATA] - terminal status (COMPLETED/FAILED)
#  (E) results_path: str [DATA] - cluster results location
#
# Step 5: a_collect_training_results
#  (P) results_path: str [DATA]
#  (E) results_downloaded: bool [ENABLER] - workflow complete
#  (E) local_results_path: str [DATA] - local download location

# =============================================================================
# BEGIN: Actions
# =============================================================================

def a_query_gpu_resources(state: State) -> Union[State, bool]:
	"""
	Class: Action

	MCP_Tool: rikyu-hpc:get_resources

	Action signature:
		a_query_gpu_resources(state)

	Action parameters:
		state: Current planning state

	Action purpose:
		Query GPU partition occupancy from RIKYU HPC cluster

	Preconditions:
		None (always executable)

	Effects:
		- partition_occupancy: dict [DATA] - occupancy info with idle/allocated/total counts
	Returns:
		True on success
	"""
	if not (hasattr(state, 'partition_occupancy') and state.partition_occupancy is not None):
		return False
	state.partition_occupancy = f"occupancy_for_gpu_partition"
	return True


def a_prepare_training_environment(state: State, code_source: str, config_source: str) -> Union[State, bool]:
	"""
	Class: Action

	MCP_Tool: rikyu-hpc:fs_upload

	Action signature:
		a_prepare_training_environment(state, code_source, config_source)

	Action parameters:
		code_source: URI/path to training code repository
		config_source: URI/path to training configuration file

	Action purpose:
		Upload training code and configuration to cluster staging directory

	Preconditions:
		- code_source must be a valid path/URI
		- config_source must be a valid path/URI

	Effects:
		- training_env_ready: bool [ENABLER] - set to True when staging complete
		- work_dir: str [DATA] - cluster working directory path
	Returns:
		True on success
	"""
	if not (hasattr(state, 'training_env_ready') and hasattr(state, 'work_dir')):
		return False
	state.training_env_ready = True
	state.work_dir = f"work_dir_for_{code_source}"
	return True


def a_submit_training_job(state: State, model_type: str, num_gpus: int, training_steps: int) -> Union[State, bool]:
	"""
	Class: Action

	MCP_Tool: rikyu-hpc:submit_job

	Action signature:
		a_submit_training_job(state, model_type, num_gpus, training_steps)

	Action parameters:
		model_type: Model type identifier (e.g., 'mdlm-base')
		num_gpus: Number of GPUs to request (must be supported by RIKYU)
		training_steps: Total training steps for the job

	Action purpose:
		Submit containerized training job to Slurm scheduler

	Preconditions:
		- state.training_env_ready must be True
		- num_gpus must be in RIKYU's supported GPU counts

	Effects:
		- job_id: str [DATA] - assigned Slurm job ID
		- job_status: str [DATA] - initial status (QUEUED)
	Returns:
		True on success
	"""
	if not (hasattr(state, 'job_id') and hasattr(state, 'job_status')):
		return False
	state.job_id = f"job_id_for_{model_type}_{num_gpus}gpus"
	state.job_status = 'QUEUED'
	return True


def a_monitor_job_execution(state: State) -> Union[State, bool]:
	"""
	Class: Action

	MCP_Tool: rikyu-hpc:get_job_status

	Action signature:
		a_monitor_job_execution(state)

	Action parameters:
		state: Current planning state (must contain job_id)

	Action purpose:
		Monitor training job execution until reaching terminal state

	Preconditions:
		- state.job_id must be set
		- job must be submitted to scheduler

	Effects:
		- job_status: str [DATA] - terminal status (COMPLETED, FAILED, CANCELLED)
		- results_path: str [DATA] - cluster path containing results/checkpoints
	Returns:
		True on success
	"""
	if not (hasattr(state, 'job_status') and hasattr(state, 'results_path')):
		return False
	state.job_status = 'COMPLETED'
	state.results_path = f"results_for_{state.job_id}"
	return True


def a_collect_training_results(state: State) -> Union[State, bool]:
	"""
	Class: Action

	MCP_Tool: rikyu-hpc:fs_download

	Action signature:
		a_collect_training_results(state)

	Action parameters:
		state: Current planning state (must contain results_path)

	Action purpose:
		Download training results (checkpoints, logs) from cluster to local storage

	Preconditions:
		- state.results_path must be set
		- results must exist on cluster

	Effects:
		- results_downloaded: bool [ENABLER] - set to True when transfer complete
		- local_results_path: str [DATA] - local filesystem download location
	Returns:
		True on success
	"""
	if not (hasattr(state, 'results_downloaded') and hasattr(state, 'local_results_path')):
		return False
	state.results_downloaded = True
	state.local_results_path = f"local_{state.results_path}"
	return True

# =============================================================================
# END: Actions
# =============================================================================


# =============================================================================
# BEGIN: Methods
# =============================================================================

def m_ensure_gpu_available(state: State) -> Union[List[Tuple], bool]:
	"""
	Class: Method

	Method signature:
		m_ensure_gpu_available(state)

	Method parameters:
		state: Current planning state

	Method purpose:
		Ensure GPU resources are available before proceeding with job submission

	Preconditions:
		None

	Task decomposition:
		- a_query_gpu_resources

	Returns:
		Task list or False if decomposition not applicable
	"""
	return [
		('a_query_gpu_resources',),
	]


def m_setup_training_simple(state: State, code_source: str, config_source: str) -> Union[List[Tuple], bool]:
	"""
	Class: Method

	Method signature:
		m_setup_training_simple(state, code_source, config_source)

	Method parameters:
		state: Current planning state
		code_source: Training code repository URI
		config_source: Training configuration file URI

	Method purpose:
		Prepare training environment by uploading code and configuration

	Preconditions:
		None

	Task decomposition:
		- a_prepare_training_environment

	Returns:
		Task list or False if decomposition not applicable
	"""
	return [
		('a_prepare_training_environment', code_source, config_source),
	]


def m_launch_training(state: State, model_type: str, num_gpus: int, training_steps: int) -> Union[List[Tuple], bool]:
	"""
	Class: Method

	Method signature:
		m_launch_training(state, model_type, num_gpus, training_steps)

	Method parameters:
		state: Current planning state
		model_type: Model type identifier
		num_gpus: GPU count for the job
		training_steps: Total training steps

	Method purpose:
		Launch training job if environment is prepared and GPUs available

	Preconditions:
		- state.training_env_ready must be True
		- state.partition_occupancy must contain idle GPUs

	Task decomposition:
		- a_submit_training_job

	Returns:
		Task list or False if preconditions not met
	"""
	if not (hasattr(state, 'training_env_ready') and state.training_env_ready):
		return False
	if not (hasattr(state, 'partition_occupancy') and state.partition_occupancy):
		return False
	return [
		('a_submit_training_job', model_type, num_gpus, training_steps),
	]


def m_await_training_completion(state: State) -> Union[List[Tuple], bool]:
	"""
	Class: Method

	Method signature:
		m_await_training_completion(state)

	Method parameters:
		state: Current planning state (must contain job_id)

	Method purpose:
		Wait for training job completion and collect results

	Preconditions:
		- state.job_id must be set from previous job submission

	Task decomposition:
		- a_monitor_job_execution
		- a_collect_training_results

	Returns:
		Task list or False if preconditions not met
	"""
	if not (hasattr(state, 'job_id') and state.job_id):
		return False
	return [
		('a_monitor_job_execution',),
		('a_collect_training_results',),
	]


def m_full_training_workflow(state: State, code_source: str, config_source: str,
							  model_type: str, num_gpus: int, training_steps: int) -> Union[List[Tuple], bool]:
	"""
	Class: Method

	Method signature:
		m_full_training_workflow(state, code_source, config_source, model_type, num_gpus, training_steps)

	Method parameters:
		state: Current planning state
		code_source: Training code repository URI
		config_source: Training configuration file URI
		model_type: Model type identifier
		num_gpus: GPU count for the job
		training_steps: Total training steps

	Method purpose:
		Orchestrate complete containerized training workflow from setup to result collection

	Preconditions:
		None (handles all prerequisite checks through sub-method conditions)

	Task decomposition:
		- ensure_gpu_available
		- setup_training
		- launch_training
		- await_training_completion

	Returns:
		Task list for complete workflow
	"""
	return [
		('ensure_gpu_available',),
		('setup_training', code_source, config_source),
		('launch_training', model_type, num_gpus, training_steps),
		('await_training_completion',),
	]

# =============================================================================
# END: Methods
# =============================================================================


# =============================================================================
# DECLARE ACTIONS TO DOMAIN
# =============================================================================
declare_actions(
	a_query_gpu_resources,
	a_prepare_training_environment,
	a_submit_training_job,
	a_monitor_job_execution,
	a_collect_training_results,
)


# =============================================================================
# DECLARE METHODS TO DOMAIN
# =============================================================================
declare_task_methods("ensure_gpu_available", m_ensure_gpu_available)
declare_task_methods("setup_training", m_setup_training_simple)
declare_task_methods("launch_training", m_launch_training)
declare_task_methods("await_training_completion", m_await_training_completion)
declare_task_methods("train_containerized_model", m_full_training_workflow)
