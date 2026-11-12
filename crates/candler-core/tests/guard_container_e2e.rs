//! The real-Docker guards of the container runner, ignored by default and run in the
//! `container-e2e` nextest group (`--run-ignored only`).
//!
//! `container_run_e2e`, `container_stop_e2e`, `env_image_e2e` and `session_e2e` hold their guards. `gateway_e2e`
//! and `supervision_gates_e2e` each hold one ignored placeholder that panics, so the container
//! gate cannot pass before they hold their own guards.
#![cfg(unix)]
#![allow(
    clippy::duplicate_mod,
    reason = "each e2e module includes the support file it uses, and `support` holds them all"
)]

#[path = "e2e/container_run_e2e.rs"]
mod container_run_e2e;
#[path = "e2e/container_stop_e2e.rs"]
mod container_stop_e2e;
#[path = "e2e/env_image_e2e.rs"]
mod env_image_e2e;
#[path = "e2e/gateway_e2e.rs"]
mod gateway_e2e;
#[path = "e2e/session_e2e.rs"]
mod session_e2e_runs;
#[path = "e2e/supervision_gates_e2e.rs"]
mod supervision_gates_e2e;
#[cfg(test)]
mod support;
