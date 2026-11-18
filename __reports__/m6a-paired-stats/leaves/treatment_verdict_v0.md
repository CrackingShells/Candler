# treatment_verdict (T5): leaf report

## Commits
See `git log c39c26fc..m6a/treatment_verdict`: the guard rows, then the rule in `compare.rs`.

## Gates
- Targeted test (`guard_comparability::`, `guard_analysis_report::a2_`): PASS, 10 run, 10 passed.
- Existing `verdicts.json` rows byte-identical: PASS (3 additions, 0 deletions).
- clippy `-p candler-core --all-targets -D warnings`: PASS. fmt: PASS.
- Ownership check: prints nothing.

## Claims checked
- Env lock, image and build inputs sit at the leaves `environments`, `isolation.image` and `inputs` of `compare`; a declared `treatment` factor covers the `treatment` leaf. Confirmed in `flatten` and `declared_covers`.
- The `guard_comparability` table iterates every row, so no `t5_` test was added.

## Bites
Each run on `stratum_verdict_table`, restored from a copy of the file.
- Explain on any declared factor (the `treatment` root check dropped): red on `t5_other_factor_does_not_explain_lock_image_inputs` (declared `model.id`, treatment equal, lock differs, expected refused at `environments`, `inputs`, `isolation.image`).
- Explain when the treatment is equal (declared `treatment` alone explains): red on `t5_equal_treatment_does_not_explain_lock_image_inputs`.
- Red before the rule: only `t5_treatment_explains_lock_image_inputs` failed (got Refused, expected comparable); the other two rows pass without the rule and hold the refusal.

## Findings
- The leaf's two commit subjects are 73 characters, over the 72 `cog` limit. They were shortened: `test(analysis): guard treatment as an explanation of lock differences` and `feat(analysis): let a treatment factor explain lock and image changes`.
- A declared factor below `treatment` (for example `treatment.tag`) does not cover the `treatment` leaf, so such a stratum is still refused at `treatment`; unchanged.
- `snapshots` differences are not explained by the treatment, as the leaf lists only lock, image and build inputs.
