"""
Containerized Training Domain - Problem Definitions. -- Generated

This module defines 3 problem scenarios for the cont-train domain:
  1. Quick Training Run: Health check → Resource check → Setup → Submit (no monitoring)
  2. Full Training with Monitoring: Complete pipeline including job monitoring
  3. Health Check and Documentation: Diagnostic run with cluster info gathering
"""

import sys
import os
from typing import Dict

# Secure gtpyhop import
try:
    from gtpyhop import State
except ImportError:
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
    from gtpyhop import State


# =============================================================================
# BEGIN: Domain: cont-train
# =============================================================================

# =============================================================================
# BEGIN: Scenario: quick_training_setup
# =============================================================================

def h_quick_training_setup() -> State:
    """
    Problem: Quick Training Run

    Scenario:
        User wants to verify cluster health, check resources, and submit a training job
        without monitoring completion. Suitable for quick validation runs.

    Goal:
        Submit training job to Slurm cluster after basic setup and resource checks.

    State initialization:
        - setup_complete: False → True after setup_training_environment completes
        - training_submitted: False → True after submit_training_job completes
        - All precondition variables initialized to False

    Workflow:
        1. Run health checks and load facility info
        2. Check resource availability
        3. Create working directory and upload config
        4. Submit training job

    Success criteria:
        - state.system_healthy == True
        - state.resources_checked == True
        - state.config_uploaded == True
        - state.job_submitted == True
        - state.job_id is set and non-empty
    """
    state = State("scenario_quick_training_setup")

    # Pre-flight state
    state.setup_complete = False
    state.training_submitted = False

    # Initialize precondition gates to False
    state.system_healthy = False
    state.facility_loaded = False
    state.resources_checked = False
    state.directory_created = False
    state.config_uploaded = False
    state.job_submitted = False

    # Initialize data containers
    state.training_dir = None
    state.results_path = None

    return state


# =============================================================================
# END: Scenario
# =============================================================================

# =============================================================================
# BEGIN: Scenario: full_training_with_monitoring
# =============================================================================

def h_full_training_with_monitoring() -> State:
    """
    Problem: Full Training Pipeline

    Scenario:
        Complete end-to-end training workflow: health checks, setup, job submission,
        monitoring until completion, and result download + verification.

    Goal:
        Execute full_training_pipeline and retrieve verified training results.

    State initialization:
        - All precondition variables initialized to False
        - All data containers initialized to None or empty

    Workflow:
        1. Run health checks and facility info loading
        2. Check resource availability
        3. Create working directory and upload configuration
        4. Submit training job
        5. Poll job status until completion (state.training_complete = True)
        6. List and download artifacts
        7. Verify transfer integrity

    Success criteria:
        - state.system_healthy == True
        - state.resources_checked == True
        - state.job_submitted == True
        - state.training_complete == True
        - state.results_downloaded == True
        - state.transfer_verified == True
        - state.results_path contains valid path string
    """
    state = State("scenario_full_training_with_monitoring")

    # Initialize precondition gates to False
    state.system_healthy = False
    state.facility_loaded = False
    state.resources_checked = False
    state.directory_created = False
    state.config_uploaded = False
    state.job_submitted = False
    state.training_complete = False
    state.artifacts_ready = False
    state.results_downloaded = False

    # Workflow gates
    state.setup_complete = False
    state.training_submitted = False
    state.results_retrieved = False

    # Initialize data containers
    state.training_dir = None
    state.job_id = None
    state.results_path = None

    return state


# =============================================================================
# END: Scenario
# =============================================================================

# =============================================================================
# BEGIN: Scenario: health_check_and_docs
# =============================================================================

def h_health_check_and_docs() -> State:
    """
    Problem: Health Check and Documentation Search

    Scenario:
        Diagnostic and informational run: verify cluster health, check resource
        occupancy, and search documentation for training best practices or
        troubleshooting guidance.

    Goal:
        Complete health checks and retrieve relevant documentation.

    State initialization:
        - All precondition variables initialized to False
        - All data containers initialized to None or empty

    Workflow:
        1. Run health check
        2. Load facility configuration
        3. Check drained nodes
        4. Check resource availability
        5. Search documentation for "MDLM training best practices" (example query)

    Success criteria:
        - state.system_healthy == True
        - state.facility_loaded == True
        - state.resources_checked == True
        - state.drained_nodes list exists (may be empty)
        - state.docs_searched == True
        - state.doc_results contains non-empty dict
    """
    state = State("scenario_health_check_and_docs")

    # Initialize precondition gates to False
    state.system_healthy = False
    state.facility_loaded = False
    state.resources_checked = False

    # Diagnostic state gates
    state.docs_searched = False

    # Initialize data containers
    state.training_dir = None
    state.results_path = None

    return state


# =============================================================================
# END: Scenario
# =============================================================================

# =============================================================================
# END: Domain
# =============================================================================


# =============================================================================
# Problem Registry
# =============================================================================

problems: Dict = {}


def get_problems() -> Dict:
    """
    Get all defined problems for the cont-train domain.

    Each problem is a tuple of (state, todo_list) where:
    - state: Initial State object with all precondition variables initialized
    - todo_list: List of top-level tasks to plan/execute

    Returns:
        Dictionary of problems keyed by scenario name
    """
    # Scenario 1: Quick Training Setup and Submission
    state1 = h_quick_training_setup()
    todo1 = [("health_check_and_setup",), ("a_check_resource_availability",),
             ("setup_training_environment", "/tmp/training_run_001", "/tmp/config.yaml"),
             ("submit_training_job_with_monitoring", "/tmp/job_spec.json")]

    # Scenario 2: Full Training Pipeline
    state2 = h_full_training_with_monitoring()
    todo2 = [("full_training_pipeline", "/tmp/training_run_002", "/tmp/config.yaml",
              "/tmp/job_spec.json", "/tmp/results")]

    # Scenario 3: Health Check and Documentation
    state3 = h_health_check_and_docs()
    todo3 = [("health_check_and_setup",), ("a_search_documentation", "MDLM training best practices")]

    return {
        "quick_training_setup": (state1, todo1),
        "full_training_with_monitoring": (state2, todo2),
        "health_check_and_docs": (state3, todo3),
    }


if __name__ == "__main__":
    # Quick test: instantiate all problems
    print("Instantiating all problems...\n")
    for key, problem_spec in get_problems().items():
        state, todo = problem_spec
        print(f"Scenario: {key}")
        print(f"  State: {state.name}")
        print(f"  Todo list: {todo}")
        print()
