"""Round 03 measurement: Candler agent-token costs after round 02's data cut.

Run: python3 -I round03_measure.py > round03_output.txt
Source: every ~/.claude/projects/*Candler* dir; main <id>.jsonl plus <id>/subagents/*.jsonl.
Each message.id is counted once; records are filtered by their own timestamp.
One rate set everywhere (USD per MTok, Claude API list prices from the claude-api skill):
  Opus 5.5   input 4, output 20, cache write 5m 5 (1.25x), 1h 8 (2x), cache read 0.20
  Sonnet 5.5 input 2, output 10, cache write 5m 2.5,        1h 4,     cache read 0.10
"""
import glob, json, os, re, sys
from collections import defaultdict

HOME = os.path.expanduser("~")
CUT = "2026-11-13T01:22:00"      # round 02 data cut (UTC)
END = "2026-11-17T04:13:58"      # time round 03 started (UTC)
RATES = {  # in, out, w5m, w1h, read
    "opus-5-5": (4, 20, 5, 8, 0.20),
    "sonnet-5-5": (2, 10, 2.5, 4, 0.10),
    "opus-5": (5, 25, 6.25, 10, 0.50),
    "opus-4": (5, 25, 6.25, 10, 0.50),
    "sonnet-5": (2, 10, 2.5, 4, 0.20),
    "sonnet-4": (3, 15, 3.75, 6, 0.30),
    "haiku-4": (1, 5, 1.25, 2, 0.10),
    "haiku-5": (0.1, 0.5, 0.125, 0.2, 0.01),
    "fable-5-2": (8, 40, 10, 16, 0.20),
    "fable-5": (10, 50, 12.5, 20, 1.0),
}
OLD_SPEND_PY = {"opus": 0.40, "sonnet": 0.20}  # cache read used by spend.py


def rate(model):
    m = model or ""
    for k in sorted(RATES, key=len, reverse=True):
        if k in m:
            return k, RATES[k]
    return None, None


def short(model):
    m = (model or "?").replace("claude-", "")
    return m


def scan(cut, end, rates_override=None):
    """Return list of call dicts."""
    seen, calls = set(), []
    dirs = sorted(glob.glob(f"{HOME}/.claude/projects/*Candler*"))
    dirs = [d for d in dirs if "mcp-python" not in d and "HTN" not in d]
    for d in dirs:
        for main in sorted(glob.glob(f"{d}/*.jsonl")):
            sid = os.path.basename(main)[:-6]
            files = [(main, False)] + [(f, True) for f in sorted(glob.glob(f"{d}/{sid}/subagents/*.jsonl"))]
            for f, sub in files:
                for line in open(f, errors="replace"):
                    try:
                        r = json.loads(line)
                    except Exception:
                        continue
                    msg = r.get("message")
                    if not isinstance(msg, dict):
                        continue
                    u, mid = msg.get("usage"), msg.get("id")
                    ts = (r.get("timestamp") or "")[:19]
                    if not u or not mid or mid in seen or not ts:
                        continue
                    if not (cut < ts <= end):
                        continue
                    seen.add(mid)
                    cc = u.get("cache_creation") or {}
                    w5 = cc.get("ephemeral_5m_input_tokens")
                    w1 = cc.get("ephemeral_1h_input_tokens")
                    tot_w = u.get("cache_creation_input_tokens", 0) or 0
                    fallback = 0
                    if w5 is None and w1 is None:
                        w5, w1, fallback = tot_w, 0, 1
                    calls.append(dict(
                        dir=os.path.basename(d), sid=sid, sub=sub, ts=ts, model=msg.get("model"),
                        inp=u.get("input_tokens", 0) or 0, out=u.get("output_tokens", 0) or 0,
                        w5=w5 or 0, w1=w1 or 0, rd=u.get("cache_read_input_tokens", 0) or 0,
                        fb=fallback))
    return calls


def price(c):
    k, r = rate(c["model"])
    if not r:
        return None
    parts = dict(
        inp=c["inp"] * r[0], out=c["out"] * r[1], w5=c["w5"] * r[2], w1=c["w1"] * r[3], rd=c["rd"] * r[4])
    return {k2: v / 1e6 for k2, v in parts.items()}


