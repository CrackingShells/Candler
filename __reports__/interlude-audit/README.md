# Interlude audit

An audit of Candler against the three tests at the head of AGENTS.md (sustainable, scalable,
efficient), with the consolidation that followed, between M4 and M5. Budget: $50 and 6 h.

## Round 00
- [00-findings_audit_v0.md](00-findings_audit_v0.md): latest. The verdict, the findings by test,
  what this interlude fixes and what it leaves for later.
- [00-audit_slices_v0.md](00-audit_slices_v0.md): latest. The four auditors' findings, condensed.

## Round 01
- [01-results_v0.md](01-results_v0.md): latest. The PRs, the audit claims the implementers
  corrected, the spend, and the lessons for running the work.

## Status
#24 to #27 are merged (v0.8.1 shipped #26). The test-layout merge is #28: one integration binary per
crate, 101 nextest binaries down to 9. The driver-trait consolidation comes next. The legacy-import
sealing rule stays as it is.
