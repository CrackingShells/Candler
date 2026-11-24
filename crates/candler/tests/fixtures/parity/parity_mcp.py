"""A stdio MCP server with one tool, `stamp`, written with the standard library alone."""
import json
import sys

TOOLS = [{
    "name": "stamp",
    "description": "Returns the parity stamp, a fixed text. Takes no argument.",
    "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
}]
STAMP = "parity-stamp-5c1e"


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    method, msg_id = msg.get("method"), msg.get("id")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": msg_id, "result": {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-03-26"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "parity", "version": "1.0.0"},
        }})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": msg_id, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        send({"jsonrpc": "2.0", "id": msg_id,
              "result": {"content": [{"type": "text", "text": STAMP}], "isError": False}})
    elif msg_id is not None:
        send({"jsonrpc": "2.0", "id": msg_id,
              "error": {"code": -32601, "message": "unknown method"}})
