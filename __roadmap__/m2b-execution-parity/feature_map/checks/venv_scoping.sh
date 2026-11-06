#!/bin/sh
# S3.R6, S3.R7 (A13), env table: agent venv set, writable by the agent, first on PATH; python and pip resolve inside it.
: "${EXEC:?}"
x() { $EXEC sh -c "$1"; }
V=$(x 'printf %s "$VIRTUAL_ENV"'); U=$(x 'printf %s "$UV_PROJECT_ENVIRONMENT"')
[ -n "$V" ] && [ "$V" = "$U" ] || { echo "FAIL venv_scoping: VIRTUAL_ENV='$V' UV_PROJECT_ENVIRONMENT='$U'"; exit 1; }
# S3.R7, amended by ruling A13: the env is the agent's own and writable (a per-run volume copy), not a path under $W.
x "touch '$V/.candler_probe' && rm '$V/.candler_probe'" || { echo "FAIL venv_scoping: the agent cannot write its venv $V"; exit 1; }
[ "$(x 'printf %s "${PATH%%:*}"')" = "$V/bin" ] || { echo "FAIL venv_scoping: first PATH entry is not $V/bin"; exit 1; }
[ "$(x 'command -v python')" = "$V/bin/python" ] || { echo "FAIL venv_scoping: bare python is not the venv's"; exit 1; }
case "$(x 'command -v pip')" in "$V"/bin/pip*) ;; *) echo "FAIL venv_scoping: bare pip is not the venv's"; exit 1;; esac
[ "$(x 'python -c "import sys;print(sys.prefix)"')" = "$V" ] || { echo "FAIL venv_scoping: sys.prefix != venv"; exit 1; }
x 'pip --version' | grep -qF "$V" || { echo "FAIL venv_scoping: pip does not live in the venv"; exit 1; }
echo "OK venv_scoping"
