"""Generate the live context vectors of the three real mini-corpus runs, with a second engine.

    uv run --with rfc8785 python gen_live_context.py <mini-corpus>          # write live_context_runs.toml
    uv run --with rfc8785 python gen_live_context.py --check <mini-corpus>  # regenerate and compare

A live context differs from an imported one (`context_digest_runs.toml`): it carries no snapshot
and no input (Tiers A-C), and its artifacts are chosen by the file rule of R03 (§File rule), which
this script writes from R03's prose and never from Candler's code:

  The artifacts of a context are the run's stored `outputs/` files matched by an artifact glob,
  minus any path with a component starting with `.` unless the glob names that component
  literally. Paths are relative to `outputs/` with `/`. An empty `outputs/` gives no artifacts.

The suite needs are the artifact glob `outputs/**` and the metadata path
`plan.prompt_domain_name`. `outputs/**` names no component literally, so every path with a dot
component is dropped. The metadata value is the JSON leaf of the run's cell's `experiment.json`
at that dotted path. The projection is `{artifacts: [{path, sha256}], snapshots: [], inputs: [],
metadata}` with the artifacts sorted by `path` by code point; its digest is `sha256` of the RFC
8785 form (python `rfc8785`). No Candler code runs and no Candler output is read.

The twin: `run-6` of the first cell with the first byte of `outputs/domain.py` XORed with 0x01.
Its digest must differ from the run's own.

Served parts (the `[[served]]` rows). A context whose kinds declare `trace`, `raw` or `manifest`
adds, to the projection above, one key per served part and only for a served part:

  trace    the sha256 of the stored `trace.jsonl`, as `sha256:<hex>`
  raw      `[{stream, sha256}]` sorted by `stream`, one per stored `raw/<stream>.jsonl.gz`
  manifest the sha256 of the stored `manifest.json`

The bytes of those files are the store's, written by the import, so the generator reads them from
a store folder given with `--store` (the hash of a file is the only thing it takes from there) and
records each hash in its row. Without `--store` it reuses the hashes its rows already record, so
`--check` needs no store; the Rust guard compares each recorded hash with the hash of the file the
store holds before it compares digests. The recorded hashes are those of the bytes a reader
writes, so the `[[served]]` rows are regenerated with `--store` whenever a reader's output changes.
"""

import argparse
import glob
import hashlib
import importlib.metadata
import json
import os
import sys
import tomllib

import rfc8785

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = "live_context_runs.toml"

V3 = "20260806-035940_v0.2.3_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-v0.2.3"
V6 = "20260806-035940_v0.2.6_claude-haiku-4-5_model-claude-haiku-4-5_skill_version-htn-creator-0.2.6"
REAL_RUNS = [(V3, 6), (V3, 17), (V6, 12)]
CONFIG = "with_skill"
ARTIFACT_GLOBS = ["outputs/**"]
METADATA_PATHS = ["plan.prompt_domain_name"]
TWIN_RUN = (V3, 6)
TWIN_FILE = "domain.py"
SERVED = [
    (V3, 6, ["trace", "raw", "manifest"]),
    (V6, 12, ["trace"]),
    (V3, 17, ["raw", "manifest"]),
]


def digest_of(value):
    """`sha256:` plus the hex of the RFC 8785 form of `value`."""
    return "sha256:" + hashlib.sha256(rfc8785.dumps(value)).hexdigest()


def jtext(value):
    """JSON text that keeps key order; ASCII only."""
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), allow_nan=False)


def toml_str(text):
    return json.dumps(text, ensure_ascii=False)


def file_rule_keeps(rel):
    """R03's file rule for the glob `outputs/**`, which matches every file below `outputs/`."""
    assert ARTIFACT_GLOBS == ["outputs/**"]
    return not any(part.startswith(".") for part in rel.split("/"))


