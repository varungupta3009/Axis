#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

APP_ID="org.axis.native"
VERSION="1.0.0"
ARCH="arm"
BIN_PATH="target/armv7-unknown-linux-gnueabi/release/axis"
OUT_IPK="dist/${APP_ID}_${VERSION}_${ARCH}.ipk"

echo "Building $BIN_PATH via cargo-zigbuild..."
cargo zigbuild --release --target armv7-unknown-linux-gnueabi.2.28

python3 -m json.tool appinfo.json >/dev/null
python3 -m json.tool assets/config.default.json >/dev/null

mkdir -p dist
python3 - "$APP_ID" "$VERSION" "$ARCH" "$BIN_PATH" "$OUT_IPK" <<'PY'
import io
import os
import stat
import sys
import tarfile
import time

app_id, version, arch, bin_path, out_ipk = sys.argv[1:6]
mtime = int(time.time())

def make_tar_gz(add_fn):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz", format=tarfile.USTAR_FORMAT) as tf:
        add_fn(tf)
    return buf.getvalue()

def add_bytes(tf, name, data, mode=0o644):
    ti = tarfile.TarInfo(name=name)
    ti.size = len(data)
    ti.mtime = mtime
    ti.mode = mode
    ti.uid = 0
    ti.gid = 0
    ti.uname = "root"
    ti.gname = "root"
    tf.addfile(ti, io.BytesIO(data))

def add_dir(tf, name):
    ti = tarfile.TarInfo(name=name)
    ti.type = tarfile.DIRTYPE
    ti.mtime = mtime
    ti.mode = 0o755
    ti.uid = 0
    ti.gid = 0
    ti.uname = "root"
    ti.gname = "root"
    tf.addfile(ti)

def build_control(tf):
    ctrl = (
        f"Package: {app_id}\n"
        f"Version: {version}\n"
        f"Section: misc\n"
        f"Priority: optional\n"
        f"Architecture: {arch}\n"
        f"Installed-Size: 2048\n"
        f"Maintainer: Varun Gupta <varungupta3009@gmail.com>\n"
        f"Description: High-performance native Rust + SDL2 + OpenGL ES 2.0 spatial launcher for LG webOS TVs\n"
        f"webOS-Package-Format-Version: 2\n"
        f"webOS-Packager-Version: 1.0.0\n"
    ).encode("utf-8")
    add_bytes(tf, "./control", ctrl, 0o644)

def build_data(tf):
    base = f"usr/palm/applications/{app_id}"
    for d in ["usr", "usr/palm", "usr/palm/applications", base, f"{base}/assets", f"{base}/assets/icons", f"{base}/assets/fonts"]:
        add_dir(tf, d)
    with open("appinfo.json", "rb") as f:
        add_bytes(tf, f"{base}/appinfo.json", f.read(), 0o644)
    with open(bin_path, "rb") as f:
        add_bytes(tf, f"{base}/axis", f.read(), 0o755)
    for fname in ["icon.png", "largeIcon.png", "config.default.json"]:
        with open(os.path.join("assets", fname), "rb") as f:
            add_bytes(tf, f"{base}/assets/{fname}", f.read(), 0o644)
    for fname in sorted(os.listdir("assets/icons")):
        p = os.path.join("assets/icons", fname)
        if os.path.isfile(p):
            with open(p, "rb") as f:
                add_bytes(tf, f"{base}/assets/icons/{fname}", f.read(), 0o644)
    for fname in sorted(os.listdir("assets/fonts")):
        p = os.path.join("assets/fonts", fname)
        if os.path.isfile(p):
            with open(p, "rb") as f:
                add_bytes(tf, f"{base}/assets/fonts/{fname}", f.read(), 0o644)

control_gz = make_tar_gz(build_control)
data_gz = make_tar_gz(build_data)

def ar_member(name, data):
    hdr = f"{name:<16}{mtime:<12}{0:<6}{0:<6}{0o100644:<8o}{len(data):<10}`\n".encode("ascii")
    pad = b"\n" if (len(data) % 2 == 1) else b""
    return hdr + data + pad

with open(out_ipk, "wb") as out:
    out.write(b"!<arch>\n")
    out.write(ar_member("debian-binary", b"2.0\n"))
    out.write(ar_member("control.tar.gz", control_gz))
    out.write(ar_member("data.tar.gz", data_gz))

print(f"Packaged {out_ipk} ({os.path.getsize(out_ipk)} bytes)")
PY
