"""A plug-in of one kind that needs the inputs `servers` and `truth` and reports what it read.

For each input of its context the kind hashes every file under the input's `path` and puts the
digest of the input, the sha256 of each file by its relative path, and the files it could write
into the evidence of its one outcome.
"""

import hashlib
import json
import os
from pathlib import Path

NAME = "seam"
VERSION = "0.1.0"

KIND_SPECS = {
    "read_inputs": {"needs": {"inputs": ["servers", "truth"]}},
}


def read_inputs(params, context, prior):
    seen = {}
    for item in context.inputs:
        root = Path(item["path"])
        files = {}
        writable = []
        for path in sorted(root.rglob("*")):
            if path.is_file():
                relative = path.relative_to(root).as_posix()
                files[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
                if os.access(path, os.W_OK):
                    writable.append(relative)
        seen[item["kind"]] = {
            "digest": item["digest"],
            "files": files,
            "writable": writable,
        }
    return {"status": "pass", "evidence": [{"note": json.dumps(seen, sort_keys=True)}]}


KINDS = {"read_inputs": read_inputs}