def run_artifacts(corpus, cell, n, edit=None):
    """The kept `outputs/` files of a run: relative path and raw-byte sha256, at any depth."""
    root = os.path.join(corpus, cell, "_run_cache", CONFIG, f"run-{n}", "outputs")
    assert os.path.isdir(root), root
    found = []
    for dirpath, _dirs, names in os.walk(root):
        for name in names:
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, root).replace(os.sep, "/")
            if not file_rule_keeps(rel):
                continue
            with open(full, "rb") as f:
                data = f.read()
            if edit is not None and rel == edit:
                data = bytes([data[0] ^ 0x01]) + data[1:]
            found.append(
                {"path": rel, "sha256": "sha256:" + hashlib.sha256(data).hexdigest()}
            )
    found.sort(key=lambda a: a["path"].encode("utf-8"))
    return found


def run_metadata(corpus, cell):
    """One key per metadata need, the value the JSON leaf of the cell's `experiment.json`."""
    with open(os.path.join(corpus, cell, "experiment.json"), encoding="utf-8") as f:
        doc = json.load(f)
    out = {}
    for dotted in METADATA_PATHS:
        node = doc
        for part in dotted.split("."):
            node = node[part]
        assert not isinstance(node, (dict, list)) and node is not None, dotted
        out[dotted] = node
    return out


def projection(artifacts, metadata):
    return {"artifacts": artifacts, "snapshots": [], "inputs": [], "metadata": metadata}


def header():
    version = importlib.metadata.version("rfc8785")
    return "\n".join(
        [
            "# live_context_runs.toml: live context digests of the three real mini-corpus runs (R03 §File rule).",
            "# Generated by tests/oracle/vectors/gen_live_context.py (python, `uv run --with rfc8785`),",
            f"# rfc8785 {version}. Do not edit: run the generator and commit the result.",
            "# JSON values are carried as JSON text; every digest is sha256 of the RFC 8785 form.",
            "# Needs: artifact glob `outputs/**`, metadata path `plan.prompt_domain_name`; no trace, snapshot or input.",
            "# File rule (R03 §File rule, written from its prose): the artifacts are the run's stored `outputs/`",
            "# files matched by the glob, minus any path with a component starting with `.` (the glob names none",
            "# literally); paths are relative to `outputs/` with `/` separators; an empty `outputs/` gives `[]`.",
            "# A live context has no snapshots and no inputs, so these digests differ from `context_digest_runs.toml`.",
            "# `run` is `<config>/run-<N>`, the run folder relative to `_run_cache/`; `metadata` keys are the dotted",
            "# paths themselves, the values the JSON leaves of the cell's `experiment.json`.",
            "# Spellings: a digest inside a JSON value is `sha256:<64 hex>`; artifacts sort by path by code point.",
            "# The twin is the run named under `[twin]` with the first byte of its `domain.py` XORed with 0x01.",
            "",
            "",
        ]
    )


def sha_of_file(path):
    with open(path, "rb") as f:
        return "sha256:" + hashlib.sha256(f.read()).hexdigest()


def stored_hashes(store, n):
    """The sha256 of the trace, manifest and each raw stream the store holds for replicate `n`."""
    found = glob.glob(os.path.join(store, "runs", f"*-r{n}-a1"))
    assert len(found) == 1, (n, found)
    run = found[0]
    raw = sorted(
        (name[: -len(".jsonl.gz")], sha_of_file(os.path.join(run, "raw", name)))
        for name in os.listdir(os.path.join(run, "raw"))
        if name.endswith(".jsonl.gz")
    )
    return {
        "trace": sha_of_file(os.path.join(run, "trace.jsonl")),
        "manifest": sha_of_file(os.path.join(run, "manifest.json")),
        "raw": raw,
    }


def recorded_hashes(path):
    """The hashes the `[[served]]` rows of the file on disk record, by replicate."""
    with open(path, "rb") as f:
        rows = tomllib.load(f).get("served", [])
    return {
        replicate_of(row["run"]): {
            "trace": row.get("trace_sha256"),
            "manifest": row.get("manifest_sha256"),
            "raw": [(r["stream"], r["sha256"]) for r in row.get("raw", [])],
        }
        for row in rows
    }


def replicate_of(run):
    return int(run.rsplit("run-", 1)[1])


