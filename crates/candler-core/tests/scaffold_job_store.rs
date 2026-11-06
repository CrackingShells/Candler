//! Class-A development tests of the job store; deleted by the last commit of the leaf.
#![cfg(test)]

mod support;

use std::fs;

use candler_core::jobs::dir::JobDir;
use candler_core::jobs::view::{ChildEntry, JobView};
use candler_core::store::layout::StoreLayout;
use candler_schemas::execution::JobState;
use support::job_store::{child_json, container_json, container_record, job_id, job_record};
use tempfile::TempDir;

#[test]
fn children_round_trip_and_unreadable() {
    let root = TempDir::new().expect("a store root");
    let layout = StoreLayout::new(root.path());
    let id = job_id(1);
    let dir =
        JobDir::create(&layout, id.clone(), &job_record(&id, JobState::Running)).expect("create");
    assert!(dir.control_dir().is_dir());
    dir.write_child(&container_record("abcdef012345abcd", &id, "r"))
        .expect("write child");
    let kids = dir.children_dir();
    fs::write(kids.join("4242.json"), child_json(4242, "m").to_string()).expect("write");
    fs::write(kids.join("bad.json"), "{\"schema\":\"child-record/1\"").expect("write");
    fs::write(kids.join("odd.json"), "{\"schema\":\"other/9\"}").expect("write");
    fs::write(kids.join("x.json.1.tmp"), "{}").expect("write");
    fs::write(
        kids.join("mode.json"),
        container_json("abcdef012345abcd", &id, "r").to_string(),
    )
    .expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(kids.join("mode.json"), fs::Permissions::from_mode(0)).expect("chmod");
    }
    fs::write(
        dir.control_dir().join("1.json"),
        child_json(1, "control").to_string(),
    )
    .expect("write");
    let entries = dir.children().expect("children");
    let kinds: Vec<&str> = entries
        .iter()
        .map(|e| match e {
            ChildEntry::Process(_) => "process",
            ChildEntry::Container(_) => "container",
            ChildEntry::Unreadable { .. } => "unreadable",
            _ => "?",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "process",
            "unreadable",
            "container",
            "unreadable",
            "unreadable",
            "unreadable"
        ]
    );
    dir.set_state(JobState::Ended).expect("set state");
    assert_eq!(dir.record().expect("record").state(), JobState::Ended);
}
