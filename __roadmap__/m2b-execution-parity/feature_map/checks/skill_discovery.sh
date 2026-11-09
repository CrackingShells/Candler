#!/bin/sh
# X (no --bare, project skill only): htn-creator is discoverable iff the work dir carries it (with_skill), never via a global copy (without_skill).
: "${STREAM_JSONL:?}"; : "${WORKSPACE_DIR:?}"
seen=$(jq -nR '[inputs | fromjson? | select(type == "object" and .type == "system" and .subtype == "init")] | first
  | ((.slash_commands // []) + (.skills // [])) | any(tostring | test("htn-creator"))' "$STREAM_JSONL")
if [ -f "$WORKSPACE_DIR/.claude/skills/htn-creator/SKILL.md" ]; then want=true; else want=false; fi
[ "$seen" = "$want" ] || { echo "FAIL skill_discovery: skill listed=$seen but work dir carries it=$want"; exit 1; }
echo "OK skill_discovery (listed=$seen)"
