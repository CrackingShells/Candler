# wire-schemas reports

Reports of milestone `wire_schemas` (`__roadmap__/candler-v1/wire_schemas.md`, campaign `__roadmap__/wire-schemas/`).

## Round 00
- [00-knowledge_transfer_v0.md](00-knowledge_transfer_v0.md) — **latest**: close-out of the milestone

## Status
The milestone has landed on `main` (merge `5087a4b`) and was pushed on 2026-10-29. The reminders are reconciled, and the HTN capture-script branch waits for HTN Phase 4.5.

The first push answered the release-tooling question of round 00:
- `release-plz release` tagged `v0.1.0` and `schemas@0.1.0` on `c5e2803`, and `release-pr` found nothing to release after them;
- `ci.yml` passed after pinning `cocogitto/cocogitto-action@v4.2.0`: the floating `v4` tag still pointed at v4.0.0, which has no `install-only` input and installs cog 6.4.0;
- the release PR path stays unexercised until a release-triggering commit lands.
