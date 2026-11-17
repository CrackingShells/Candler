# Agent token costs

Where the Candler campaign's tokens went, what every subagent starts with, and how to configure the agent system to spend less.

## Documents

### Round 00 (2026-11-12)
- [00-findings_campaign_costs_v0.md](00-findings_campaign_costs_v0.md): cost by token category, session, role, milestone and day; the window since PR #17; cache rewrites. Superseded by round 02.
- [00-findings_subagent_prefix_v0.md](00-findings_subagent_prefix_v0.md) **(latest)**: a measurement of the 61k-token fixed prompt of a general-purpose subagent, and what in it an implementer needs.
- [00-architecture_agent_system_v0.md](00-architecture_agent_system_v0.md) **(latest)**: the proposed configuration (agent definitions with tool allowlists, a project-level plugin trim, a fix-round cap, leaf sizing), with alternatives and risks.

### Round 01 (2026-11-12)
- [01-findings_context_window_v0.md](01-findings_context_window_v0.md): what the change from a 250k to a 500k window did to compactions, cost, latency and behaviour, with a replay of the same transcripts at 250k, 500k and 1M. Superseded by round 02.

### Round 02 (2026-11-13): M4 complete
- [02-findings_campaign_costs_v1.md](02-findings_campaign_costs_v1.md) **(latest)**: the campaign at 1,678 USD with M4 complete (332 USD, the most of any milestone); M4 by leaf and phase; the second overnight PR cycle; cost against code landed; a corrected role classifier.
- [02-findings_context_window_v1.md](02-findings_context_window_v1.md) **(latest)**: the window analysis re-run with M4. The replay now gives -24% at 250k for 500k-era subagents, and M4's long implementers carry the cost.

### Round 03 (2026-11-17): M5, M5b and the coordinator
- [03-findings_campaign_costs_v2.md](03-findings_campaign_costs_v2.md) **(latest)**: 275 USD from 2026-11-13 01:22 to 2026-11-17 04:14 UTC, by group (M5, M5b, coordinator, interlude audit, driver work) against budget; main threads against subagents; one corrected rate set that lowers rounds 00-02 from 1,678 to 1,380 USD (Sonnet 5.5 cache read is 0.10, not 0.20).
- Script: [scripts/round03_measure.py](scripts/round03_measure.py), output in [scripts/round03_output.txt](scripts/round03_output.txt). It reads the transcripts directly and reproduces round 02's 1,678 with round 02's rates.

## Related
- M4's knowledge transfer on coordinating implementers and verifiers: `__reports__/m4-session-runner/coordination_knowledge_transfer_v0.md` (169231e, on main). It adds a cost driver this data doesn't isolate: the full gates re-run three times per fix round, by implementer, verifier and coordinator.

## Status

- Lost data: M4's analysis scripts (`analyse_v1.py`, `tbl.py`, `report_v1.txt`, `calls.csv`) were lost from a cleared temp dir, so round 02's M4 numbers (per-leaf, per-phase, role tables) cannot be recomputed. The totals can: `round03_measure.py` re-prices the raw transcripts.
- `scripts/spend.py` used Opus 5.5 cache read 0.40 and Sonnet 0.20, twice the list prices; it now uses 0.20 and 0.10. Figures quoted from it before 2026-11-17 13:30 JST (the M5b reminder's first ≈ $140) are overstated.

- Data covers 2026-10-28 to 2026-11-13 01:22 UTC (round 02): 25,894 API calls, 12 sessions, 279 subagents. Rounds 00 and 01 stop at 2026-11-12 00:30 UTC. Costs are API-equivalent at list price.
- Round 02 corrects the role classifier, which moved 15 USD between roles. The architecture and prefix reports (round 00) are unchanged: M4 adds no new evidence on the prefix, and its data supports the leaf-sizing and fix-round proposals.
- Decision pending (Eliott): whether to pilot the agent definitions and the plugin trim on M4's next leaves.
- Rounds 00-03 are committed in this repository by the PR that lands them.
