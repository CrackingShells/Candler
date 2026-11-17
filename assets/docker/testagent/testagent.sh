#!/bin/sh
# Candler M2b test agent. Mode is the first argument:
#   stream | probe | ignore-term | sleep-forever | exit3 | stream-bytes | echo-stdin | term-echo | seeker
#   seeker <pattern> [phase...]  Looks for a canary the agent must never see. <pattern> is an
#     extended regular expression given at run time (the image holds no canary); each phase
#     (default: start turn end, the order of a run) prints one report on stdout, where the
#     harness's trace is read:
#       seeker:<phase>:file:<path>    a readable file outside /proc and /sys whose path or bytes match
#       seeker:<phase>:environ:<pid>:<line>   every line of every /proc/<pid>/environ
#       seeker:<phase>:mount:<line>   the mount table
#       seeker:<phase>:done           the phase's report is complete
# `timeout <args>` runs `timeout` on <args>, so the run's expiry wrapper reaches the mode.
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

# Reports, for phase $2, what could be read that matches the pattern $1.
seek() {
  pat="$1"; ph="$2"
  find / \( -path /proc -o -path /sys \) -prune -o -type f -print 2>/dev/null |
    while IFS= read -r f; do
      [ -r "$f" ] || continue
      if printf '%s\n' "$f" | grep -Eq -- "$pat"; then
        printf 'seeker:%s:file:%s\n' "$ph" "$f"
      elif grep -Eaq -- "$pat" "$f" 2>/dev/null; then
        printf 'seeker:%s:file:%s\n' "$ph" "$f"
      fi
    done
  for e in /proc/[0-9]*/environ; do
    [ -r "$e" ] || continue
    pid="${e#/proc/}"; pid="${pid%/environ}"
    tr '\000' '\n' < "$e" 2>/dev/null | while IFS= read -r l; do
      printf 'seeker:%s:environ:%s:%s\n' "$ph" "$pid" "$l"
    done
  done
  while IFS= read -r l; do printf 'seeker:%s:mount:%s\n' "$ph" "$l"; done < /proc/self/mounts
  printf 'seeker:%s:done\n' "$ph"
}

case "$mode" in
seeker)
  shift
  pat="${1:?seeker needs a pattern}"; shift
  [ "$#" -gt 0 ] || set -- start turn end
  for ph in "$@"; do seek "$pat" "$ph"; done
  ;;
timeout)
  shift
  exec timeout "$@"
  ;;
exit3)
  printf 'frame-1\n'
  printf '\000\377tail-without-newline'
  printf 'err-1\n' >&2
  exit 3
  ;;
stream-bytes)
  printf '{"n":1}\n{"n":2}\n\000\377\001no-newline'
  ;;
echo-stdin)
  while IFS= read -r line; do printf 'echo:%s\n' "$line"; done
  ;;
term-echo)
  trap 'echo got-term; exit 143' TERM
  echo ready
  while :; do sleep 1; done
  ;;
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
