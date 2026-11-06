#!/bin/sh
# Candler M2b test agent. Mode is the first argument:
#   stream | probe | ignore-term | sleep-forever
# Oracle fixture: it reports facts, it never judges them.
mode="${1:-}"

# Writes a file in $1; prints ok, erofs or denied. Removes what it made.
try_write() {
  d="$1"
  [ -n "$d" ] || { echo none; return; }
  f="$d/.candler-probe-$$"
  if err=$( { : > "$f"; } 2>&1 ); then
    rm -f "$f"; echo ok
  else
    case "$err" in
      *[Rr]ead-only*) echo erofs ;;
      *) echo denied ;;
    esac
  fi
}

case "$mode" in
stream)
  # Shape of a real Claude Code stream-json (streams/multi_init): an init frame, then a result frame.
  printf '%s\n' '{"type":"system","subtype":"init","cwd":"/workspace","session_id":"00000000-0000-4000-8000-000000000001","tools":["Bash","Edit","Read","Write"],"mcp_servers":[],"model":"test-model","permissionMode":"bypassPermissions","apiKeySource":"none","claude_code_version":"0.0.0","output_style":"default","skills":[],"plugins":[],"uuid":"00000000-0000-4000-8000-000000000002"}'
  printf '%s\n' '{"is_error":false,"duration_api_ms":1,"num_turns":1,"stop_reason":"end_turn","session_id":"00000000-0000-4000-8000-000000000001","total_cost_usd":0.001,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5},"permission_denials":[],"terminal_reason":"completed","subtype":"success","api_error_status":null,"result":"done","type":"result","duration_ms":1,"uuid":"00000000-0000-4000-8000-000000000003"}'
  ;;
probe)
  euid=$(id -u)
  capeff=$(sed -n 's/^CapEff:[[:space:]]*//p' /proc/self/status)
  nnp=$(sed -n 's/^NoNewPrivs:[[:space:]]*//p' /proc/self/status)
  w_root=$(try_write /)
  w_tmp=$(try_write /tmp)
  w_home=$(try_write "${HOME:-}")
  w_ws=$(try_write "$(pwd)")
  w_ro=$(try_write "${PROBE_RO:-}")
  if [ -n "${PROBE_SENTINEL:-}" ] && [ -e "$PROBE_SENTINEL" ]; then sent=true; else sent=false; fi
  conn=""
  for hp in ${PROBE_CONNECT:-}; do
    h="${hp%:*}"; p="${hp##*:}"
    if nc -w 2 "$h" "$p" </dev/null >/dev/null 2>&1; then r=open; else r=fail; fi
    conn="${conn:+$conn,}\"$hp\":\"$r\""
  done
  printf '{"euid":%s,"cap_eff":"%s","no_new_privs":"%s","write":{"root":"%s","tmp":"%s","home":"%s","workspace":"%s","ro":"%s"},"sentinel_exists":%s,"connect":{%s}}\n' \
    "$euid" "$capeff" "$nnp" "$w_root" "$w_tmp" "$w_home" "$w_ws" "$w_ro" "$sent" "$conn"
  ;;
ignore-term)
  trap '' TERM
  # Two children in their own sessions; each ignores TERM (inherited by sleep).
  setsid sh -c "trap '' TERM; while :; do sleep 1; done" &
  setsid sh -c "trap '' TERM; while :; do sleep 1; done" &
  while :; do sleep 1; done
  ;;
sleep-forever)
  exec sleep 2147483647
  ;;
*)
  echo "testagent: unknown mode '$mode'" >&2
  exit 64
  ;;
esac
