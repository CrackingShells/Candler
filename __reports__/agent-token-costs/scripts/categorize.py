"""Categorise every tool call of a session tree and attribute cost and context to it.

usage: categorize.py <label>=<projects-dir>/<session-id> ...   (prints JSON)
Cost per assistant message (unique message.id) at list rates is split evenly over its tool_use
blocks; a message with no tool_use counts as "reply". Context = input + cache write + cache read.
"""
import glob, json, os, re, sys

RATES = {'opus': (4, 20, 8, 0.4), 'sonnet': (2, 10, 4, 0.2), 'haiku': (1, 5, 2, 0.1)}


def rate(model):
    for k, v in RATES.items():
        if k in (model or ''):
            return v
    return None


GIT_READ = re.compile(r'\bgit\b[^|;&]*\b(grep|show|diff|log|blame|ls-files)\b')
GIT_WRITE = re.compile(r'\bgit\b[^|;&]*\b(commit|rebase|merge|push|worktree|branch|cherry-pick|checkout|reset|add|fetch|stash|restore)\b')
CODE_PATH = re.compile(r'\.(rs|toml|py|md|json|yml|yaml|sh|js|conf|Dockerfile)\b|Dockerfile|/crates/|/src/|__roadmap__|__design__|docs/')
LOG_PATH = re.compile(r'\.log\b|\.out\b|\.txt\b|m5b-target|/tmp/|scratchpad|tasks/')


def bash_cat(cmd):
    c = cmd
    if re.search(r'\b(cargo|nextest)\b', c):
        return 'build/test'
    if re.search(r'\bdocker\b|lockf', c):
        return 'docker'
    if re.search(r'\bgh\b', c):
        return 'ci (gh)'
    if re.search(r'\b(sleep|until|while)\b', c) and not re.search(r'\bcargo\b', c):
        return 'wait/poll'
    if GIT_READ.search(c) and not GIT_WRITE.search(c):
        return 'read: git (grep/show/diff/log)'
    if GIT_WRITE.search(c) or re.search(r'\bcog\b|dirtree', c):
        return 'git/commit/roadmap tooling'
    if re.search(r'\b(sed -n|cat|head|tail|grep|rg|awk|less|wc|find|ls)\b', c):
        if LOG_PATH.search(c) and not CODE_PATH.search(c):
            return 'read: logs/outputs'
        if CODE_PATH.search(c):
            return 'read: code/docs via shell'
        return 'read: other shell'
    if re.search(r'python3?\b|perl\b', c):
        return 'edit/script'
    return 'shell: other'


def tool_cat(t):
    n = t.get('name') or ''
    inp = t.get('input') or {}
    if 'colgrep' in n:
        return 'read: colgrep (' + n.split('__')[-1] + ')'
    if n == 'Read':
        p = inp.get('file_path', '')
        return 'read: Read tool (code/docs)' if CODE_PATH.search(p) else 'read: Read tool (logs/notes)'
    if n in ('Grep', 'Glob'):
        return 'read: Grep/Glob tool'
    if n in ('Edit', 'Write', 'MultiEdit', 'NotebookEdit'):
        return 'write: Edit/Write'
    if n == 'Bash':
        return bash_cat(inp.get('command', ''))
    if n in ('Monitor', 'TaskOutput', 'ScheduleWakeup'):
        return 'wait/poll'
    if n in ('Agent', 'SendMessage', 'TaskStop') or 'send_message' in n:
        return 'coordinate: dispatch/message'
    if n in ('Skill', 'ToolSearch'):
        return 'setup: skill/tool load'
    if n.startswith('mcp__ccd'):
        return 'app: plan/wrap-up/session'
    return 'other: ' + n


def wake_kind(m):
    c = m.get('content')
    if isinstance(c, list):
        if any(isinstance(x, dict) and x.get('type') == 'tool_result' for x in c):
            return None
        text = ' '.join(x.get('text', '') for x in c if isinstance(x, dict))
    else:
        text = c or ''
    if '<task-notification>' in text:
        return 'subagent finished'
    if 'cross-session-message' in text or 'side-sessions-event' in text or 'finished a turn' in text:
        return 'session message'
    if 'system-reminder' in text and len(text) < 400:
        return None
    return 'human/other'


def scan(path):
    msgs = {}
    order = []
    a = {'msgs': 0, 'cost': 0.0, 'tool_calls': 0, 'wakes': {}, 'model': ''}
    for line in open(path):
        try:
            d = json.loads(line)
        except ValueError:
            continue
        m = d.get('message') or {}
        if not isinstance(m, dict):
            continue
        if d.get('type') == 'user' and m.get('role') == 'user':
            k = wake_kind(m)
            if k:
                a['wakes'][k] = a['wakes'].get(k, 0) + 1
            continue
        mid = m.get('id')
        if not mid or not m.get('usage'):
            continue
        if mid not in msgs:
            msgs[mid] = {'usage': m['usage'], 'model': m.get('model'), 'tools': []}
            order.append(mid)
        e = msgs[mid]
        e['usage'] = m['usage']
        e['tools'] += [c for c in (m.get('content') or []) if isinstance(c, dict) and c.get('type') == 'tool_use']
    calls = {}
    ctx_sum = 0
    for mid in order:
        e = msgs[mid]
        r = rate(e['model'])
        if not r:
            continue
        u = e['usage']
        x = (u.get('input_tokens', 0), u.get('output_tokens', 0),
             u.get('cache_creation_input_tokens', 0), u.get('cache_read_input_tokens', 0))
        cost = sum(v * rr / 1e6 for v, rr in zip(x, r))
        a['model'] = e['model']
        a['msgs'] += 1
        a['cost'] += cost
        ctx_sum += x[0] + x[2] + x[3]
        cats = [tool_cat(t) for t in e['tools']] or ['reply (no tool)']
        for k in cats:
            c = calls.setdefault(k, [0, 0.0])
            if k != 'reply (no tool)':
                c[0] += 1
                a['tool_calls'] += 1
            c[1] += cost / len(cats)
    a['avg_ctx_k'] = round(ctx_sum / max(a['msgs'], 1) / 1000)
    a['cost'] = round(a['cost'], 2)
    a['cats'] = {k: [v[0], round(v[1], 2)] for k, v in sorted(calls.items(), key=lambda kv: -kv[1][1])}
    return a


out = {}
for arg in sys.argv[1:]:
    label, base = arg.split('=', 1)
    main = scan(base + '.jsonl')
    main['role'] = 'coordinator'
    out[label + ' / coordinator'] = main
    for f in sorted(glob.glob(base + '/subagents/agent-*.jsonl')):
        meta = f[:-6] + '.meta.json'
        desc = ''
        if os.path.exists(meta):
            desc = json.load(open(meta)).get('description', '')
        s = scan(f)
        s['role'] = desc
        out[label + ' / ' + os.path.basename(f)[6:14]] = s
json.dump(out, sys.stdout, indent=0)
