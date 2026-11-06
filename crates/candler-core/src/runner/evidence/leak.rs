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

#[cfg(test)]
mod scaffold {
    use super::*;
    use crate::runner::evidence::{InputDigest, OutputDigest};
    use candler_schemas::execution::RunTree;
    use candler_schemas::primitives::Digest;

    fn d(c: char) -> Digest {
        format!("sha256:{}", c.to_string().repeat(64))
            .parse()
            .unwrap()
    }

    fn evidence(out: &[(&str, char)]) -> Evidence {
        Evidence {
            sessions: Vec::new(),
            events: Vec::new(),
            outputs: out
                .iter()
                .map(|(p, c)| OutputDigest {
                    path: (*p).to_owned(),
                    digest: d(*c),
                })
                .collect(),
            tree: RunTree::builder().build().unwrap(),
        }
    }

    #[test]
    fn equal_digest_under_another_name_leaks() {
        let inputs = Inputs {
            digests: vec![InputDigest {
                path: "in.txt".to_owned(),
                digest: d('a'),
            }],
        };
        assert_eq!(
            input_leak(&evidence(&[("renamed.txt", 'a')]), &inputs),
            Some(InvalidReason::InputLeak)
        );
        assert_eq!(input_leak(&evidence(&[("in.txt", 'b')]), &inputs), None);
        assert_eq!(
            input_leak(&evidence(&[("in.txt", 'a')]), &Inputs::default()),
            None
        );
    }
}
