//! The real-Docker guards of M2b, ignored by default and run in the `container-e2e` nextest group
//! (`--run-ignored only`). Each e2e leaf fills its own module.
//!
//! Each module holds one ignored placeholder that panics, so the container gate cannot pass before
//! the leaf has replaced it with its own guards.
#![cfg(unix)]

#[path = "container_run_e2e.rs"]
mod container_run_e2e;
#[path = "container_stop_e2e.rs"]
mod container_stop_e2e;
#[path = "gateway_e2e.rs"]
mod gateway_e2e;
#[path = "supervision_gates_e2e.rs"]
mod supervision_gates_e2e;
