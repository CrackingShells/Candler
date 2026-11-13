//! The integration tests of this crate, one binary: one module per former test file.
#![cfg(test)]

mod support;

/// The binary's driver registry, compiled from its source so the capability page reads the drivers
/// the binary registers. The registry reaches the binary's command layer only to build the refusal
/// of an unknown harness, which these stand-ins give.
#[path = "../../src/drivers.rs"]
#[expect(
    dead_code,
    reason = "the page reads the registry's drivers, not every method"
)]
mod drivers;

/// The stand-ins for the binary's command layer that `drivers` names.
#[expect(
    dead_code,
    reason = "the registry only builds the refusal it never reads here"
)]
mod commands {
    /// A refusal of a command.
    #[derive(Debug)]
    pub struct CliError;

    impl CliError {
        /// The error of a refusal.
        pub fn refused_by(_: refusal::CliRefusal) -> Self {
            Self
        }
    }

    /// The binary's refusals.
    pub mod refusal {
        /// A refusal of the binary.
        #[derive(Debug)]
        pub enum CliRefusal {
            /// A harness that no driver answers to.
            UnknownHarness {
                /// The name asked for.
                name: String,
                /// The names registered.
                registered: String,
            },
        }
    }
}

mod contract_pages;
mod guard_analyze;
mod guard_analyze_golden;
mod guard_cli;
mod guard_cli_diff_refusal;
mod guard_cli_e2e;
mod guard_cli_evaluate;
mod guard_cli_kill;
mod guard_cli_kill_live;
mod guard_cli_observe;
mod guard_cli_refusal;
mod guard_cli_run;
mod guard_cli_surface;
mod guard_eval_infra;
mod guard_golden_parity;
mod guard_ingest;
mod guard_input_seam;
mod guard_inspect;
mod guard_oracle_ac;
mod guard_registry;
