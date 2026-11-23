"""A stdio MCP server with one tool, `audit_stamp`, written with the standard library alone."""
import json
import sys

TOOLS = [{
    "name": "audit_stamp",
    "description": "Returns the audit stamp, a fixed text. Takes no argument.",
    "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
}]
STAMP = "stamp-7f3a"


def reply(msg_id, result):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg_id, "result": result}) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    method, msg_id = msg.get("method"), msg.get("id")
    if method == "initialize":
        reply(msg_id, {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-03-26"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "audit", "version": "1.0.0"},
        })
    elif method == "tools/list":
        reply(msg_id, {"tools": TOOLS})
    elif method == "tools/call":
        reply(msg_id, {"content": [{"type": "text", "text": STAMP}], "isError": False})
    elif msg_id is not None:
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": msg_id,
                                     "error": {"code": -32601, "message": "unknown method"}}) + "\n")
        sys.stdout.flush()
