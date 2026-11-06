"""An echo plug-in: its one kind returns the context and prior it received as evidence."""

import json

NAME = "echo"
VERSION = "0.1.0"


def echo(params, context, prior):
    seen = {
        "artifacts": [
            {"path": a["path"], "sha256": a["sha256"]} for a in context.artifact_files
        ],
        "snapshots": [
            {"kind": s["kind"], "digest": s["digest"]} for s in context.snapshots
        ],
        "inputs": [{"kind": i["kind"], "digest": i["digest"]} for i in context.inputs],
        "metadata": dict(context.metadata),
        "prior": [
            {"expectation": p.expectation, "outcome": dict(p.outcome)} for p in prior
        ],
    }
    return {"status": "pass", "evidence": [{"note": json.dumps(seen, sort_keys=True)}]}


KINDS = {"echo": echo}
