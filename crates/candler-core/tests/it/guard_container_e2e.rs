//! The real-Docker guards of the container runner, ignored by default and run in the
//! `container-e2e` nextest group (`--run-ignored only`).
//!
//! `container_run_e2e`, `container_stop_e2e`, `env_image_e2e`, `controls_e2e` and `version_probe_e2e` hold their guards. `gateway_e2e`
//! and `supervision_gates_e2e` each hold one ignored placeholder that panics, so the container
//! gate cannot pass before they hold their own guards.
#![cfg(unix)]

#[path = "guard_container_e2e/container_run_e2e.rs"]
mod container_run_e2e;
#[path = "guard_container_e2e/container_stop_e2e.rs"]
mod container_stop_e2e;
#[path = "guard_container_e2e/controls_e2e.rs"]
mod controls_e2e_runs;
#[path = "guard_container_e2e/env_image_e2e.rs"]
mod env_image_e2e;
#[path = "guard_container_e2e/gateway_e2e.rs"]
mod gateway_e2e;
#[path = "guard_container_e2e/supervision_gates_e2e.rs"]
mod supervision_gates_e2e;
#[cfg(test)]
#[path = "guard_container_e2e/version_probe_e2e.rs"]
mod version_probe_e2e;
