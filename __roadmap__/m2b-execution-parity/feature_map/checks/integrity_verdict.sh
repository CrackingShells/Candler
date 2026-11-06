#!/bin/sh
# X33: integrity is valid, or invalid with a reason drawn from R01 §Ends.
: "${RUN_DIR:?}"
jq -e '.integrity | (.kind=="valid") or (.kind=="invalid" and (.reason | test("^(gateway|budget_stop|killed|interrupted|input_leak|collection_error|reader_error|format_unknown)$")))' "$RUN_DIR/manifest.json" >/dev/null 2>&1 \
 || { echo "FAIL integrity_verdict: integrity absent or reason outside the Ends table"; exit 1; }
echo "OK integrity_verdict"
