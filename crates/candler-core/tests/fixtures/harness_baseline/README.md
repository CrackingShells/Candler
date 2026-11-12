# Harness baseline

What the core produced for a Claude Code run at commit cf3de30 (`v0.7.0`), recorded there and
never re-recorded. The guards in `tests/guard_harness_facts.rs` compare the current core with
these files byte for byte; a difference is a defect of the change under test.

- `argv.json`: `headless_run`, the `docker run` argv of a reference cell (every flag of the
  headless command line set; the scene's root is written `<root>`), and
  `control_version_probe`, the argv of the control container that reads the harness version,
  with the `version` it printed.
- `census.json`: the three `census.*` values of every Claude Code stream fixture (the gzip
  streams of `candler-harness-claude-code/tests/fixtures/streams` and the two recorded session
  stdouts), or `null` where the core records no census.
- `comparable/*.json`: the `comparable` of every cell of an experiment file, resolved and planned
  as `candler run` does it, over an image service that reports Claude Code 2.1.311.

Recorded by running the producers of `tests/support/harness_facts.rs`, unchanged, in a scratch
checkout of cf3de30, with the two session fixtures copied in (they are newer than cf3de30).

The same producers recorded the same bytes at a26fe53 (`v0.6.0`): the move from a26fe53 to
cf3de30 changed none of these values.
