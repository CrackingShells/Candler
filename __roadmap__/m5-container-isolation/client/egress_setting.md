# Egress setting

**Goal**: `isolation.egress` resolves to a canonical list of `host:port` entries, or is refused naming the entry. An absent value leaves every cell as it was (D3).
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [nl] OWNED: `crates/candler-core/src/resolve/steps/egress.rs` (new) and its registration line, the step's refusal route in `crates/candler-core/src/resolve/{preflight,refusal}.rs`, the per-cell settle in `crates/candler-core/src/runner/plan.rs` and its row in `runner/refusal.rs` (amended at L1: a derived value ranks below the experiment file and steps see no factor levels, so the `runner.mode` precedent applies), a new guard file; the two codes appended to `docs/contract/codes.toml` with their `Refusal` impl
**Success Gates**:
- ✅ [cmd] The parse table passes: canonical form (lowercase, sorted, deduplicated); IPv4, IPv6 and bracketed literals, wildcards, schemes, paths, port 0, port above 65535 and a missing port refused with `isolation.egress_invalid`; any entry together with `isolation.network = bridge` refused with `isolation.egress_with_bridge`: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_setting::/)'`
- ✅ [cmd] A cell with egress unset keeps its 18142ec digest, and a set value enters the cell: DIGEST, and `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_setting::egress_enters_the_cell/)'`
- ✅ [cmd] The new codes have their pages: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_refusal_codes::/)'` and `cargo nextest run -p candler --no-tests=fail -E 'test(/^contract_pages::/)'`
- ✅ [cmd] STD (`-p candler-core`), LINUX and SUBJ pass
- ✅ [nl] The leaf report records the bite of each row class
**References**: [R06 D3](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m5-approved-plan.md); [R01 §Agent environment and secrets](../../../__design__/architecture.md) — the configuration table; RFC 1123 §2.1 (the hostname rule)

## Step 1: Parse and refuse the egress allowlist
**Goal**: A pure `settle(egress, network)` validates and canonicalises `isolation.egress`; a resolve step calls it for an early refusal, and the plan calls it per cell, writing the canonical list (an empty list is absent) before the cell is hashed.
**Implementation Logic**:
1. Guards first, seen red, in a table whose oracle is RFC 1123's label rule, transcribed by hand:
   - letters, digits and hyphens;
   - no leading or trailing hyphen;
   - labels of 1 to 63 characters;
   - names of at most 253 characters;
   - at least one dot.

   Rows: one per accepted class (a mixed-case name is lowercased; duplicates go; the order is sorted) and one per refused class.
2. Bites: an IP literal accepted, which a parser that only checks characters would allow; a missing port defaulted to 443.
3. The step runs after the plug-in step registers (`plugin_environment`, L2). Leave a slot for it and the order comment in `resolve/steps/mod.rs`. The coordinator checks the order at L2.
4. `egress_enters_the_cell`: the same experiment with and without the setting. Absent gives the 18142ec digest; present gives a different digest whose cell shows the canonical list. Bite: the path classed as a setting.
**Deliverables**: `crates/candler-core/src/resolve/steps/egress.rs`: `EgressStep`, `Egress {entries: Vec<HostPort>}`, `HostPort`; codes `isolation.egress_invalid`, `isolation.egress_with_bridge` in `docs/contract/codes.toml`; `crates/candler-core/tests/it/guard_egress_setting.rs`
**Consistency Checks**: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^guard_egress_setting::/)'` (expected: PASS)
**Commit**: `feat(isolation): resolve an egress allowlist of hostnames and ports`
