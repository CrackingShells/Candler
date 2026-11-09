# candler-evaluator

The Python helper for Candler evaluator plug-ins. It speaks the evaluator protocol on a
private pipe, isolates each item, reports the identity of its instruments and checks every
message against models generated from the published wire schemas. M2a fills it in; see
`__roadmap__/m2a-evaluation-parity/`.

## Wire models

`src/candler_evaluator/_models/` is generated from `crates/candler-schemas/schema/` and never
edited by hand. After a schema changes, regenerate it from this directory:

```sh
uv run --frozen python scripts/gen_models.py
```

CI regenerates the models and fails when they differ from the committed ones.

## Version and releases

The version lives only in `pyproject.toml`; `candler_evaluator.__version__` reads it from the
installed package metadata. A release is tagged `candler-evaluator@X.Y.Z`, and the package is
never published to PyPI without Eliott's go. See `CONTRIBUTING.md` at the repository root.

## Checks

```sh
uv run --frozen pytest -q && uv run --frozen ruff check && uv run --frozen ruff format --check
```
