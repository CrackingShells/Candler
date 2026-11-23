# Diet

The architecture diet between M6b (surface consolidation) and M7 (MCP server): fix S1, cut the suite to guards at the consumer interfaces, remove proven dead and duplicated code, and judge each README claim from real runs. Roadmap: [`__roadmap__/diet/`](../../__roadmap__/diet/README.md).

## Documents

### Round 00
- [`00-architecture_diet_plan_v0.md`](00-architecture_diet_plan_v0.md) (latest). The plan: where S1 lives and lands, the RiVault audit per README claim, the suite cut by family (about 836 to 525), what the code sheds, the build in pieces with cost, decisions K1 to K6, and compat changes.

- [`01-findings_e2e_audit_v0.md`](01-findings_e2e_audit_v0.md) (latest). The RiVault audit: one verdict per README claim from 18 live OpenCode runs and the stored bytes, with the broken claims for `audit_fixes`. Commands and records: [`audit/`](audit/).

## Status
Planned on 2026-11-24 from the M6b tip `1002cb82`. Waiting for approval of K1 to K6. Nothing is built.

## Round 01: coordinator rulings on the plan (2026-11-24, 01:55 JST)

Eliott delegated the diet overnight; the milestone coordinator ruled in his stead. He reviews these in the morning briefing.

| Decision | Ruling |
|:--|:--|
| K1 test policy (six consumer interfaces; every deletion cites its guard or "structure") | Approved |
| K2 S1 fix (seal stores tree blobs; `workspace/` removed after the seal) | Approved. `workspace/` goes only if neither Candler nor the HTN consumer (read-only check) reads it; otherwise it stays and the piece says so |
| K3 restore the founding purpose paragraph in `README.md` | **Deferred to Eliott.** The store-window README was an M6b item he accepted. The yardstick is still the paragraph as written at `254ff4a0` |
| K4 refusal codes: retire only rows a probe proves unemitted | Approved |
| K5 silent acceptance becomes refusals | Approved. The `resolve` failure record replaces `print_failures`' hand-built JSON; it adds no second shape |
| K6 one capped `audit_fixes` piece | Approved |
| Parallelism | At most three implementers building at once (each worktree has its own target dir; the machine is shared) |
