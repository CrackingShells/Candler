#!/usr/bin/env python3
"""A scripted fake of the Anthropic Messages API (standard library, and the openssl CLI).

Environment:
    FAKE_TOKEN   required. The credential the gateway must forward, as Authorization bearer and
                 as x-api-key. A request that carries anything else is answered 401.
    FAKE_HOST    the name the certificate is for (default fake-anthropic.test).
    FAKE_PORT    the TLS port (default 8443).
    FAKE_DIR     where the certificates and the records go (default /tmp/fake).
    FAKE_OUT_TOKENS  output tokens each answer reports (default 20).

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

Any other model id (the harness's small model, for titles and the like) gets a short harmless
text. Every path other than /v1/messages is answered 200 with a small JSON body, and an unknown
one is logged as such.
"""

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
        ext.write("subjectAltName=DNS:" + HOST + "\nbasicConstraints=CA:FALSE\n"
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


def script_blocks(model, state):
    """The content blocks, the stop reason and a label of the answer for one request."""
    if model.startswith("c2"):
        return [{"type": "text", "text": "hello"}], "end_turn", "c2-hello"
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
            blocks, stop, label = script_blocks(model, state)
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
