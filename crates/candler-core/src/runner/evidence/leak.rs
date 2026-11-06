//! The input-leak check: an output that equals an input by content.

use candler_schemas::execution::InvalidReason;

use super::{Evidence, Inputs};

/// Returns [`InvalidReason::InputLeak`] when an output has the same digest as an input. Only
/// digests are compared: a copied input is renamed, and a different file may share a size. It
/// runs on collected output only.
#[must_use]
pub fn input_leak(evidence: &Evidence, inputs: &Inputs) -> Option<InvalidReason> {
    let leaked = evidence.outputs.iter().any(|output| {
        inputs
            .digests
            .iter()
            .any(|input| input.digest == output.digest)
    });
    leaked.then_some(InvalidReason::InputLeak)
}
