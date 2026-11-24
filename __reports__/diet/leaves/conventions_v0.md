# Conventions leaf, v0

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| AGENTS.md section replaced, word for word, nothing else changed but the link | PASS | `git show f57ba4fd --stat` touches AGENTS.md only; heading `What sane work means in Candler`; old heading gone; paragraph re-wrapped to 100 columns, no word changed |
| Template holds the conventions of D3 | PASS | `__roadmap__/leaf_template.md`, six conventions, convention 6 reads "estimate, its cap as a stop action, the target dir the campaign README names, and their cleanup" |
| AGENTS.md §Where things are written down links the template | PASS | last bullet of that section |
| Fresh reader finds no leaf breaking the template | PASS after fixes | see below |

The gates are file-content checks; none can be shown failing on a code base, because this leaf has no code.

## Fresh Sonnet reader (given the template and the leaf list)

Fixed (prose fields only):
- Convention 6, cleanup: the Estimate of collect_oracle, kill_off_tty, one_reader, sealed_run_producer and catalog_snapshot now says builds use the target dir the campaign README names, removed at the leaf's merge.
- Convention 6, stop action: seam_review_one, seam_review_two and dogfooding_pass gained "At the cap, stop, commit what is green, and list what is left."
- catalog_snapshot's gate still named the `candler-diet` target dir (nested below the level the first sweep covered); it now names the campaign README's.

Not changed:
- Convention 4, seam_review_two hands the reviewer plan v2, seam review 1's report and the diff since it, not the milestone's whole diff. This changes review scope, a design choice for the lead, not a prose field.
- Convention 2, gates of one_reader (3 to 7), sealed_run_producer (2 to 4), catalog_snapshot (2 to 4), kill_off_tty (2), collect_oracle (2), both seam reviews and conventions.md are structural, process or invariant checks. They are guards beside the observation gates of the same leaf, not substitutes for them, so I did not rewrite them; the lead may judge otherwise.
- Convention 5 for seam_review_one, seam_review_two, conventions and dogfooding_pass: the campaign README's gate "Every merge into `feat/integrity` waited for a green `container-e2e`" covers them.
- conventions.md has no target dir sentence: it runs no builds.

## Left

Nothing in scope. Cog accepted scope `contributing` for every commit.
