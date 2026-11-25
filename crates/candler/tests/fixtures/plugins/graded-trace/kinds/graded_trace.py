"""One kind that grades a run by what its trace says of the run's shell and MCP calls."""

from pathlib import Path

from candler_evaluator import read_trace

NAME = "graded-trace"
VERSION = "0.1.0"
KIND_SPECS = {
    "graded_run": {"needs": {"trace": True, "artifacts": ["outputs/answer.txt"]}},
}


def _verdict(problems, note):
    if problems:
        return {"status": "fail", "evidence": [{"note": "; ".join(problems)}]}
    return {"status": "pass", "evidence": [{"note": note}]}


def graded_run(params, context, prior):
    """Passes when the trace holds each `(argv, exit)` pair and the MCP origin that `params`
    names, each as a pair or an origin of the trace and not as a substring of it, and the
    artifact `params` names is served with its text."""
    if context.trace is None:
        return _verdict(["the context serves no trace"], "")
    argv_of = {}  # keyed by (session, span): a span is the harness's id within one session
    pairs = []
    origins = []
    for record in read_trace(context.trace):
        if record.kind != "event":
            continue
        body = record.event.kind.root.model_dump(mode="json")
        span = record.event.span.root if hasattr(record.event.span, "root") else None
        key = (record.session.root, span)
        if body["kind"] == "tool_call":
            origins.append(body["origin"])
            if "argv" in body:
                argv_of[key] = body["argv"]
        elif body["kind"] == "tool_result" and span is not None and key in argv_of and "exit" in body:
            pairs.append((argv_of[key], body["exit"]))
    problems = []
    for shell in params["shell"]:
        want = (shell["argv"], {"kind": "code", "code": shell["exit"]})
        if pairs.count(want) != 1:
            problems.append(f"{want} is held {pairs.count(want)} times among {pairs}")
    if origins.count(params["origin"]) != 1:
        problems.append(f"origin {params['origin']} is held {origins.count(params['origin'])} times among {origins}")
    served = [f for f in context.artifact_files if f["path"] == params["artifact"]["path"]]
    if len(served) != 1:
        problems.append(f"{params['artifact']['path']} is not served once among {context.artifacts}")
    elif Path(served[0]["abs"]).read_text(encoding="utf-8") != params["artifact"]["text"]:
        problems.append(f"{params['artifact']['path']} does not hold {params['artifact']['text']!r}")
    return _verdict(problems, f"pairs {pairs}; origins {origins}")


KINDS = {"graded_run": graded_run}
