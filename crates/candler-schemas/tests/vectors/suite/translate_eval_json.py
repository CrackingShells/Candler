# /// script
# requires-python = ">=3.11"
# dependencies = ["tomli-w"]
# ///
"""Translate the golden-corpus `eval.json` into the suite file `cont_train.toml`.

Source: the `eval.json` of `htn-creator-eval@0.4.2`, the suite the golden regrade ran, read only
from the mcp-python-ingestion repository with
    git -C <repo> show htn-creator-eval@0.4.2:.claude/skills/htn-creator-workspace/infrastructure/evals/eval.json
The version comes from `PROVENANCE.json` `pinned` (`htn-creator-eval@0.4.2`, commit 28be2c15),
not from `harness/pyproject.toml`. The script refuses to translate unless the bytes hash to
`SOURCE_SHA256`, which is `PROVENANCE.json` `eval_infra."eval.json".sha256`; the hash and the
source are recorded in the `source` table of the output.

Run: uv run --with tomli-w translate_eval_json.py [REPO [REF [OUT]]]
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import tomli_w

REPO = "/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion"
REF = "htn-creator-eval@0.4.2"
PATH = ".claude/skills/htn-creator-workspace/infrastructure/evals/eval.json"
SOURCE_SHA256 = "ca7f12afee55152ed0e543a1d9b457c29fc5c02fd49e1ec6a417a9dcc1c97f9e"
OUT = Path(__file__).with_name("cont_train.toml")
EVALUATOR = {"name": "htn-creator-eval", "range": "^0.4"}
EXTRA = ("text", "question", "tier", "scale")


def entry(eval_, expectation):
    spec = dict(expectation["grading_spec"])
    kind = spec.pop("type")
    for key in EXTRA:
        assert key not in spec, f"{expectation['id']}: grading_spec already has {key!r}"
        spec[key] = expectation[key]
    return {
        "id": expectation["id"],
        "group": str(eval_["id"]),
        "role": expectation["role"],
        "evaluator": dict(EVALUATOR),
        "kind": kind,
        "params": spec,
        "depends_on": [],
        "applies_to": [
            {"path": "plan.config", "any_of": list(eval_["configurations"])}
        ],
    }


def main(argv):
    repo = argv[0] if len(argv) > 0 else REPO
    ref = argv[1] if len(argv) > 1 else REF
    out = Path(argv[2]) if len(argv) > 2 else OUT
    text = subprocess.run(
        ["git", "-C", repo, "show", f"{ref}:{PATH}"],
        check=True, capture_output=True,
    ).stdout
    actual = hashlib.sha256(text).hexdigest()
    if actual != SOURCE_SHA256:
        sys.exit(f"refusing: {ref}:{PATH} hashes to {actual}, not {SOURCE_SHA256}")
    document = json.loads(text)
    entries = [entry(e, x) for e in document["evals"] for x in e["expectations"]]
    ids = [e["id"] for e in entries]
    assert len(ids) == len(set(ids)), "expectation ids are not unique"
    source = {"ref": ref, "path": PATH, "sha256": actual}
    out.write_text(
        tomli_w.dumps({"schema": "suite/1", "source": source, "expectations": entries})
    )
    print(f"{len(entries)} entries -> {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
