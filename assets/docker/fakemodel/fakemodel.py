#!/usr/bin/env python3
"""A scripted fake of the Anthropic Messages API (standard library, and the openssl CLI).

Environment:
    FAKE_TOKEN   required. The credential the gateway must forward, as Authorization bearer and
                 as x-api-key. A request that carries anything else is answered 401.
    FAKE_HOST    the name the certificate is for (default fake-anthropic.test).
    FAKE_PORT    the TLS port (default 8443).
    FAKE_DIR     where the certificates and the records go (default /tmp/fake).
    FAKE_OUT_TOKENS  output tokens each answer reports (default 20).
    FAKE_G1_COMMANDS  a JSON list of two or more shell commands, the script of model g1 (default: none).
    FAKE_DELAY_SECS  how long the answer to a c3 request is held, after it is recorded (default 0,
                 at most 120).

The server exits when the file FAKE_DIR/done exists, so that whoever started it can end it
with a `docker exec` and no destroy verb.

At start it makes a certificate authority and a server certificate for FAKE_HOST under FAKE_DIR.
The authority is FAKE_DIR/ca.pem: hand it to the gateway sidecar at /run/gw/ca.pem. Every request
is appended to FAKE_DIR/records.jsonl (method, path, model, whether the credential was right,
the body) and summarised on stdout as one JSON line.

The script is chosen by the model id of a /v1/messages request:

    c1  must wait. A first user prompt is answered with a Bash tool call that starts a 20 s job in
        the background (it writes /work/job.done, outside /tmp, which is noexec). The tool result
        is answered with text. A user message that carries a background job's completion is
        answered with a second text. Anything else is answered with text.
    c2  must stop. Every request is answered "hello".
    p1  must call a tool. A first user prompt is answered with one call to the tool `stamp` of the
        MCP server `parity`, under the name the request offers for it (`parity_stamp` for
        OpenCode). A request that defers its MCP tools (Claude Code, which offers ToolSearch) is
        answered with a ToolSearch for `mcp__parity__stamp` first, and its result with the call.
        Anything else is answered with text.
    g1  must run what it is told. A first user prompt is answered with a call to the shell tool
        (`Bash`, or `shell` for OpenCode) for the first command of FAKE_G1_COMMANDS, the result of
        each with the next command, and the result of the last with the call to the stamp tool,
        as for p1. Anything else is answered with text.
    h1  must run a shell command. A first user prompt is answered with one call to the shell tool
        (`Bash` for Claude Code, `shell` for OpenCode) that echoes `hook-not-blocked`, so that a
        hook which blocks the call shows as an error answer and none as that text. Anything else
        is answered with text.
    c3  must be slow. Every request is answered "hello" after FAKE_DELAY_SECS seconds, so that a
        run is live and its agent is mid-turn for that long.

Any other model id (the harness's small model, for titles and the like) gets a short harmless
text. Every path other than /v1/messages is answered 200 with a small JSON body, and an unknown
one is logged as such.
"""

import ipaddress
import json
import os
import ssl
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

TOKEN = os.environ["FAKE_TOKEN"]
HOST = os.environ.get("FAKE_HOST", "fake-anthropic.test")
PORT = int(os.environ.get("FAKE_PORT", "8443"))
DIR = os.environ.get("FAKE_DIR", "/tmp/fake")
OUT_TOKENS = int(os.environ.get("FAKE_OUT_TOKENS", "20"))
DELAY_SECS = max(0, min(int(os.environ.get("FAKE_DELAY_SECS", "0")), 120))
PARITY_TOOLS = ("mcp__parity__stamp", "parity_stamp")
JOB_COMMAND = "sleep 20 && date +%s%N > /work/job.done && echo job-finished"
LOCK = threading.Lock()
COUNTER = [0]


