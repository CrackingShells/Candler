#!/bin/sh
# S2, X: pre-run work dir holds MCP server .py files, scripts/, and a bare pyproject.toml marking it a project.
: "${EXEC:?}"
x() { $EXEC sh -c "$1"; }
x 'test -f pyproject.toml && grep -q "^\[project\]" pyproject.toml && ! grep -q "build-system" pyproject.toml' \
  || { echo "FAIL layout: pyproject.toml missing, no [project], or has [build-system]"; exit 1; }
x 'test -d scripts && [ -n "$(find scripts -name "*.py" -print -quit)" ]' || { echo "FAIL layout: scripts/ missing or empty"; exit 1; }
x '[ -n "$(find . -name "*.py" -not -path "./scripts/*" -not -path "./.venv/*" -not -path "./.claude/*" -print -quit)" ]' \
  || { echo "FAIL layout: no MCP server .py file outside scripts/"; exit 1; }
echo "OK layout"
