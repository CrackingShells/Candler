//! The integration tests of this crate, one binary: one module per former test file.
#![cfg(test)]

mod catalog_fixtures;
mod catalog_properties;
mod guard_cell_digest;
mod guard_context_digest;
mod guard_evaluator_protocol;
mod guard_event_kinds;
mod guard_execution_wire;
mod guard_experiment_file;
mod guard_import_origin;
mod guard_jcs;
mod guard_pattern_vectors;
mod guard_refusal;
mod guard_rpc_wire;
mod guard_trace_wire;
mod guard_wire_shapes;
mod outcome_fixtures;
mod outcome_properties;
mod schema_drift;