def make_certs():
    """Makes ca.pem, cert.pem and key.pem under DIR with the openssl CLI."""
    os.makedirs(DIR, exist_ok=True)

    def run(*args):
        subprocess.run(["openssl", *args], check=True, capture_output=True, cwd=DIR)

    run("ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", "ca.key")
    run("req", "-new", "-x509", "-key", "ca.key", "-out", "ca.pem", "-days", "2",
        "-subj", "/CN=fakemodel test authority",
        "-addext", "basicConstraints=critical,CA:TRUE",
        "-addext", "keyUsage=critical,keyCertSign,cRLSign")
    run("ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", "key.pem")
    run("req", "-new", "-key", "key.pem", "-out", "server.csr", "-subj", "/CN=" + HOST)
    with open(os.path.join(DIR, "ext.cnf"), "w", encoding="utf-8") as ext:
        try:
            ipaddress.ip_address(HOST)
            kind = "IP:"
        except ValueError:
            kind = "DNS:"
        ext.write("subjectAltName=" + kind + HOST + "\nbasicConstraints=CA:FALSE\n"
                  "keyUsage=digitalSignature\nextendedKeyUsage=serverAuth\n")
    run("x509", "-req", "-in", "server.csr", "-CA", "ca.pem", "-CAkey", "ca.key",
        "-CAcreateserial", "-out", "cert.pem", "-days", "2", "-extfile", "ext.cnf")


def text_of(content):
    """The text blocks of a message content, joined."""
    if isinstance(content, str):
        return content
    return "\n".join(b.get("text", "") for b in content if isinstance(b, dict) and b.get("type") == "text")


def last_user_state(messages):
    """prompt, tool_result or notification, from the last message that is not a system one."""
    turns = [m for m in messages if m.get("role") != "system"]
    if not turns or turns[-1].get("role") != "user":
        return "other"
    content = turns[-1].get("content")
    if isinstance(content, list) and any(
        isinstance(b, dict) and b.get("type") == "tool_result" for b in content
    ):
        return "tool_result"
    text = text_of(content)
    if "task-notification" in text or "<task-id>" in text or "<task_id>" in text:
        return "notification"
    return "prompt"


def stamp_call(name):
    """A call to the stamp tool under `name`."""
    return {"type": "tool_use", "id": "toolu_fake_stamp", "name": name, "input": {}}


def last_tool_result(messages):
    """The tool_use_id of the last tool result of the last user message, or None."""
    content = messages[-1].get("content") if messages else None
    ids = [b.get("tool_use_id") for b in content or []
           if isinstance(b, dict) and b.get("type") == "tool_result"] if isinstance(content, list) else []
    return ids[-1] if ids else None


SHELL_TOOLS = ("Bash", "shell")


def graded_blocks(state, tools, messages):
    """The answer of model g1: each shell command in turn, then the stamp call, then text."""
    commands = json.loads(os.environ.get("FAKE_G1_COMMANDS", "[]"))
    shell = next((name for name in tools if name in SHELL_TOOLS), None)
    last = last_tool_result(messages) if state == "tool_result" else None
    steps = [f"toolu_fake_g1_{index}" for index in range(len(commands))]

    def shell_call(index):
        return {"type": "tool_use", "id": steps[index], "name": shell,
                "input": {"command": commands[index]}}

    if shell and len(commands) >= 2:
        if state == "prompt":
            return [shell_call(0)], "tool_use", "g1-shell-0"
        if last in steps[:-1]:
            following = steps.index(last) + 1
            return [shell_call(following)], "tool_use", f"g1-shell-{following}"
        if last == steps[-1]:
            offered = [name for name in tools if name in PARITY_TOOLS]
            if offered:
                return [stamp_call(offered[0])], "tool_use", "g1-call"
            if "ToolSearch" in tools:
                search = {"type": "tool_use", "id": "toolu_fake_search", "name": "ToolSearch",
                          "input": {"query": "select:" + PARITY_TOOLS[0], "max_results": 1}}
                return [search], "tool_use", "g1-search"
        if last == "toolu_fake_search":
            return [stamp_call(PARITY_TOOLS[0])], "tool_use", "g1-call"
    return [{"type": "text", "text": "done"}], "end_turn", "g1-text"