def served_projection(base, parts, held):
    proj = dict(base)
    if "trace" in parts:
        proj["trace"] = held["trace"]
    if "raw" in parts:
        proj["raw"] = [{"stream": s, "sha256": h} for s, h in sorted(held["raw"])]
    if "manifest" in parts:
        proj["manifest"] = held["manifest"]
    return proj


def served_rows(corpus, hashes):
    out = [
        "# Served parts: the same runs and needs as above, with the kinds declaring the parts named in",
        "# `needs`. `trace_sha256`, `manifest_sha256` and `raw` record the hash of each stored file",
        "# (see gen_live_context.py); a part a row does not serve has none of them and no key in its projection.",
        "",
    ]
    base_digests = {}
    for cell, n, parts in SERVED:
        held = hashes[n]
        base = projection(run_artifacts(corpus, cell, n), run_metadata(corpus, cell))
        proj = served_projection(base, parts, held)
        digest = digest_of(proj)
        assert digest != digest_of(base)
        assert digest not in base_digests.values()
        base_digests[n] = digest
        out += [
            "[[served]]",
            f"cell = {toml_str(cell)}",
            f"run = {toml_str(f'{CONFIG}/run-{n}')}",
            f"needs = [{', '.join(toml_str(p) for p in parts)}]",
        ]
        if "trace" in parts:
            out.append(f"trace_sha256 = {toml_str(held['trace'])}")
        if "manifest" in parts:
            out.append(f"manifest_sha256 = {toml_str(held['manifest'])}")
        if "raw" in parts:
            items = ", ".join(
                f"{{ stream = {toml_str(s)}, sha256 = {toml_str(h)} }}"
                for s, h in sorted(held["raw"])
            )
            out.append(f"raw = [{items}]")
        out += [
            f"projection = {toml_str(jtext(proj))}",
            f"digest = {toml_str(digest)}",
            "",
        ]
    return "\n".join(out)


def generate(corpus, hashes):
    out = [header()]
    digests = {}
    for cell, n in REAL_RUNS:
        proj = projection(run_artifacts(corpus, cell, n), run_metadata(corpus, cell))
        digest = digest_of(proj)
        digests[(cell, n)] = digest
        out += [
            "[[run]]",
            f"cell = {toml_str(cell)}",
            f"run = {toml_str(f'{CONFIG}/run-{n}')}",
            f"projection = {toml_str(jtext(proj))}",
            f"digest = {toml_str(digest)}",
            "",
        ]
    cell, n = TWIN_RUN
    twin = projection(
        run_artifacts(corpus, cell, n, edit=TWIN_FILE), run_metadata(corpus, cell)
    )
    twin_digest = digest_of(twin)
    assert twin_digest != digests[TWIN_RUN]
    assert len(set(digests.values()) | {twin_digest}) == len(REAL_RUNS) + 1
    out += [
        "[twin]",
        f"cell = {toml_str(cell)}",
        f"run = {toml_str(f'{CONFIG}/run-{n}')}",
        f"file = {toml_str(TWIN_FILE)}",
        "xor_first_byte = 1",
        f"projection = {toml_str(jtext(twin))}",
        f"digest = {toml_str(twin_digest)}",
        "",
    ]
    return "\n".join(out) + "\n" + served_rows(corpus, hashes)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument(
        "--check", action="store_true", help="compare with the file on disk"
    )
    ap.add_argument("corpus", help="the mini-corpus folder")
    ap.add_argument(
        "--store",
        help="a store folder holding the import of the corpus: the source of the served files' hashes",
    )
    args = ap.parse_args()
    path = os.path.join(HERE, OUT)
    if args.store:
        hashes = {n: stored_hashes(args.store, n) for _, n, _ in SERVED}
    else:
        hashes = recorded_hashes(path)
    text = generate(args.corpus, hashes)
    if not args.check:
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        print(f"wrote {OUT}")
        return 0
    if not os.path.isfile(path):
        print(f"missing {OUT}", file=sys.stderr)
        return 1
    with open(path, encoding="utf-8", newline="") as f:
        if f.read() != text:
            print(f"differs {OUT}", file=sys.stderr)
            return 1
    print(f"vectors OK: {OUT}")
    return 0


if __name__ == "__main__":
    sys.dont_write_bytecode = True
    sys.exit(main())
