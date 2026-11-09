"""The kinds of the engine guards: the helper's `ok` and `echo` kinds, a `fail` and a `counter`.

The echo kind is the helper's own, with the needs of the parity suite so that the context it
echoes is the live one. `counter` keeps its count in the file named by its `file` param.
"""

from pathlib import Path

from echo_plugin import echo
from synthetic_plugin import ok

NAME = "engine"
VERSION = "0.1.0"

KIND_SPECS = {
    "echo": {
        "needs": {
            "artifacts": ["outputs/**"],
            "metadata": ["plan.prompt_domain_name"],
        }
    },
}


def fail(params, context, prior):
    return {"status": "fail", "evidence": [{"note": "fails on purpose"}]}


def counter(params, context, prior):
    path = Path(params["file"])
    count = int(path.read_text()) + 1 if path.exists() else 1
    path.write_text(str(count))
    return {"status": "pass", "value": {"kind": "count", "count": count}}


KINDS = {"ok": ok, "echo": echo, "fail": fail, "counter": counter}