def script_blocks(model, state, tools, messages):
    """The content blocks, the stop reason and a label of the answer for one request."""
    if model.startswith("g1"):
        return graded_blocks(state, tools, messages)
    if model.startswith("p1"):
        offered = [name for name in tools if name in PARITY_TOOLS]
        if state == "prompt" and offered:
            return [{"type": "text", "text": "I will call the stamp tool."},
                    stamp_call(offered[0])], "tool_use", "p1-call"
        if state == "prompt" and "ToolSearch" in tools:
            search = {"type": "tool_use", "id": "toolu_fake_search", "name": "ToolSearch",
                      "input": {"query": "select:" + PARITY_TOOLS[0], "max_results": 1}}
            return [{"type": "text", "text": "I will load the stamp tool."}, search], \
                "tool_use", "p1-search"
        if state == "tool_result" and last_tool_result(messages) == "toolu_fake_search":
            return [stamp_call(PARITY_TOOLS[0])], "tool_use", "p1-call"
        return [{"type": "text", "text": "done"}], "end_turn", "p1-text"
    if model.startswith("h1"):
        shell = [name for name in tools if name in ("Bash", "bash", "shell")]
        if state == "prompt" and shell:
            call = {"type": "tool_use", "id": "toolu_fake_hook", "name": shell[0],
                    "input": {"command": "echo hook-not-blocked", "description": "Echo"}}
            return [{"type": "text", "text": "I will run a command."}, call], "tool_use", "h1-call"
        return [{"type": "text", "text": "done"}], "end_turn", "h1-text"
    if model.startswith("c2"):
        return [{"type": "text", "text": "hello"}], "end_turn", "c2-hello"
    if model.startswith("c3"):
        return [{"type": "text", "text": "hello"}], "end_turn", "c3-slow"
    if model.startswith("c1"):
        if state == "prompt":
            tool = {"type": "tool_use", "id": "toolu_fake_job", "name": "Bash",
                    "input": {"command": JOB_COMMAND, "run_in_background": True,
                              "description": "Run a 20 s background job"}}
            return [{"type": "text", "text": "I will start a 20 s job in the background."}, tool], \
                "tool_use", "c1-start-job"
        if state == "notification":
            return [{"type": "text", "text": "The background job has finished."}], \
                "end_turn", "c1-after-notification"
        return [{"type": "text", "text": "The job is running; I will wait for it."}], \
            "end_turn", "c1-after-tool"
    return [{"type": "text", "text": "ok"}], "end_turn", "aux"


