#!/bin/sh
# X30 (I17): every stored path is relative; no value is an absolute path; inputs are digests; secrets hold names only.
: "${RUN_DIR:?}"; m="$RUN_DIR/manifest.json"
jq -e '
 def rel: type=="string" and (startswith("/")|not) and (test("^[A-Za-z]:[\\\\/]")|not) and (split("/")|index("..")|not);
 ([.sources[]?.path] | all(rel))
 and ([.comparable.values[]? | .value? | select(type=="string") | select(startswith("/"))] | length==0)
' "$m" >/dev/null || { echo "FAIL no_abs_paths: absolute or escaping path in manifest"; exit 1; }
if [ -f "$RUN_DIR/tree.json" ]; then
  jq -e '(if (.files? | type)=="object" then .files else . end) | keys | all(.[]; (startswith("/")|not) and (split("/")|index("..")|not))' "$RUN_DIR/tree.json" >/dev/null \
   || { echo "FAIL no_abs_paths: tree.json holds an absolute or escaping path"; exit 1; }
fi
echo "OK no_abs_paths"
