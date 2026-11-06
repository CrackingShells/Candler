# M2b Gateway Spike: A (host proxy) vs B (internal network + sidecar) - Findings (v0)

Date: 2026-11-06

---
type: findings
topic: m2b-execution-parity
date: 2026-11-06
version: v0
prior-version: none
key-metric: canary leaks into agent-visible surfaces: 0 of 12 probes, both designs (prior: N/A, delta: N/A)
decision-required: confirm
---

## Headline Result

metric: designs meeting every checklist criterion
value: B 8 of 8 with one caveat (cap rejects, does not queue); A 7 of 8 (fails "no egress", by construction)
unit: criteria
prior: N/A
direction: new

Recommendation: **B, nginx sidecar.** Off-the-shelf nginx covers the logging need, so the lead's escape hatch to A does not trigger.

Engine: Docker Desktop 29.9.2, linux/arm64. Fake upstream only; no real token, no LLM call. Canary = `openssl rand -hex 16`.

## Topology tested

```mermaid
flowchart LR
  subgraph int["candler-spike-gwspike-int  (--internal)"]
    AG["agent<br/>dummy token<br/>BASE_URL=http://gw:8080"]
    GW["gateway nginx<br/>token in tmpfs /run/gw"]
  end
  subgraph egr["candler-spike-gwspike-egress (bridge)"]
    UP["fake upstream<br/>echoes headers"]
    CTL["control container"]
  end
  AG -->|dummy| GW
  GW -->|real canary| UP
  CTL -.control.-> UP
```

Design A: agent on default bridge, `host.docker.internal:18991` -> python host proxy (127.0.0.1) -> upstream's published loopback port.

## Results Tables

### Criterion 1: canary absence (B unless noted; count of matches, 0 = absent)

| Surface | Command (abridged) | B | A |
|---|---|---|---|
| agent `docker inspect` | `docker inspect <agent> \| grep -c $C` | 0 | 0 |
| sidecar `docker inspect` (incl. Env/Entrypoint/Cmd) | same | 0 | n/a |
| both networks `docker network inspect` | same | 0 | n/a |
| agent `env` | `docker exec <agent> env \| grep -c` | 0 | 0 |
| agent `/proc/*/environ` and `/proc/*/cmdline` | `tr '\0' '\n' < /proc/N/environ` loop, script fed on stdin | 0 | 0 |
| agent filesystem | `grep -rl $C` over every top-level dir except proc, sys, dev | 0 (positive control: planted file found) | 0 |
| agent logs | `docker logs <agent> \| grep -c` | 0 | n/a |
| gateway logs | `docker logs <gw> \| grep -c` | 0 | proxy log: 0 |
| gateway env, `/proc/1/environ`, nginx master cmdline, `docker top` | as above | 0 | n/a |
| gateway filesystem | `grep -rl` | only `/run/gw/auth.conf` (tmpfs, mode 0700, root) | n/a |
| gateway `docker diff` | `docker diff <gw>` | only `/run/gw` | n/a |
| agent-visible response body from a header-echoing upstream | `/v1/messages`, `/err` | 0 (`Bearer <redacted>`) | 0 (my proxy scrubs) |

### Criterion 2: the swap

Agent sent `Authorization: Bearer dummy-token-not-real` and `x-api-key: dummy...`. Upstream log (B and A) recorded `auth="Bearer <canary>"` and `x-api-key=<canary>`. The dummy never reached upstream. Same for the upstream's 401 `/err` path.

### Criterion 3: B has no route off the internal network (agent), bridge control (ctl)

