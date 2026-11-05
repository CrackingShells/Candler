# Golden oracle tools

`extract_corpus.sh <artifact repo> <dest>` materialises the golden corpus
(`campaign/20260806-cont-train` at tag `candler-m2a-golden`) into an empty `<dest>` from a
`git archive`. The artifact repository is only read: the script runs `git archive` and writes
nothing under `<artifact repo>`.

`git` keeps no empty directory, so a run whose agent produced no output has no `outputs/` in the
archive. The script recreates an empty `outputs/` in every `_run_cache/with_skill/run-N/` that
lacks one and prints each path, so every run has the layout the importer expects.

The corpus is then at `<dest>/campaign/20260806-cont-train`; pass that path as
`CANDLER_GOLDEN_CORPUS`.

`select_mini.py` and `make_synthetic.py` build the vendored mini-corpus in
`../fixtures/mini-corpus`; `vectors/gen_vectors.py` writes the digest and identity vectors.

`vectors/context_digest_runs.toml` pins the context projection and digest of each real mini-corpus run (`legacy_mapping.md` Step 3), computed in Python from the raw files.
