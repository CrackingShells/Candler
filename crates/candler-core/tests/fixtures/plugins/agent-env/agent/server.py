"""A stdlib-only MCP server over stdio (newline-delimited JSON-RPC) that offers two tools."""
import json
import sys

import tinydep

TOOLS = [
    {
        "name": "echo",
        "description": "Returns its text. " + tinydep.TAG,
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        },
    },
    {
        "name": "add",
        "description": "Adds two integers.",
        "inputSchema": {
            "type": "object",
            "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}},
            "required": ["a", "b"],
        },
    },
]


def reply(ident, result):
    sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": ident, "result": result}) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method == "initialize":
        reply(
            message["id"],
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "tiny", "version": "1.0.0"},
            },
        )
    elif method == "tools/list":
        reply(message["id"], {"tools": TOOLS})
