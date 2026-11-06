#!/bin/sh
# S3.R4, S3.R5, env table: interpreter-locator vars gone, no conda/foreign venv on PATH, bytecode off, CLAUDECODE gone.
: "${EXEC:?}"
env_out=$($EXEC env) || { echo "FAIL env_scrubbed: cannot run env"; exit 1; }
for v in CONDA_PREFIX CONDA_DEFAULT_ENV MAMBA_EXE MAMBA_ROOT_PREFIX CONDA_EXE CONDA_SHLVL PYTHONPATH PYTHONHOME CLAUDECODE; do
  printf '%s\n' "$env_out" | grep -q "^$v=" && { echo "FAIL env_scrubbed: $v is set"; exit 1; }
done
printf '%s\n' "$env_out" | grep -qx 'PYTHONDONTWRITEBYTECODE=1' || { echo "FAIL env_scrubbed: PYTHONDONTWRITEBYTECODE!=1"; exit 1; }
V=$(printf '%s\n' "$env_out" | sed -n 's/^VIRTUAL_ENV=//p')
printf '%s\n' "$env_out" | sed -n 's/^PATH=//p' | tr ':' '\n' | while read -r p; do
  case "$p" in */condabin|*/miniforge*|*/miniconda*|*/anaconda*) echo "FAIL env_scrubbed: PATH has $p"; exit 1;; esac
  case "$p" in */.venv/bin) [ "$p" = "$V/bin" ] || { echo "FAIL env_scrubbed: foreign venv on PATH: $p"; exit 1; };; esac
done || exit 1
echo "OK env_scrubbed"
