"""A stdlib-only MCP client: launches the server the config names, lists its tools, and looks
for an evaluator-only file by name and by bytes.

usage: client.py CONFIG CANARY_NAME CANARY_BYTES
prints one JSON line: {"tools": [...], "canary": {"by_name": [...], "by_bytes": [...]}}
"""
import json
import os
import subprocess
import sys

config = json.load(open(sys.argv[1]))["mcpServers"]["tools"]
canary_name, canary_bytes = sys.argv[2], sys.argv[3].encode()

server = subprocess.Popen(
    [config["command"], *config["args"]],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
)


def call(ident, method, params=None):
    request = {"jsonrpc": "2.0", "method": method, "params": params or {}}
    if ident is not None:
        request["id"] = ident
    server.stdin.write(json.dumps(request) + "\n")
    server.stdin.flush()
    if ident is None:
        return None
    return json.loads(server.stdout.readline())["result"]


call(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "client", "version": "1"}})
call(None, "notifications/initialized")
tools = call(2, "tools/list")["tools"]
server.stdin.close()
server.wait()

by_name, by_bytes = [], []
for top, dirs, files in os.walk("/"):
    dirs[:] = [d for d in dirs if os.path.join(top, d) not in ("/proc", "/sys", "/dev")]
    for name in files:
        path = os.path.join(top, name)
        if name == canary_name:
            by_name.append(path)
        try:
            if os.path.islink(path) or not os.path.isfile(path) or os.path.getsize(path) > (1 << 20):
                continue
            if canary_bytes in open(path, "rb").read():
                by_bytes.append(path)
        except OSError:
            pass
print(json.dumps({"tools": tools, "canary": {"by_name": by_name, "by_bytes": by_bytes}}))
