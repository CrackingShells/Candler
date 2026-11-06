//! The run's tree: the files it produced, by store-relative path.

use candler_schemas::execution::{ExecutionError, RunTree, TreeEntry};
use candler_schemas::primitives::RelPath;

use std::collections::BTreeSet;

use super::{Evidence, Inputs};

/// Why a tree could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EvidenceError {
    /// An output path is not a relative path.
    #[error("output path `{path}` is not a relative path: {detail}")]
    Path {
        /// The path.
        path: String,
        /// What is wrong with it.
        detail: String,
    },
    /// The outputs do not form a tree.
    #[error("the outputs form no tree: {detail}")]
    Tree {
        /// What is wrong.
        detail: String,
    },
}

/// The name of the marker file of a Python environment.
const ENV_MARKER: &str = "pyvenv.cfg";

/// Whether `path` is `root` or lies under it; an empty `root` is the workspace itself.
fn under(path: &str, root: &str) -> bool {
    root.is_empty()
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The tree of the run: one file entry per output, sorted by path. The workspace path is a
/// container path and is never stored. No path under a directory that holds a `pyvenv.cfg`
/// (the marker file included) is listed, and none under a read-only input mount point, which
/// `inputs` names by the workspace path of each input.
///
/// # Errors
/// [`EvidenceError::Path`] for an output path that is not relative, and
/// [`EvidenceError::Tree`] for outputs that repeat a path or nest a path under a file.
pub fn tree_json(evidence: &Evidence, inputs: &Inputs) -> Result<RunTree, EvidenceError> {
    let envs: BTreeSet<&str> = evidence
        .outputs
        .iter()
        .filter_map(|output| {
            let (dir, name) = output
                .path
                .rsplit_once('/')
                .unwrap_or(("", output.path.as_str())); // BORROW: explicit .as_str()
            (name == ENV_MARKER).then_some(dir)
        })
        .collect();
    let mut builder = RunTree::builder();
    for output in &evidence.outputs {
        let hidden = envs.iter().any(|dir| under(output.path.as_str(), dir)) // BORROW: explicit .as_str()
            || inputs
                .digests
                .iter()
                .any(|input| under(output.path.as_str(), input.path.as_str())); // BORROW: explicit .as_str()
        if hidden {
            continue;
        }
        let path = output
            .path
            .parse::<RelPath>() // BORROW: explicit parse of the stored path text
            .map_err(|error| EvidenceError::Path {
                path: output.path.clone(), // BORROW: .clone() gives the error its own text
                detail: error.to_string(), // BORROW: .to_string() gives owned text
            })?;
        builder = builder.entry(TreeEntry::file(path, output.digest.clone(), false)); // BORROW: .clone() copies the digest
    }
    builder
        .build()
        .map_err(|error: ExecutionError| EvidenceError::Tree {
            detail: error.to_string(), // BORROW: .to_string() gives owned text
        })
}

#[cfg(test)]
mod scaffold {
    use super::*;
    use crate::runner::evidence::{InputDigest, OutputDigest};

    #[test]
    fn env_dirs_and_input_mounts_are_left_out() {
        let digest: candler_schemas::primitives::Digest =
            format!("sha256:{}", "c".repeat(64)).parse().unwrap();
        let ev = |paths: &[&str]| Evidence {
            sessions: Vec::new(),
            events: Vec::new(),
            outputs: paths
                .iter()
                .map(|p| OutputDigest {
                    path: (*p).to_owned(),
                    digest: digest.clone(),
                })
                .collect(),
            tree: RunTree::builder().build().unwrap(),
        };
        let names = |t: RunTree| -> Vec<String> {
            t.entries().iter().map(|e| e.path().to_string()).collect()
        };
        let mount = Inputs {
            digests: vec![InputDigest {
                path: "in/data.txt".to_owned(),
                digest: digest.clone(),
            }],
        };
        let all = ev(&[
            "out/a.txt",
            "venv/pyvenv.cfg",
            "venv/lib/y.py",
            "venvx/z.py",
            "in/data.txt",
            "in/data.txt.bak",
            "deep/env/pyvenv.cfg",
            "deep/env/bin/p",
            "deep/keep",
        ]);
        assert_eq!(
            names(tree_json(&all, &mount).unwrap()),
            ["deep/keep", "in/data.txt.bak", "out/a.txt", "venvx/z.py"]
        );
        let none = Inputs::default();
        assert_eq!(
            names(tree_json(&ev(&["in/data.txt", "a"]), &none).unwrap()),
            ["a", "in/data.txt"]
        );
        assert_eq!(
            names(tree_json(&ev(&["pyvenv.cfg", "a"]), &none).unwrap()),
            Vec::<String>::new()
        );
        assert!(matches!(
            tree_json(&ev(&["/abs"]), &none),
            Err(EvidenceError::Path { .. })
        ));
        assert!(matches!(
            tree_json(&ev(&["a", "a/b"]), &none),
            Err(EvidenceError::Tree { .. })
        ));
    }
}
