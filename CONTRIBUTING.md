# Contributing to Candler

Humans and agents both commit here. The rules below are what the commit-lint and release machinery enforce once they are installed; until then they are the convention every commit follows.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/), with a mandatory scope:

```
<type>(<scope>): <why this change exists, imperative, lowercase>

<body: the reasoning, trade-offs and context the diff cannot show>

<footers: BREAKING CHANGE:, Resolves #n, Co-Authored-By:>
```

- The subject says why, not only what. Write a body whenever the reason does not fit in the subject.
- Subject: imperative mood, lowercase after the colon, no trailing period, at most 72 characters.
- Body lines wrap at about 72 characters.
- A breaking change uses `!` after the scope or a `BREAKING CHANGE:` footer, and its body states the migration path.
- Agent-authored commits end with a `Co-Authored-By:` trailer naming the model.

### Types

| Type | Use for |
|---|---|
| `feat` | New capability |
| `fix` | A defect corrected |
| `perf` | Faster or cheaper, same behaviour (with a measurement in the body) |
| `refactor` | Restructuring with no behaviour change |
| `style` | Formatting only |
| `test` | Tests and fixtures only |
| `docs` | Documentation, reports and roadmaps |
| `build` | Packaging, dependencies, environments |
| `ci` | CI workflows |
| `chore` | Repository upkeep that fits nothing above |
| `revert` | Reverting an earlier commit (body names it) |
| `merge` | The merge commit that integrates a branch |

### Scopes

A scope names the topic of the commit, never the action or a layer (`core`, `misc`, `update` are not scopes). Related commits share one scope so the history reads as a story. Use these, and add a new one only when no existing scope fits:

| Area | Scopes |
|---|---|
| Operation API | `api`, `resolve`, `jobs`, `budgets` |
| Execution | `experiment`, `orchestrator`, `runner`, `isolation`, `injection`, `census`, `catalog` |
| Ingestion | `trace-model`, `adapter-claude-code` (one `adapter-<harness>` per harness), `importer` |
| Storage | `run-store`, `manifest`, `outcome-store` |
| Evaluation | `evaluation`, `outcome-algebra`, `evaluator-protocol`, `registry` |
| Analysis | `analysis`, `stats` |
| Surfaces | `library`, `cli`, `mcp` |
| Wire schemas | `schemas` |
| Documents | `design`, `reports`, `roadmap`, `readme`, `contributing` |
| Repository | `repo`, `release`, `deps` |

## Versioning

Two release lines, each with its own tags:

| Line | Tags | Covers |
|---|---|---|
| Core | `v<major>.<minor>.<patch>` | Everything except the wire schemas |
| Wire schemas | `schemas@<major>.<minor>.<patch>` | The versioned schema files (outcome record, tool-catalog snapshot) |

Plug-ins pin a schema version, so a core release never forces a schema bump. A commit that changes a schema file uses scope `schemas` and touches nothing else.

### Core line

| Commits since the last release | Bump |
|---|---|
| `!` or a `BREAKING CHANGE:` footer | major (minor while the version is `0.y.z`) |
| `feat` | minor |
| `fix`, `perf`, `refactor`, `style`, `revert`, `docs(readme)` | patch |
| anything else | no release |

The highest bump across the batch wins. The core stays at `0.y.z` until the evaluator contract is frozen; the freeze is the `1.0.0` release.

### Schema line

| Change to a schema | Bump |
|---|---|
| A record valid before becomes invalid (field removed or renamed, optional field made required, enum value removed, type narrowed) | major |
| A new optional field or enum value; every record valid before stays valid | minor |
| Descriptions, examples or `$comment` only | patch |

## Branches and integration

- `main` is always releasable. Work happens on a topic branch named `<type>/<topic>`, for example `feat/outcome-record`.
- Integrate by rebasing the branch onto `main`, then merging with `--no-ff`. The merge commit is `merge(<scope>): <what landed and why>`, so the merge itself records the reason.
- Remove the branch and its worktree once it has landed.

## Enforcement

A local `commit-msg` hook gives immediate feedback, and CI checks every pull request against the same rules. Both are added together with the release tooling, once the implementation language is fixed.
