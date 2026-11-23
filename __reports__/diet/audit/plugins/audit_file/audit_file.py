"""A plug-in whose kind reads the file the agent wrote and checks it holds the stamp."""
NAME = "audit-file"
VERSION = "0.1.0"
STAMP = "stamp-7f3a"


def file_says(params, context, prior):
    files = [a for a in context.artifact_files if a["path"] == "outputs/answer.txt"]
    if not files:
        return {"status": "fail", "evidence": [{"note": "outputs/answer.txt is not among the artifacts"}]}
    text = (context.root / files[0]["path"]).read_text(encoding="utf-8")
    ok = text.strip() == STAMP
    return {"status": "pass" if ok else "fail", "evidence": [{"note": f"outputs/answer.txt holds {text.strip()!r}"}]}


KINDS = {"file_says": file_says}
KIND_SPECS = {"file_says": {"needs": {"artifacts": ["outputs/**"]}}}
