#!/bin/sh
# S3.R8, S3.R9, S3.R19, X: pre-run venv = Python 3.12, the five stable deps in range, one planner, no answer files, no htn_creator_scripts.
: "${EXEC:?}"
x() { $EXEC sh -c "$1"; }
V=$(x 'printf %s "$VIRTUAL_ENV"')
x 'python -c "import sys;print(\"%d.%d\" % sys.version_info[:2])"' | grep -qx 3.12 || { echo "FAIL venv_contents: python is not 3.12"; exit 1; }
j=$(x 'python -m pip list --format=json') || { echo "FAIL venv_contents: pip list failed (pip not seeded?)"; exit 1; }
printf '%s' "$j" | jq -e '
  def n: ascii_downcase | gsub("[-_.]+"; "-");
  (map({(.name|n): .version}) | add) as $d
  | def maj($k): $d[$k] | split(".")[0] | tonumber;
  ($d | has("libcst") and has("jsonschema") and has("python-arango") and has("python-dotenv") and has("mcp"))
  and maj("libcst") == 1 and maj("python-arango") == 8 and maj("python-dotenv") == 1 and maj("jsonschema") >= 4
  and (($d.mcp | split(".") | map(tonumber? // 0)) as $m | $m[0] > 1 or ($m[0] == 1 and $m[1] >= 18))
  and (($d | has("htn-creator-scripts")) | not) and (($d | has("scipy")) | not) and (($d | has("gtpyhop-examples")) | not)
  and (([$d | has("gtpyhop"), has("gtpyhop-core")] | map(select(.)) | length) == 1)' >/dev/null \
  || { echo "FAIL venv_contents: dependency set wrong (need 5 stable deps, exactly one planner, no htn_creator_scripts/scipy/examples)"; exit 1; }
[ -z "$(x "find '$V' \\( -name domain.py -o -name problems.py \\) -print -quit")" ] || { echo "FAIL venv_contents: domain.py/problems.py inside venv"; exit 1; }
x 'python -c "import gtpyhop"' >/dev/null 2>&1 || { echo "FAIL venv_contents: planner not importable"; exit 1; }
echo "OK venv_contents"
