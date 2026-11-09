# Changelog

Releases of this package are tagged `candler-evaluator@X.Y.Z` and are never published to
PyPI without Eliott's go. The version is bumped by hand in `pyproject.toml` from the
`evaluator-helper` commits since the last tag; see `CONTRIBUTING.md`.

## Unreleased

- The wire models are generated from the published schemas by `scripts/gen_models.py`; the
  loop parses and builds every message through them, and the shared wire vectors decide
  what they accept.
- `candler_evaluator.__version__` reads the installed package version.
- `pydantic` is the package's one runtime dependency.
