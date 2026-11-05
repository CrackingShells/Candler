"""
Problem definitions for containerized training of a specialized MDLM diffusion model.
-- Generated 2026-08-06

This file defines initial states for cont_train workflows.
The workflow demonstrates coordination between MCP servers:
  - rikyu-hpc: GPU resource checking, job submission, monitoring, and result transfer

Scenarios:
  - basic_training: Fresh training with available GPUs -> ~5 actions
  - gpu_constrained: Training under GPU resource contention -> ~5 actions
  - monitor_existing: Resume monitoring from running job -> ~2 actions
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
		print("Please install gtpyhop using: pip install gtpyhop-core")
		sys.exit(1)


# =============================================================================
# HELPER FUNCTIONS
# =============================================================================

def h_create_initial_state(name: str, partition_occupancy: dict = None,
						   job_id: str = "", training_env_ready: bool = False) -> State:
	"""
	Helper function to create initial state for cont_train domain.

	Args:
		name: State name identifier
		partition_occupancy: GPU partition occupancy dict (optional)
		job_id: Existing job ID for resume scenarios (optional)
		training_env_ready: Whether environment is pre-staged (optional)

	Returns:
		Configured gtpyhop.State object with all required properties
	"""
	state = State(name)

	# Domain-specific inputs
	state.partition_occupancy = partition_occupancy or {}
	state.job_id = job_id
	state.training_env_ready = training_env_ready

	# DATA containers (all start empty)
	state.work_dir = ""
	state.job_status = ""
	state.results_path = ""
	state.local_results_path = ""

	# ENABLER flags (all start False unless overridden)
	state.results_downloaded = False

	# Execution tracking
	state.servers_called = []
	state.api_calls = 0

	return state


# =============================================================================
# SCENARIOS
# =============================================================================

problems: Dict[str, Tuple[State, List[Tuple], str]] = {}

# BEGIN: Domain: cont_train

# BEGIN: Scenario: basic_training
# Configuration
_occupancy = {'idle': 20, 'allocated': 380, 'other': 0, 'total': 400}
_code_source = 'github.com/example/mdlm-code'
_config_source = 'configs/diffusion-v2.yaml'
_model_type = 'mdlm-base'
_num_gpus = 8
_training_steps = 10000

# State
initial_state_basic_training = h_create_initial_state(
	'basic_training',
	partition_occupancy=_occupancy,
	job_id="",
	training_env_ready=False
)

# Problem
problems['basic_training'] = (
	initial_state_basic_training,
	[('train_containerized_model', _code_source, _config_source, _model_type, _num_gpus, _training_steps)],
	f"""Basic Training Scenario

Fresh training workflow with adequate GPU availability.
GPUs available, environment needs staging, full workflow execution from setup to results.
Expected action sequence: query_gpu_resources -> prepare_environment -> submit_job -> monitor_execution -> collect_results
~5 actions to complete."""
)
# END: Scenario

# BEGIN: Scenario: gpu_constrained
# Configuration
_occupancy_constrained = {'idle': 2, 'allocated': 398, 'other': 0, 'total': 400}
_code_source = 'github.com/example/mdlm-code'
_config_source = 'configs/diffusion-v2-compact.yaml'
_model_type = 'mdlm-small'
_num_gpus = 4
_training_steps = 5000

# State
initial_state_gpu_constrained = h_create_initial_state(
	'gpu_constrained',
	partition_occupancy=_occupancy_constrained,
	job_id="",
	training_env_ready=False
)

# Problem
problems['gpu_constrained'] = (
	initial_state_gpu_constrained,
	[('train_containerized_model', _code_source, _config_source, _model_type, _num_gpus, _training_steps)],
	f"""GPU-Constrained Training Scenario

Training workflow under GPU resource contention.
GPU availability is limited (2/400 idle), compact model requested, workflow must proceed despite constraints.
Tests condition checking in launch_training method (occupancy check before job submission).
~5 actions, with m_launch_training potentially returning False if occupancy insufficient."""
)
# END: Scenario

# BEGIN: Scenario: monitor_existing
# Configuration
_occupancy = {'idle': 15, 'allocated': 385, 'other': 0, 'total': 400}
_existing_job_id = 'training_job_2024_08_06_001'

# State
initial_state_monitor_existing = h_create_initial_state(
	'monitor_existing',
	partition_occupancy=_occupancy,
	job_id=_existing_job_id,
	training_env_ready=True
)
initial_state_monitor_existing.work_dir = 'work_dir_for_github.com/example/mdlm-code'
initial_state_monitor_existing.job_status = 'RUNNING'

# Problem
problems['monitor_existing'] = (
	initial_state_monitor_existing,
	[('await_training_completion',)],
	f"""Resume from Existing Job Scenario

Monitoring and result collection for an already-submitted and running job.
Environment is pre-staged, job already submitted and running, only monitoring + collection required.
Tests resumption from partial workflow state and skipping unnecessary setup steps.
~2 actions (monitor_job_execution -> collect_training_results)."""
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
