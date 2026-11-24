"""A second plug-in on the same runs: counts the artifacts, so the two producers differ."""
NAME = "audit-second"
VERSION = "0.1.0"


def artifact_count(params, context, prior):
    n = len(context.artifact_files)
    return {"status": "pass" if n >= 1 else "fail", "value": {"kind": "count", "count": n},
            "evidence": [{"note": f"{n} artifact(s)"}]}


KINDS = {"artifact_count": artifact_count}
KIND_SPECS = {"artifact_count": {"needs": {"artifacts": ["outputs/**"]}}}
