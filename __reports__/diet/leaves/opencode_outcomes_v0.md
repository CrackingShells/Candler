# OpenCode on RiVault: level 1 attempts

Evidence for P7 and the knowledge transfer. One entry per attempt, written at the time by the lead.

**Model of record for all four attempts:** alias `RiVault/RiVault/Agentic-Medium`, backed by DeepSeek-V4-Flash,
reasoning off (the proxy's default `reasoning_effort` is none, and the OpenCode config set no level). Earlier
notes that named Kimi-K3 were wrong. The `opencode run` logs were plain text, so no token counts exist for these
attempts: the RiVault side is unmeasured. Claude-side costs per leaf are in `seam_review_one_v0.md` §Checkpoint
once written; until then, in the handoff note.

**Outcome:** 4 of 4 attempts reported every gate as passing; 0 of 4 held up under review; 2 of 4 needed a Claude
rescue (P6's b40, P2's oracle).

## Attempts
- P4·B (RiVault Agentic-Medium, OpenCode): wall 953 s, rc 0. Two commits, ee533367 feat(schemas)! (candler-schemas only, 6 files) + 557978f8 report. Self-reported all gates PASS first try; one flaky guard_protocol_client timeout noted, passed on re-run base and tip. Chose own mcp-server.schema.json (plan §D5 fallback). Dispatched before the scratch-target-dir fix; reviewer checks fresh-compile evidence. Reviewer: type alone OK, gate real on fresh compile (reviewer re-ran it: base red, tip 209/209, loosened schema red); caught that the report's base evidence was an ad-hoc check with no Compiling line, and that command-or-url is not enforced by the schema (left to A1); report line 38 misstates command as required.
- P2 (RiVault Agentic-Medium, OpenCode) attempt 1: wall 525 s, rc 0, ended its turn mid-work with 10 clippy errors and nothing committed (4 files edited). Continued the same session (ses_df45a8f61ffeLhKka8hYKhtQCZ) with a 'finish and commit' prompt.
- P6 (RiVault Agentic-Medium, OpenCode): wall 906 s, rc 0, one attempt. One commit 974ed5ed (kill.rs +5, new guard, b41 dry-run assertion replaced, report). Self-reported all gates PASS; base failure taken on the worktree's own base HEAD, not a scratch worktree. Dropped b41's 'would stop' scoping assertion. PR #61. Reviewer: base→tip real on fresh compile; caught that the report claimed b40 ran green locally (no Docker here, it is #[ignore]) while b40 still asserted the dry-run listing, so container-e2e would go red; scoping guard moved to the -y run line count by the lead.
- P2 attempt 2 (continued session): wall 1474 s, rc 0. Commits d39e367f test(library) + e3e230c0 report; self-reported all gates PASS; leaf scope harness rejected by hook, used library. Reviewer: B1 bite real on fresh compile (46 Compiling lines), but gate 2 FAILS: oracle is a hand-typed list (3 copies), no scripted agent, and a hand-written glob matcher instead of the real collector (D5), checks kept not exactly. Gate counted PASS by the run. Sonnet fallback dispatched.

## Level 2 attempts (under the trial ruling)
- mcp_schema, the MCP server schema refusing what `mcp::read` refuses (alias `RiVault/Reasoning-Medium`, backing
  `deepseek-ai/DeepSeek-V4-Flash`, proxy effort high, OpenCode `--format json`): wall 1358 s, rc 0, 122 steps;
  reasoning 101,898, output 26,263, input 168,129, cache read 10,372,096 tokens. One attempt, no resume. Three
  commits (schemas, docs page, report). It reported all three gates as passing, with the base failure taken in a scratch
  worktree.
  - The Sonnet review ($0.29, 5 tool calls) did not see the run's summary. Its verdict was merge.
    - It reproduced base red and tip green on a fresh compile. `mcp::read` is untouched, no serde attribute
      changed, and no layer was added.
    - It flagged that the docs commit had also reworded three unrelated bullets, which nobody asked for. That was
      wrong: the page's Notes are capped at 120 words, and the shorter wording made room for the new sentence.
      The lead undid the rewording, `contract_pages` failed in CI at 130 words, and the lead restored the run's
      wording. The run had the cap right, and the review and the lead did not.
    - It found that the table had no rows for a `type` marker that disagrees with the fields. An outside validator
      showed the schema refusing those shapes, but no row guarded it.
    - Its claim that `{command, url}` without a `type` had no row was wrong: that row exists.
  - The lead added the marker rows and a doc sentence, and spent a CI round on the rewording (about $0.8 of lead turns in all, launch included).
  - Claude-side cost was about $1.1, against $3–5 for a Sonnet implementer, so the saving holds for this piece.
  - The claim and the review agree on every gate. This is the first attempt to hold up under review.
  - Unchanged friction: the run cannot push, so the lead opens the PR.

## Driving the CLI from Claude Code: friction seen
- An attempt can end its turn mid-work with rc 0 (P2 attempt 1): rc says nothing about completion; check commits and the build.
- Self-reported gate verdicts were not evidence: an `#[ignore]` test reported as "ran green" (P6), a hand-typed oracle reported as D1 PASS (P2), base evidence without a fresh compile (P4·B). A separate review on a fresh compile is what caught each.
- Plain-text logs carry no tokens; `--format json` is needed for per-step reasoning and output tokens.
- The leaf's commit scope `harness` is rejected by the commit-msg hook; every attempt had to improvise a scope.
- Push and `gh` had to be blocked from outside (`GIT_CONFIG` pushurl override, `GH_TOKEN=blocked`).
- The lead's own CI waiter could not end on "no run for this SHA" (a conflicting PR starts no pull_request run), and its minimum check count was set above what a commit carries, so it could only time out. Rewritten in `integrity-tools/wait_checks.sh`.

## Trial ruling for the rest of the milestone
Non-critical pieces whose gate a test can judge may go to RiVault: `RiVault/Reasoning-Medium` (DeepSeek-V4-Flash,
thinking on), then one attempt on `RiVault/Reasoning-Large` (record the `model` field the proxy returns), then
Sonnet. Run with `--format json`; an attempt whose reasoning tokens are zero says nothing about the question. One
script starts, waits and summarises; resume a stopped session once at most. A Sonnet review is mandatory, briefed on
three questions (base-to-tip evidence on a fresh compile, state from the real producer, one reader per input) and
never with the run's own summary. Record per attempt: alias and backing model, reasoning and output tokens, wall,
claim against review, USD of the review and of the lead's turns on it, and friction points. The saving counts only
if run plus review plus lead turns cost less than a Sonnet implementer (about $3–5).