def titles():
    t = {}
    for f in glob.glob(f"{HOME}/.claude/projects/*Candler*/*.jsonl"):
        sid = os.path.basename(f)[:-6]
        for line in open(f, errors="replace"):
            if '"customTitle"' in line:
                try:
                    t[sid] = json.loads(line).get("customTitle") or t.get(sid)
                except Exception:
                    pass
    return t


def group_of(c, title):
    d, sid, tl = c["dir"], c["sid"], (title or "").lower()
    ts = c["ts"]
    if sid.startswith("8776070d"):
        return "milestone coordinator"
    if "interlude" in tl or ("2026-11-13T03:27" <= ts <= "2026-11-13T04:45" and "m5" not in tl and "driver" not in tl):
        return "interlude audit"
    if "amend" in tl or "rdm" in d:
        return "roadmap amendments"
    if sid.startswith("9af68a2d"):
        return "predecessor coordinator (outside the audit window)"
    if sid.startswith("3fa53dcd"):
        return "side chat (outside the audit window)"
    if "m5b" in d or "priceless-hermann" in d or sid.startswith("8cc176cd"):
        return "M5b"
    if "m5-container" in d or re.search(r"\bm5a?\b", tl):
        return "M5 / M5a"
    if "cost" in tl and "report" in tl:
        return "cost-report session"
    if re.search(r"driver|c8", tl) or "driver" in d:
        return "driver work"
    if "roadmap" in tl or "amend" in tl or "rdm" in d:
        return "roadmap amendments"
    return "other: " + (title or d[-30:])


