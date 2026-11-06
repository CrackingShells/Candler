#!/bin/sh
# S3.R6, S3.R7, env table: agent venv set, inside the work dir, first on PATH; python and pip resolve inside it.
: "${EXEC:?}"
x() { $EXEC sh -c "$1"; }
W=$(x 'pwd -P'); V=$(x 'printf %s "$VIRTUAL_ENV"'); U=$(x 'printf %s "$UV_PROJECT_ENVIRONMENT"')
[ -n "$V" ] && [ "$V" = "$U" ] || { echo "FAIL venv_scoping: VIRTUAL_ENV='$V' UV_PROJECT_ENVIRONMENT='$U'"; exit 1; }
case "$V" in "$W"/*) ;; *) echo "FAIL venv_scoping: venv $V not inside work dir $W"; exit 1;; esac
[ "$(x 'printf %s "${PATH%%:*}"')" = "$V/bin" ] || { echo "FAIL venv_scoping: first PATH entry is not $V/bin"; exit 1; }
[ "$(x 'command -v python')" = "$V/bin/python" ] || { echo "FAIL venv_scoping: bare python is not the venv's"; exit 1; }
case "$(x 'command -v pip')" in "$V"/bin/pip*) ;; *) echo "FAIL venv_scoping: bare pip is not the venv's"; exit 1;; esac
[ "$(x 'python -c "import sys;print(sys.prefix)"')" = "$V" ] || { echo "FAIL venv_scoping: sys.prefix != venv"; exit 1; }
x 'pip --version' | grep -qF "$V" || { echo "FAIL venv_scoping: pip does not live in the venv"; exit 1; }
echo "OK venv_scoping"
