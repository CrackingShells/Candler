# Contributing to Candler

Humans and agents both commit here. The rules below are what the `commit-msg` hook, `cog` and `release-plz` enforce (see [Enforcement](#enforcement)).

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/), with a mandatory scope and three types of our own (`merge`, `rdm`, `reports`):

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
| `docs` | Documentation outside `__roadmap__/` and `__reports__/` |
| `rdm` | Roadmaps in `__roadmap__/` |
| `reports` | Reports in `__reports__/` |
| `build` | Packaging, dependencies, environments |
| `ci` | CI workflows |
| `chore` | Repository upkeep that fits nothing above |
| `revert` | Reverting an earlier commit (body names it) |
| `merge` | The merge commit that integrates a branch |

### Scopes

A scope names the topic of the commit, never the action or a layer (`core`, `misc`, `update` are not scopes). Related commits share one scope so the history reads as a story: a milestone's plan, merge and close-out share the scope of what it builds (`rdm(schemas)`, `merge(schemas)`, `reports(schemas)`). Use these, and add a new one only when no existing scope fits:

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
| Documents | `design`, `readme`, `contributing` |
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

## Tools

| Tool | Used for | Version checked |
|---|---|---|
| `cog` (cocogitto) | Checking commit messages, locally and in CI | 7.0.0 |
| `release-plz` | Versions, changelogs and releases from the commits | 0.4.2 |
| `cargo-nextest` | Running the tests | 0.9.150 |

```
brew install cocogitto cargo-nextest
cargo install --locked release-plz   # Homebrew has no release-plz formula
cog install-hook --all               # once per clone, from the main checkout
```

Git does not copy hooks on clone, so `cog install-hook --all` is the per-clone step. It writes into the repository's `.git/hooks/`, which every worktree shares, and fails when run inside a linked worktree.

## Enforcement

| Check | Runs | Rejects |
|---|---|---|
| `commit-msg` hook | On every local commit, once `cog install-hook --all` has been run | A subject without a `type(scope):` prefix, then, through `cog verify`, a type or scope outside the tables above |
| `cog check <base>..<head>` | In CI (`.github/workflows/ci.yml`), on the range each push or pull request adds (run it by hand as `cog check main..HEAD`) | The same, for every commit in the range |
| `release-plz` | In CI (`.github/workflows/release-plz.yml`), on every push to `main`: `release` tags each package version that has no tag yet, then `release-pr` opens or updates the release PR | Nothing; it publishes nothing, and a release lands only when its PR is merged |

The types and scopes live in `cog.toml` and equal the tables in this file; change both together. `cog` itself does not require a scope, which is why the hook checks the prefix first. A checkout without `cog.toml` passes the hook, so a branch cut before the file landed can still commit.

Commits made before `cog.toml` landed on `main` are outside every checked range: a pull request's range starts at `main`, so CI never sees them.
