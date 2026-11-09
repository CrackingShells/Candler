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
| Evaluation | `evaluation`, `outcome-algebra`, `evaluator-protocol`, `registry`, `evaluator-helper` (the Python package in `python/candler-evaluator`) |
| Analysis | `analysis`, `stats` |
| Surfaces | `library`, `cli`, `mcp` |
| Wire schemas | `schemas` |
| Documents | `design`, `readme`, `contributing` |
| Repository | `repo`, `release`, `deps` |

## Versioning

Three release lines, each with its own tags:

| Line | Tags | Covers |
|---|---|---|
| Core | `v<major>.<minor>.<patch>` | Everything except the wire schemas and the evaluator helper |
| Wire schemas | `schemas@<major>.<minor>.<patch>` | Every schema file under `crates/candler-schemas/schema/`, the roots that [R01 §The `schemas@` line holds](__design__/architecture.md) lists |
| Evaluator helper | `candler-evaluator@<major>.<minor>.<patch>` | The Python package `python/candler-evaluator`; its version lives only in its `pyproject.toml`, and it is never published to PyPI without Eliott's go |

Plug-ins pin a schema version, so a core release never forces a schema bump. A commit that changes a schema file uses scope `schemas` and touches nothing else.

### Core line

| Commits since the last release | Bump |
|---|---|
| `!` or a `BREAKING CHANGE:` footer | major (minor while the version is `0.y.z`) |
| `feat` | minor |
| `fix`, `perf`, `refactor`, `style`, `revert`, `docs(readme)` | patch |
| anything else | no release |

The highest bump across the batch wins. The core stays at `0.y.z` until the evaluator contract is frozen; the freeze is the `1.0.0` release.

**While a version is `0.y.z`**, a breaking change and a `feat` both bump the minor (`release-plz.toml` sets `features_always_increment_minor`), and every other release-triggering commit bumps the patch. A breaking change is still marked with `!` or a `BREAKING CHANGE:` footer, with its migration path in the body: the version cannot tell it from a feature, so the marker is what puts it in the changelog as breaking. The same holds for the schema and evaluator helper lines.

### Schema line

| Change to a schema | Bump |
|---|---|
| A record valid before becomes invalid (field removed or renamed, optional field made required, enum value removed, type narrowed) | major |
| A new optional field or enum value; every record valid before stays valid | minor |
| Descriptions, examples or `$comment` only | patch |

`release-plz` reads the bump from the commit type, not from the schema diff, so the commit's type must carry the class of its change:

| Class | Commit |
|---|---|
| major (minor while `0.y.z`) | `feat(schemas)!` or `fix(schemas)!`, or either with a `BREAKING CHANGE:` footer; a footer on a type that releases nothing (`test`, `chore`, …) releases nothing |
| minor | `feat(schemas)` |
| patch | `fix(schemas)` |

A change that makes any record valid before invalid is never a plain `fix(schemas)`, even when it corrects a defect: it would ship as a patch and break every plug-in pinned to the line.

### Evaluator helper line

`release-plz` does not cover the Python helper. Its version is bumped by hand in `python/candler-evaluator/pyproject.toml` with the same rules as the core line, read from the `evaluator-helper` commits since its last tag, and the tag `candler-evaluator@<version>` is cut only on Eliott's go.

## Branches and integration

- `main` is always releasable and changes only through pull requests. Work happens on a topic branch named `<type>/<topic>`, for example `feat/outcome-record`.
- Push the branch and open a pull request against `main`. Its title becomes the merge commit's subject, `merge(<scope>): <what lands and why>`, so the merge itself records the reason. GitHub appends ` (#<number>)` to it, and the title with that suffix fits in 72 characters. Its body becomes the merge commit's body; an agent-authored one ends with the `Co-Authored-By:` trailer.
- CI runs the gates and the commit check on the pull request. Before merging, rebase the branch onto `main` (`git rebase origin/main`, then `git push --force-with-lease`).
- Merge with a merge commit; the repository refuses squash and rebase merges. The merge is a push to `main`, which runs CI again and then the release (see [Enforcement](#enforcement)).
- GitHub deletes the merged branch. Remove the local branch and its worktree, then pull `main`, which may also carry the release job's `chore(release)` commit.

## Tools

| Tool | Used for | Version checked |
|---|---|---|
| `cog` (cocogitto) | Checking commit messages, locally and in CI | 7.0.0 |
| `release-plz` | Versions, changelogs and releases from the commits | 0.4.3, as the release job pins it |
| `cargo-nextest` | Running the tests | 0.9.150 |
| `uv` | The evaluator helper's tests and lints, and the evaluation guards that run it | 0.9.18 |
| Docker | The container guards (`guard_container_e2e`, ignored by default) | Docker Desktop or a Linux engine |
| `python3` and `openssl` | The gateway's TLS fake in the container guards (`tests/fixtures/gateway/`) | Python 3.9 or later |

```
brew install cocogitto cargo-nextest uv
cargo install --locked release-plz@0.4.3   # Homebrew has no release-plz formula
cog install-hook --all               # once per clone, from the main checkout
```

Git does not copy hooks on clone, so `cog install-hook --all` is the per-clone step, and it is run again whenever the hook in `cog.toml` changes, since the installed hook is a copy. It writes into the repository's `.git/hooks/`, which every worktree shares, and fails when run inside a linked worktree.

## Enforcement

| Check | Runs | Rejects |
|---|---|---|
| `commit-msg` hook | On every local commit, once `cog install-hook --all` has been run | A subject without a `type(scope):` prefix, a subject over 72 characters, then, through `cog verify`, a type or scope outside the tables above |
| `cog check <base>..<head>` | In CI (`.github/workflows/ci.yml`), on the range each pull request or push to `main` adds (run it by hand as `cog check main..HEAD`) | The same, for every commit in the range |
| Pull request title | In CI, on every pull request, as the subject of the merge commit it becomes | A title that is not `merge(<scope>): …` with a scope from the tables above, or that does not fit in 72 characters with GitHub's ` (#<number>)` |
| `release-plz` | In CI (`.github/workflows/ci.yml`), on a push to `main` once the gates and the commit check pass: `release-plz update` commits the new versions and changelogs as `chore(release): … [skip ci]`, then `release-plz release` tags each new version and creates its GitHub release | Nothing; a push with no release-triggering commit releases nothing, and nothing goes to crates.io |

The types and scopes live in `cog.toml` and equal the tables in this file; change both together. `cog` itself does not require a scope, which is why the hook checks the prefix first. A checkout without `cog.toml` passes the hook, so a branch cut before the file landed can still commit.

Commits made before `cog.toml` landed on `main` are outside every checked range: a pull request's range starts at `main`, so CI never sees them.