def main():
    ttl = titles()
    calls = scan(CUT, END)
    unk = defaultdict(int)
    for c in calls:
        c["usd"] = price(c)
        if c["usd"] is None:
            unk[c["model"]] += 1
    calls = [c for c in calls if c["usd"]]
    print(f"window {CUT}Z .. {END}Z; calls {len(calls)}; unpriced calls by model {dict(unk)}")
    print("fallback (no 5m/1h split) calls:", sum(c["fb"] for c in calls))

    # per session
    S = defaultdict(lambda: defaultdict(float))
    for c in calls:
        s = S[c["sid"]]
        t = sum(c["usd"].values())
        s["usd"] += t
        s["calls"] += 1
        s["rd_usd"] += c["usd"]["rd"]
        s["sub_usd" if c["sub"] else "main_usd"] += t
        s["sub_calls" if c["sub"] else "main_calls"] += 1
        s["dir"] = c["dir"]
        s["first"] = min(s["first"] or c["ts"], c["ts"]) if s["first"] else c["ts"]
        s["last"] = max(s["last"], c["ts"]) if s["last"] else c["ts"]
        mk = "opus" if "opus" in (c["model"] or "") else "sonnet" if "sonnet" in (c["model"] or "") else "other"
        s["u_" + mk] += t
    print("\n## per session (id8, dir tail, title, usd, calls, first, last)")
    for sid, s in sorted(S.items(), key=lambda kv: -kv[1]["usd"]):
        print(f"{sid[:8]} | {str(s['dir'])[-34:]} | {ttl.get(sid)} | {s['usd']:.1f} | {int(s['calls'])} | {s['first']} | {s['last']}")

    # per group
    G = defaultdict(lambda: defaultdict(float))
    for c in calls:
        g = G[group_of(c, ttl.get(c["sid"]))]
        t = sum(c["usd"].values())
        g["usd"] += t
        g["calls"] += 1
        g["rd"] += c["usd"]["rd"]
        g["sub" if c["sub"] else "main"] += t
        g["subc" if c["sub"] else "mainc"] += 1
        mk = "opus" if "opus" in (c["model"] or "") else "sonnet" if "sonnet" in (c["model"] or "") else "other"
        g[mk] += t
        for k in ("inp", "out", "w5", "w1", "rd"):
            g["usd_" + k] += c["usd"][k]
        g["sessions_" + c["sid"]] = 1
    print("\n## per group: usd | main usd | sub usd | main calls | sub calls | opus usd | sonnet usd | other usd | cache-read share of usd | sessions")
    tot = 0
    for name, g in sorted(G.items(), key=lambda kv: -kv[1]["usd"]):
        n = sum(1 for k in g if k.startswith("sessions_"))
        print(f"{name} | {g['usd']:.1f} | {g['main']:.1f} | {g['sub']:.1f} | {int(g['mainc'])} | {int(g['subc'])} | "
              f"{g['opus']:.1f} | {g['sonnet']:.1f} | {g['other']:.1f} | {100*g['rd']/g['usd']:.0f}% | {n}")
        tot += g["usd"]
    print(f"TOTAL {tot:.1f}")
    # token categories
    cat = defaultdict(float)
    for c in calls:
        for k, v in c["usd"].items():
            cat[k] += v
    print("\n## by category usd:", {k: round(v, 1) for k, v in cat.items()})
    mm = defaultdict(lambda: [0, 0.0])
    for c in calls:
        mm[short(c["model"])][0] += 1
        mm[short(c["model"])][1] += sum(c["usd"].values())
    print("## by model (calls, usd):", {k: (v[0], round(v[1], 1)) for k, v in mm.items()})
    # coordinator detail: long-lived overhead
    print("\n## coordinator and long-lived sessions: calls, mean context (input+cache read+write, k), usd per call")
    for sid, s in sorted(S.items(), key=lambda kv: -kv[1]["usd"])[:8]:
        cs = [c for c in calls if c["sid"] == sid and not c["sub"]]
        if cs:
            ctx = sum(c["inp"] + c["rd"] + c["w5"] + c["w1"] for c in cs) / len(cs) / 1000
            print(f"{sid[:8]} {ttl.get(sid)} main calls {len(cs)} mean ctx {ctx:.0f}k, main usd {s['main_usd']:.1f}, usd/call {s['main_usd']/len(cs):.3f}")
    # daily
    D = defaultdict(float)
    for c in calls:
        D[c["ts"][:10]] += sum(c["usd"].values())
    print("\n## by day:", {k: round(v, 1) for k, v in sorted(D.items())})

    # interlude cross-check window
    it = [c for c in calls if "2026-11-13T03:27" <= c["ts"] <= "2026-11-13T04:45"]
    print(f"\n## all Candler calls 11-13 03:27-04:45 UTC: {sum(sum(c['usd'].values()) for c in it):.1f} USD")

    # rate check over rounds 00-02 (everything up to the cut)
    allc = scan("0000", CUT)
    new = old = n = 0
    for c in allc:
        p = price(c)
        if not p:
            continue
        n += 1
        new += sum(p.values())
        k, r = rate(c["model"])
        r02 = OLD_SPEND_PY_R02(c)
        old += r02
    print(f"\n## rounds 00-02 re-priced from transcripts (calls {n}, up to {CUT}Z):")
    print(f"corrected rates (Opus read 0.20, Sonnet read 0.10): {new:.1f} USD")
    print(f"round 02 rates (Opus read 0.20, Sonnet read 0.20):  {old:.1f} USD  (round 02 published 1,678)")
    r0 = scan("0000", "2026-11-12T00:30:00")
    print(f"round 00 (cut 2026-11-12 00:30Z) corrected: {sum(sum(price(c).values()) for c in r0 if price(c)):.1f} USD (round 00 published 1,346)")
    print(f"spend.py rates (Opus read 0.40, Sonnet read 0.20):  {OLD_SPEND(allc):.1f} USD")


def OLD_SPEND_PY_R02(c):
    k, r = rate(c["model"])
    rd = 0.20 if ("opus" in (c["model"] or "") or "sonnet" in (c["model"] or "")) else r[4]
    return (c["inp"] * r[0] + c["out"] * r[1] + c["w5"] * r[2] + c["w1"] * r[3] + c["rd"] * rd) / 1e6


def OLD_SPEND(calls):
    t = 0
    for c in calls:
        k, r = rate(c["model"])
        if not r:
            continue
        rd = OLD_SPEND_PY["opus"] if "opus" in (c["model"] or "") else OLD_SPEND_PY["sonnet"] if "sonnet" in (c["model"] or "") else r[4]
        t += (c["inp"] * r[0] + c["out"] * r[1] + c["w5"] * r[2] + c["w1"] * r[3] + c["rd"] * rd) / 1e6
    return t


if __name__ == "__main__":
    main()
