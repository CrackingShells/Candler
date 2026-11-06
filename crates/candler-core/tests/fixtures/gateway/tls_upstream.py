#!/usr/bin/env python3
"""A local TLS upstream for the gateway e2e tests (standard library only).

    tls_upstream.py CERT KEY TOKEN_FILE PORT RECORDS

Serves HTTPS on PORT with the certificate chain CERT and the key KEY. A request is authentic when
its Authorization is `Bearer <token>` and its x-api-key is `<token>`, where <token> is the first
line of TOKEN_FILE. Every request is appended to RECORDS as one JSON object: method, path, Host,
the SNI the client sent, both credential headers and the body.

Routes:
    GET  /v1/models    200 when authentic, else 401
    POST /v1/messages  200 with a canned message when authentic, else 401
    GET  /echo         200, the body repeats the Authorization header
    GET  /error-page   500, an HTML error page that repeats the Authorization header
    GET  /echo-forms   200, a JSON body with the credential escaped as JSON writes a slash, and in base64
    GET  /echo-headers 302, the credential repeated in X-Echo-Auth, Set-Cookie and Location
    GET  /slow         200 after three seconds

The certificate is read again for every connection: when /tmp/swap/ready exists, the chain and
key in /tmp/swap are served instead of CERT and KEY, so a test can change the server's identity.
"""

import base64
import json
import os
import ssl
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CERT, KEY, TOKEN_FILE, PORT, RECORDS = sys.argv[1:6]
with open(TOKEN_FILE, encoding="utf-8") as handle:
    TOKEN = handle.readline().strip()
LOCK = threading.Lock()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def reply(self, status, body, kind, extra=()):
        data = body.encode("utf-8")
        self.send_response(status)
        for name, value in extra:
            self.send_header(name, value)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def handle_one(self):
        size = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(size).decode("utf-8", "replace") if size else ""
        auth = self.headers.get("Authorization", "")
        key = self.headers.get("x-api-key", "")
        record = {
            "method": self.command,
            "path": self.path,
            "host": self.headers.get("Host", ""),
            "sni": getattr(self.request, "sni", None),
            "authorization": auth,
            "x_api_key": key,
            "body": body,
        }
        with LOCK:
            with open(RECORDS, "a", encoding="utf-8") as out:
                out.write(json.dumps(record) + "\n")
        authentic = auth == "Bearer " + TOKEN and key == TOKEN
        path = self.path.split("?", 1)[0]
        if path == "/echo":
            self.reply(200, "you sent Authorization: " + auth + "\n", "text/plain")
        elif path == "/error-page":
            page = "<html><body>bad request, Authorization was: " + auth + "</body></html>"
            self.reply(500, page, "text/html")
        elif path == "/echo-forms":
            forms = {
                "escaped": key.replace("/", "\\/"),
                "base64": base64.b64encode(key.encode("utf-8")).decode("ascii"),
            }
            # json.dumps would double the backslash, so the body is written by hand.
            body = '{"escaped":"' + forms["escaped"] + '","base64":"' + forms["base64"] + '"}'
            self.reply(200, body, "application/json")
        elif path == "/echo-headers":
            extra = [
                ("X-Echo-Auth", auth),
                ("Set-Cookie", "session=" + key + "; Path=/"),
                ("Location", "/x?t=" + key),
            ]
            self.reply(302, "moved\n", "text/plain", extra)
        elif path == "/slow":
            time.sleep(3)
            self.reply(200, "slow\n", "text/plain")
        elif not authentic:
            self.reply(401, json.dumps({"error": "invalid credentials"}), "application/json")
        elif path == "/v1/models":
            self.reply(200, json.dumps({"data": [{"id": "test-model"}]}), "application/json")
        elif path == "/v1/messages":
            message = {"id": "msg_1", "type": "message", "role": "assistant", "content": []}
            self.reply(200, json.dumps(message), "application/json")
        else:
            self.reply(404, json.dumps({"error": "not found"}), "application/json")

    do_GET = handle_one
    do_POST = handle_one


def remember_sni(sock, name, _context):
    sock.sni = name


def current_context():
    swapped = os.path.exists("/tmp/swap/ready")
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    if swapped:
        context.load_cert_chain("/tmp/swap/cert.pem", "/tmp/swap/key.pem")
    else:
        context.load_cert_chain(CERT, KEY)
    context.sni_callback = remember_sni
    return context


class Server(ThreadingHTTPServer):
    def get_request(self):
        sock, addr = self.socket.accept()
        return current_context().wrap_socket(sock, server_side=True), addr


Server(("0.0.0.0", int(PORT)), Handler).serve_forever()