| Probe (busybox `wget -T 4`) | Agent (internal only) | Control (bridge) |
|---|---|---|
| `http://1.1.1.1/` | Network unreachable | 200 body |
| `http://example.com/` | bad address (no DNS) | 200 body |
| `host.docker.internal:<published port>` | bad address (SERVFAIL) | 200 body |
| `192.168.65.254:<port>` (Desktop VM host IP) | Network unreachable | n/a |
| `172.20.0.1:<port>` (internal net's own gateway IP) | **connection refused** (address answers, nothing listens) | n/a |
| gateway's egress-side IP `172.21.0.3:8080` | Network unreachable (cross-network isolation holds) | 200 |
| `gw:8080/healthz` | ok | bad address |

### Criterion 4: `host.docker.internal` under A

| Check | Result on Docker Desktop |
|---|---|
| Resolves from a bridge container | yes (`192.168.65.254` via nslookup, `fdc4:...::254` via getent), also with `--add-host host.docker.internal:host-gateway` |
| Reaches a host proxy bound to `127.0.0.1` | **yes** (Desktop forwards to host loopback) |
| Proxy bound to `0.0.0.0` | reachable from the host's LAN IP too: an open token-swapping proxy on the LAN |

### Criteria 5-7: logging, streaming, cap

| Item | B (nginx) | A (host proxy) |
|---|---|---|
| Log fields | method, `$uri` (no query string), status, request time, upstream time, request bytes, response bytes, body bytes, model | method, path (no query), status, ms, request bytes, response bytes; model needs code |
| Model | `map $request_body` regex, works up to `client_body_buffer_size` (8 MB); larger bodies log `model=-`. Prompt text never logged | json parse in code |
| Auth scrub, request | headers are never logged (allow-list by omission); `error_log warn` shows request line only | same by code |
| Auth scrub, response body | `sub_filter` on the token, `sub_filter_types *`, `Accept-Encoding ""` upstream: echoes appear as `<redacted>` | `bytes.replace` in code |
| Auth scrub, response header | `proxy_hide_header` by name only; an unknown echo header is not scrubbed | allow-list of response headers |
| SSE | 5 chunks 1 s apart arrive at 0.01, 1.01, 2.02, 3.02, 4.02 s with `sub_filter` active, `proxy_buffering off` | same arrivals |
| Concurrency cap 2 on 5 simultaneous 3 s requests | 2 x 200 after 3 s, 3 x **429** after 0 s (`limit_conn`, key `"all"`) | 200 after 3, 3, 6, 6, 9 s: **queues** (semaphore) |

### Criterion 8: teardown

Labelled `candler.spike=gwspike`, removed by recorded id after `docker inspect` confirmed the label: 6 containers (upstream, gateway, 2 agents, control, agent-A) and 2 networks. The host proxy exited through a `/__quit` request, not a signal. `docker ps -a --filter label=candler.spike=gwspike -q` and the network equivalent print nothing. One transient `docker run --rm` (nginx -V) auto-removed.

## Observations

| Signal | Baseline / Expected | Observed [source] | Interpretation |
|---|---|---|---|
| Token path off argv and env | stdin write, no env, no argv | `printf` builtin piped to `docker exec -i`; inspect and `/proc/1/environ` clean [c1.out] | Works. The file exists only in tmpfs, absent from `docker diff` |
| Sidecar start order | token before first request | entrypoint waits for `/run/gw/auth.conf`; before injection port 8080 refuses [restart test] | Fails closed. A `docker restart` empties the tmpfs, so the supervisor must re-inject; readiness gates it |
| `limit_conn` default key | works on `$server_name` | **silently inert**: unnamed server gives `$server_name == ""`, which is not counted | Use a literal key (`"all"`). A zone key also cannot change on reload, so rename the zone |
| Rejecting cap | queue like youzuoqi's semaphore | 429 immediately | Claude Code retries 429, so tolerable; the retry pressure is the cost. nginx OSS cannot queue |
| youzuoqi proxy | scrubs what the agent sees | `rv_logging_proxy.py` scrubs only the log copy; the chunk written to the client is unscrubbed [harness/rv_logging_proxy.py] | A done as youzuoqi does it leaks an echoed key to the agent. A needs the scrub on the wire |
| Internal net gateway IP | unreachable | answers "connection refused" | Harmless on Desktop; stays closed only while nothing on the host/VM listens on the internal network address |

## Contradictions & Surprises

- Quoted-tilde map syntax (`~"..."`) passed `nginx -t` and silently matched nothing. A log probe is needed, not just a syntax check.
- Two probe mistakes produced false canary hits: the canary on my own argv, and `grep -r` into `/dev/stdin` hanging a `docker exec`. Probes feed the canary on stdin and skip `/dev`.

## Decision table

| Criterion | A: host proxy | B: internal net + nginx sidecar |
|---|---|---|
| 1 Canary absent from agent surfaces | pass | pass |
| 1 Canary absent from gateway logs | pass | pass |
| 2 Upstream receives real header | pass | pass |
| 3 No route off the network | **fail**: agent has internet and the host's other ports | pass |
| 4 Host reachability | works on Desktop, even loopback bind; Linux needs `host-gateway` and a bridge-IP bind; 0.0.0.0 exposes LAN | not needed |
| 5 Scrub and logging | pass if coded (new Rust HTTP stack and an HTTP crate) | pass with config only; header echo needs a name list |
| 6 SSE | pass | pass |
| 7 Concurrency cap | pass, queues | pass, rejects with 429 |
| 8 Teardown by label | host proxy lives in the supervisor, no label to reap | pass, one more labelled container and network |
| Same behaviour on Linux and Desktop | no | yes |

## Recommendation

**B.** Reasons, strongest first:
1. It is the only design where the agent has no route to the host or internet (criterion 3), and it behaves the same on Docker Desktop and Linux.
2. nginx logging is enough: model, status, latencies, byte counts, no auth, no prompt. No custom code, so no new HTTP dependency.
3. The token is only in a 0700 tmpfs file, never on env or argv; the sidecar fails closed until the token is injected.

Cost: no queueing (429), and one more container.

### Chosen design, exact recipe

- **Image:** `nginx@sha256:a8b39bd9cf0f83869a2162827a0caf6137ddf759d50a171451b335cecc87d236` (nginx 1.28.3 alpine, arm64 manifest in that index; includes `sub_filter` and `ssl`). Spike also used `python@sha256:4d511039677430640754cd1417737349067dfcdaba4dcf7e576334516f9f4c30` for agent, probes and the fake upstream.
- **Config:** below, mounted read-only at `/etc/nginx/nginx.conf`. `proxy_pass` points at the fake upstream; the real one is `https://<litellm>` and needs `proxy_ssl_server_name on;`, `proxy_ssl_verify on;`, a `resolver` and `proxy_set_header Host <host>` (TLS not exercised in the spike).

```nginx
worker_processes 1;
error_log /dev/stderr warn;
pid /tmp/nginx.pid;
events { worker_connections 512; }
http {
  access_log off;
  map $request_body $gw_model { "~\"model\"[[:space:]]*:[[:space:]]*\"(?<m>[A-Za-z0-9._:/@-]+)\"" $m; default "-"; }
  log_format gw 'ts=$time_iso8601 method=$request_method path=$uri status=$status req_ms=$request_time up_ms=$upstream_response_time req_bytes=$request_length resp_bytes=$bytes_sent body_bytes=$body_bytes_sent model=$gw_model';
  limit_conn_zone "all" zone=cap_all:64k;
  client_body_buffer_size 8m;
  client_max_body_size 64m;
  server {
    listen 8080;
    access_log /dev/stdout gw;
    include /run/gw/auth.conf;          # Authorization, x-api-key, sub_filter: written from stdin
    sub_filter_once off;  sub_filter_types *;
    proxy_http_version 1.1;  proxy_set_header Connection "";
    proxy_set_header Accept-Encoding "";  proxy_set_header Host $proxy_host;
    proxy_buffering off;  proxy_request_buffering on;  proxy_read_timeout 600s;
    location = /healthz { access_log off; default_type text/plain; return 200 "ok\n"; }
    location / { limit_conn cap_all 2; limit_conn_status 429; proxy_pass http://fake-upstream:8080; }
  }
}
```

- **Network commands** (every object `--label candler.spike|candler.job=<id>`; names `candler-<job>-int|egress`):
  1. `docker network create --internal <int>`; `docker network create <egress>`
  2. `docker create --name <gw> --network <int> --network-alias gw --read-only --tmpfs /run/gw:rw,mode=0700,size=64k,noexec,nosuid --tmpfs /tmp --tmpfs /var/cache/nginx --cap-drop ALL --cap-add CHOWN --cap-add SETUID --cap-add SETGID --security-opt no-new-privileges -v <conf>:/etc/nginx/nginx.conf:ro --entrypoint sh <image> -c 'until [ -s /run/gw/auth.conf ]; do sleep 0.1; done; exec nginx -g "daemon off;"'`
  3. `docker network connect <egress> <gw>`; `docker start <gw>`
  4. Agent: `docker run --network <int> -e ANTHROPIC_BASE_URL=http://gw:8080 -e ANTHROPIC_AUTH_TOKEN=<dummy>`, no other network.
- **Token injection** (supervisor writes to the child's stdin; the secret never touches argv or env):
  `printf 'proxy_set_header Authorization "Bearer %s";\nproxy_set_header x-api-key "%s";\nsub_filter "%s" "<redacted>";\n' T T T | docker exec -i <gw> sh -c 'umask 077; cat > /run/gw/auth.tmp && mv /run/gw/auth.tmp /run/gw/auth.conf'`. In Rust, write the same bytes to the exec child's piped stdin. Re-inject after any restart of the sidecar.
- **Readiness:** `docker exec <gw> wget -q -T 2 -O- http://127.0.0.1:8080/v1/models` returns the model list (proves token loaded and upstream reachable); `/healthz` proves only nginx is up. Before injection the port refuses, so a failed probe means "not ready", never "unauthenticated".
- **Teardown:** `docker rm -f <recorded cid>` for gateway and agent after a label check, then `docker network rm <recorded id>` for both networks.

### What a Linux engine changes

Not tested here (Desktop only); CI should confirm.

| Item | B | A |
|---|---|---|
| Sidecar, internal network, aliases | unchanged | n/a |
| `--internal` | same isolation (no gateway, no masquerade) | n/a |
| Host reachability | none required | `--add-host host.docker.internal:host-gateway`; proxy must bind the bridge IP (`172.17.0.1` or the job network gateway), since `127.0.0.1` is unreachable; `0.0.0.0` exposes the host's LAN |
| Egress | still no route | agent keeps the host's whole network unless a firewall rule is added |
| Rootless Docker | slirp4netns or pasta, re-check the `--internal` probe | host-gateway semantics differ |

## Steering Questions

- [now] Accept B and 429 on the cap? The alternative to queue is a custom sidecar, which gives up "off-the-shelf".
- [now] Does the real LiteLLM terminate TLS? The recipe then adds the `proxy_ssl_*` lines; they were not exercised.
- [next run] Make `isolation.network = bridge` an explicit per-cell value, since B's default removes the agent's internet.
- [next run] Should response headers be allow-listed (an njs module) rather than hidden by name? The spike hid only the known fake header.
- [later] Reuse the internal network plus sidecar as the M5 egress allowlist: add an allowed-host `proxy_pass` map.

## Pointers

- Plan decision 7: `/Users/hacker/.claude/plans/system-reminder-linked-sessions-now-stateful-pascal.md`
- Spike scripts and raw outputs: `/private/tmp/claude-501/m2b-phase0/gwspike/` (`up.sh`, `nginx.conf`, `upstream.py`, `hostproxy.py`, `c1.out`, `c1b.out`, `ids.txt`, `nets.txt`)
- youzuoqi proxy: `/Users/hacker/Documents/explore/youzuoqi/harness/rv_logging_proxy.py`
- Roadmap leaf: `__roadmap__/m2b-execution-parity/gateway.md`
