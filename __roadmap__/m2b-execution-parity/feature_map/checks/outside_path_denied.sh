#!/bin/sh
# S3.R14, X (hook rule): every Read/Write/Edit/Bash call naming a path outside the work dir (absolute, ~, or .. token; URLs and
# /dev/{null,zero,random,urandom,stdout,stderr,tty} excepted) got an error result. Passes vacuously if the agent never tried.
set -f
: "${STREAM_JSONL:?}"
ev=$(jq -nR '[inputs | fromjson? | select(type == "object")]' "$STREAM_JSONL")
cwd=$(printf '%s' "$ev" | jq -r '[.[] | select(.type == "system" and .subtype == "init")] | first | .cwd')
printf '%s' "$ev" | jq -r '.[] | select(.type == "assistant") | .message.content[]? | select(.type == "tool_use" and (.name | IN("Read","Write","Edit","Bash")))
  | [.id, (.input.file_path // .input.command // "")] | @tsv' > "${TMPDIR:-/tmp}/uses.$$"
bad=0; n=0
while IFS="$(printf '\t')" read -r id payload; do
  toks=$(printf '%s' "$payload" | sed -e 's/\\n/ /g' -e 's/\\t/ /g' -e 's#[A-Za-z][A-Za-z0-9+.-]*://[^ "'"'"'(){}<>|&;`]*##g' | tr -s ' "'"'"'(){}<>|&;`' '\n')
  out=0
  for t in $toks; do
    t=$(printf '%s' "$t" | sed 's/[.,:;]*$//')
    [ -n "$t" ] && [ -n "$(printf '%s' "$t" | tr -d /)" ] || continue
    case "$t" in /dev/null|/dev/zero|/dev/random|/dev/urandom|/dev/stdout|/dev/stderr|/dev/tty) continue;; esac
    case "$t" in /*|~*|*..*) ;; *) continue;; esac
    r=$(printf '%s' "$t" | awk -v c="$cwd" '{ p=$0; if (p !~ /^\//) p=c "/" p; m=split(p,a,"/"); k=0
      for(i=1;i<=m;i++){ if(a[i]==""||a[i]==".")continue; if(a[i]==".."){if(k>0)k--;continue}; s[++k]=a[i] }
      o=""; for(i=1;i<=k;i++)o=o "/" s[i]; print (o==""?"/":o) }')
    case "$t" in "~"*) r=/__home__;; esac
    case "$r" in "$cwd"|"$cwd"/*) ;; *) out=1;; esac
  done
  [ $out = 1 ] || continue
  n=$((n+1))
  ok=$(printf '%s' "$ev" | jq -r --arg id "$id" '[.[] | select(.type == "user") | .message.content[]? | select(.type == "tool_result" and .tool_use_id == $id)]
    | first | if . == null then "false" else ((.is_error // false) or ((.content | tostring) | test("denied|not permitted|outside"; "i"))) end')
  [ "$ok" = true ] || { echo "FAIL outside_path_denied: call $id used '$payload' outside $cwd and was not refused"; bad=1; break; }
done < "${TMPDIR:-/tmp}/uses.$$"
rm -f "${TMPDIR:-/tmp}/uses.$$"
[ $bad = 0 ] || exit 1
[ $n -gt 0 ] && echo "OK outside_path_denied ($n outside attempt(s) all refused)" || echo "OK outside_path_denied (vacuous: no outside-path attempt in stream)"
