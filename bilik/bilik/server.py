"""Server UI lokal Bilik (stdlib, tanpa dependency)."""
import os
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from . import store, keys

UI_DIR = os.path.join(os.path.dirname(__file__), "ui")
HOOKS = {"open_url": None}   # diisi app.py


def public(c):
    if not c:
        return None
    return {k: c.get(k) for k in
            ("id", "filename", "startURL", "host", "cmid", "configKey", "importedAt")}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json"):
        if isinstance(body, (dict, list)):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split("?")[0]
        if path in ("/", "/index.html"):
            with open(os.path.join(UI_DIR, "index.html"), "rb") as f:
                return self._send(200, f.read(), "text/html; charset=utf-8")
        if path == "/api/state":
            return self._send(200, {"ok": True, "active": public(store.active()),
                                    "configs": [public(c) for c in store.all_configs()]})
        return self._send(404, {"ok": False, "error": "not found"})

    def do_POST(self):
        path = self.path.split("?")[0]
        n = int(self.headers.get("Content-Length") or 0)
        try:
            payload = json.loads(self.rfile.read(n) or b"{}")
        except Exception:
            return self._send(400, {"ok": False, "error": "body bukan JSON"})

        if path == "/api/config":
            content = payload.get("content") or ""
            filename = payload.get("filename") or "config.seb"
            if not content:
                return self._send(400, {"ok": False, "error": "content kosong"})
            try:
                meta = keys.load_seb(content.encode(), filename)
            except keys.ConfigError as e:
                return self._send(400, {"ok": False, "error": str(e)})
            entry = store.add(meta)
            if HOOKS["open_url"]:
                HOOKS["open_url"](entry["startURL"])
            return self._send(200, {"ok": True, "config": public(entry)})

        if path == "/api/activate":
            c = store.activate(payload.get("id") or "")
            if not c:
                return self._send(404, {"ok": False, "error": "config tidak ditemukan"})
            if HOOKS["open_url"]:
                HOOKS["open_url"](c["startURL"])
            return self._send(200, {"ok": True, "config": public(c)})

        if path == "/api/delete":
            store.delete(payload.get("id") or "")
            return self._send(200, {"ok": True, "active": public(store.active())})

        return self._send(404, {"ok": False, "error": "not found"})


def start(port=8765):
    srv = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    t = threading.Thread(target=srv.serve_forever, daemon=True)
    t.start()
    return srv