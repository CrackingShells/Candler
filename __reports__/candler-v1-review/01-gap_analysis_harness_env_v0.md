# Gap Analysis: candler-v1 — the core holds Claude Code and a Python venv as built-in concepts

Source of amendment A3. It is logged in `__roadmap__/candler-v1/contracts/parity/alignment/surfaces/README.md` (M4) and `surfaces/isolation/README.md` (M5). A3 adds no node. It adds a Success Gate and a planning item to the leaves `m4_session_runner.md` and `isolation/m5_container_isolation.md`. Eliott approved it on 2026-11-10 ("yes to all"). Findings H1–H3 of `00-findings_v0.md` are the source.

## Problem Statement

R01 puts what is specific to one harness in that harness's driver crate, and what is specific to one agent environment in that environment's implementation. M2b's Docker backend put both in `candler-core`:

- it runs Claude Code's version command;
- it names Claude Code's gateway and config variables;
- it parses Claude Code's `system/init` event;
- it assumes the agent's environment is a Python 3.12 venv at `/opt/env`.

M4 adds a second harness, and M5 adds plug-in-declared environments. Neither leaf has a gate that would make either milestone move this content out. A second driver could pass its fixture suite while the core still runs `claude --version` for every image.

## Evidence

On `main` at 656be71 (M2a merged), re-checked 2026-11-10:

| id | Where | What |
|:--|:--|:--|
| H1 | `crates/candler-core/src/isolation/image.rs:25,34,119,854,895` | The core runs `claude --version` in a control container and parses its output. The driver trait already has `HarnessDriver::version_probe` (`candler-harness/src/driver.rs:55`), and the core does not use it. |
| H1 | `isolation/container/run.rs:29-38` | `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY` as the gateway's variable names |
| H1 | `isolation/container/mounts.rs:18` | `CLAUDE_CONFIG_DIR` |
| H1 | `runner/evidence/census.rs:35` | the census reads Claude Code's `system/init` event (`subtype == "init"`) |
| H2 | `isolation/layout.rs:32-35` | `CONTAINER_ENV = "/opt/env"`, documented as "the agent's Python environment" |
| H2 | `isolation/image.rs:827` | `uv pip compile --python-version 3.12` |
| H2 | `assets/docker/env/Dockerfile:8,15` | `uv venv /opt/env --python 3.12`, `ENV VIRTUAL_ENV=/opt/env` |
| H2 | `runner/evidence/tree.rs:31,59` | `ENV_MARKER = "pyvenv.cfg"` decides what the evidence tree excludes. No test covers it (review E1). |
| H3 | `crates/candler/src/commands/run.rs:111` | the binary finds `assets/docker` via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker")`, so an installed `candler` has no assets |

The I1 guard denies domain names only. After A2 it will deny HTN role names too, but no harness or environment name.

## Root Cause

- **The parity gate.** M2b's leaf asked for runs equivalent to the HTN reference, which runs Claude Code in a Python venv. One harness and one environment were enough to pass it.
- **The planned deviation.** M2b's architecture report planned the Claude Code naming in the core (refuted as a breach in `00-findings_v0.md`), but nothing scheduled its removal.
- **Gates that test addition, not removal.** M4's gates test that a second driver works. M5's gates test that a declared environment is provisioned. Neither tests that the first harness and the first environment have left the core.

## Impact Assessment

- **Scope.**
  - Included:
    - the version probe, the gateway's and the config's variable names, and the census event parser, behind the driver (M4);
    - a harness deny-list in the I1 guard (M4);
    - a declared kind for the agent environment, with the venv paths, variables, markers and Python version inside that kind's implementation (M5);
    - the binary's assets found without the source tree (M5).
  - Excluded:
    - the evaluator environment, which is M2a's `python/candler-evaluator`;
    - the `candler` binary's wiring of the drivers, which may name them because it composes the crates.
- **Dependencies.**
  - M4's guard builds on the segment-matching guard that A2 adds in M2c.
  - M5's environment kind is the declaration field its Step 1 already adds to the plug-in manifest.
  - E1's tree-exclusion test, in the code-fix PR, pins today's behaviour before M5 moves it.
- **Risk.**
  - **Cell digests.** Moving the probe or the variable names must not move a cell digest. The harness version recorded in `comparable` must stay byte-identical for Claude Code.
  - **The deny-list.** It must not trip on the driver crate's own name in comments or paths outside `candler-core/src`. It scans `candler-core/src` and `candler-schemas/src` only.
  - **Waiting.** If A3 waits, OpenCode's driver will need its own branches inside the core, and the second harness will make the coupling harder to remove.

## Proposed Solution

1. **M4 gate:** "`candler-core/src` names no harness; the image's harness version, the census and the gateway's variable names come from the driver." The I1 guard gains a harness deny-list (`claude`, `anthropic`, `opencode` as identifier segments), with a planted bite in the core.
2. **M5 gate:** "The agent environment has a declared kind; the core holds no Python path, variable or marker outside that kind's implementation." A `candler` binary run outside the source tree finds its assets.

## Recommendations

1. Write both gates into the leaves now, before either milestone plans, so their plans include the work.
2. R01 §Harness drivers and §Isolation say which facts a driver or an environment kind supplies. Today's coupling came in because R01 was silent on this.
3. M3 owns packaging. If M3 already embeds or installs the assets, M5's H3 clause is met by that, and M5 only checks it.
