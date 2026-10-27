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


class WakeHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def do_GET(self):
        client_ip = self.client_address[0]
        if not client_ip.startswith(ALLOWED_PREFIXES):
            self.send_response(403)
            self.end_headers()
            return

        parsed = urlparse(self.path)
        if parsed.path == "/wake":
            qs = parse_qs(parsed.query)
            svc = (qs.get("service") or ["plex"])[0].lower()
            if svc == "plex":
                subprocess.Popen(
                    ["/usr/bin/open", "-g", "-a", "Plex Media Server"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                msg = {"ok": True, "started": "Plex Media Server"}
            elif svc in ("jellyfin", "metal"):
                subprocess.Popen(
                    ["/usr/bin/open", "-g", "-a", "Jellyfin"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                msg = {"ok": True, "started": "Jellyfin"}
            else:
                msg = {"ok": False, "error": "unknown service"}

            body = json.dumps(msg).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        self.send_response(404)
        self.end_headers()


if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", 8099), WakeHandler)
    server.serve_forever()
