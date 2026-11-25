//! The needs one request carries when it serves several kinds.

use candler_schemas::evaluation::Needs;

/// The needs of one request that serves every kind of `kinds`: a flag is set when any kind sets
/// it, and each list holds the members of the kinds once, in the order they are first named.
#[must_use]
pub fn union_needs<'a, I>(kinds: I) -> Needs
where
    I: IntoIterator<Item = &'a Needs>,
{
    let mut union = Needs::default();
    for more in kinds {
        union.trace |= more.trace;
        union.raw |= more.raw;
        union.manifest |= more.manifest;
        for artifact in &more.artifacts {
            if !union.artifacts.contains(artifact) {
                union.artifacts.push(artifact.clone()); // BORROW: the union owns its globs
            }
        }
        for kind in &more.snapshots {
            if !union.snapshots.contains(kind) {
                union.snapshots.push(kind.clone()); // BORROW: the union owns its kinds
            }
        }
        for kind in &more.inputs {
            if !union.inputs.contains(kind) {
                union.inputs.push(kind.clone()); // BORROW: the union owns its kinds
            }
        }
        for path in &more.metadata {
            if !union.metadata.contains(path) {
                union.metadata.push(path.clone()); // BORROW: the union owns its paths
            }
        }
    }
    union
}