def message_json(model, blocks, stop):
    COUNTER[0] += 1
    return {"id": "msg_fake_%d" % COUNTER[0], "type": "message", "role": "assistant",
            "model": model, "content": blocks, "stop_reason": stop, "stop_sequence": None,
            "usage": {"input_tokens": 100, "output_tokens": OUT_TOKENS,
                      "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}}


def sse_events(message):
    """The SSE events of a streamed answer."""
    start = dict(message, content=[], stop_reason=None)
    start["usage"] = dict(message["usage"], output_tokens=1)
    events = [("message_start", {"type": "message_start", "message": start})]
    for index, block in enumerate(message["content"]):
        if block["type"] == "text":
            events.append(("content_block_start", {"type": "content_block_start", "index": index,
                           "content_block": {"type": "text", "text": ""}}))
            events.append(("content_block_delta", {"type": "content_block_delta", "index": index,
                           "delta": {"type": "text_delta", "text": block["text"]}}))
        else:
            events.append(("content_block_start", {"type": "content_block_start", "index": index,
                           "content_block": {"type": "tool_use", "id": block["id"],
                                             "name": block["name"], "input": {}}}))
            events.append(("content_block_delta", {"type": "content_block_delta", "index": index,
                           "delta": {"type": "input_json_delta",
                                     "partial_json": json.dumps(block["input"])}}))
        events.append(("content_block_stop", {"type": "content_block_stop", "index": index}))
    events.append(("message_delta", {"type": "message_delta",
                   "delta": {"stop_reason": message["stop_reason"], "stop_sequence": None},
                   "usage": {"output_tokens": message["usage"]["output_tokens"]}}))
    events.append(("message_stop", {"type": "message_stop"}))
    return events


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def send_json(self, status, value):
        data = json.dumps(value).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def send_stream(self, events):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        for name, payload in events:
            chunk = ("event: %s\ndata: %s\n\n" % (name, json.dumps(payload))).encode("utf-8")
            self.wfile.write(b"%x\r\n%s\r\n" % (len(chunk), chunk))
            self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()

    def handle_any(self):
        size = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(size).decode("utf-8", "replace") if size else ""
        try:
            body = json.loads(raw) if raw else None
        except ValueError:
            body = None
        auth = self.headers.get("Authorization", "")
        key = self.headers.get("x-api-key", "")
        authentic = auth == "Bearer " + TOKEN and key == TOKEN
        path = self.path.split("?", 1)[0]
        model = body.get("model", "") if isinstance(body, dict) else ""
        stream = bool(body.get("stream")) if isinstance(body, dict) else False
        messages = body.get("messages", []) if isinstance(body, dict) else []
        decision = "unknown-path"
        status = 200
        if not authentic:
            decision, status = "unauthorised", 401
        elif path == "/v1/messages" and self.command == "POST" and isinstance(body, dict):
            state = last_user_state(messages)
            tools = [t.get("name", "") for t in body.get("tools") or [] if isinstance(t, dict)]
            blocks, stop, label = script_blocks(model, state, tools, messages)
            decision = "%s/%s" % (label, state)
            message = message_json(model, blocks, stop)
        elif path == "/v1/messages/count_tokens":
            decision = "count_tokens"
        elif path == "/v1/models" or path.startswith("/v1/models/"):
            decision = "models"
        record = {"ts_ns": time.time_ns(), "method": self.command, "uri": self.path,
                  "model": model, "stream": stream, "authentic": authentic,
                  "messages": len(messages), "decision": decision, "body": body}
        with LOCK:
            with open(os.path.join(DIR, "records.jsonl"), "a", encoding="utf-8") as out:
                out.write(json.dumps(record) + "\n")
            print(json.dumps({k: v for k, v in record.items() if k != "body"}), flush=True)
        if decision.startswith("c3-slow"):
            time.sleep(DELAY_SECS)
        if status == 401:
            self.send_json(401, {"type": "error", "error": {"type": "authentication_error",
                                                            "message": "invalid credentials"}})
        elif decision.startswith("unknown"):
            self.send_json(200, {})
        elif decision == "count_tokens":
            self.send_json(200, {"input_tokens": 10})
        elif decision == "models":
            self.send_json(200, {"data": [{"id": "c1", "type": "model"}, {"id": "c2", "type": "model"}],
                                 "has_more": False})
        elif stream:
            self.send_stream(sse_events(message))
        else:
            self.send_json(200, message)

    do_GET = handle_any
    do_POST = handle_any
    do_HEAD = handle_any


def exit_on_done_file():
    """Exits the process once FAKE_DIR/done exists."""
    done = os.path.join(DIR, "done")
    while not os.path.exists(done):
        time.sleep(0.2)
    os._exit(0)


def main():
    make_certs()
    threading.Thread(target=exit_on_done_file, daemon=True).start()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(os.path.join(DIR, "cert.pem"), os.path.join(DIR, "key.pem"))
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print(json.dumps({"event": "ready", "host": HOST, "port": PORT}), flush=True)
    server.serve_forever()


if __name__ == "__main__":
    sys.exit(main())
