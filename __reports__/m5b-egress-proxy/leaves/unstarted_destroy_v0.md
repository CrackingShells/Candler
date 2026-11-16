# unstarted_destroy leaf report (v0, PARTIAL)

## Probe P6
Question: how is a record failure injected in `DockerIsolation::run_request` without changing
`src/`? Answer: `DockerIsolation::with_recorder` takes a `ChildRecorder`, which the test builds
over a `JobDir`; the record write goes to `JobDir::children_dir()`. Replacing that directory by a
regular file makes the write fail, and the recording fake needs nothing more. A start-request
failure needs the driver's `attach` to refuse; the fake had no such switch, so
`RecordingDocker::with_attach_refused` was added (`isolation/testing.rs`, owned by the A1 widening).
Decision: the `record` row uses the file-in-the-way fixture, the `start_request` row the new switch.
