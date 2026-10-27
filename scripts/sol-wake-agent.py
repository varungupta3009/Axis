#!/usr/bin/env python3
"""
Sol Homelab TV Wake Agent (192.168.50.100:8099)
Listens for LAN wake triggers from the LG OLED C2 (org.axis.native)
and launches Plex Media Server.app or Jellyfin.app in background (-g) mode.
"""

import json
import subprocess
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

ALLOWED_PREFIXES = ("192.168.50.", "127.0.0.1", "::1")


class ReusableHTTPServer(HTTPServer):
    allow_reuse_address = True


class WakeHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def _send_json(self, status: int, payload: dict):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        client_ip = self.client_address[0]
        if not client_ip.startswith(ALLOWED_PREFIXES):
            self.send_response(403)
            self.end_headers()
            return

        parsed = urlparse(self.path)
        if parsed.path == "/health":
            self._send_json(200, {"ok": True, "service": "sol-wake-agent"})
            return

        if parsed.path == "/wake":
            qs = parse_qs(parsed.query)
            svc = (qs.get("service") or ["plex"])[0].lower()
            if svc == "plex":
                subprocess.Popen(
                    ["/usr/bin/open", "-g", "-a", "Plex Media Server"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                self._send_json(200, {"ok": True, "started": "Plex Media Server"})
            elif svc in ("jellyfin", "metal"):
                subprocess.Popen(
                    ["/usr/bin/open", "-g", "-a", "Jellyfin"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                self._send_json(200, {"ok": True, "started": "Jellyfin"})
            else:
                self._send_json(400, {"ok": False, "error": "unknown service"})
            return

        self.send_response(404)
        self.end_headers()


if __name__ == "__main__":
    server = ReusableHTTPServer(("0.0.0.0", 8099), WakeHandler)
    server.serve_forever()
