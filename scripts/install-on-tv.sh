#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

APP_ID="org.axis.native"
TV_HOST="${TV_HOST:-root@192.168.50.107}"
SSH_KEY="${SSH_KEY:-../id_ed25519_lgc2}"
IPK_PATH="dist/${APP_ID}_1.0.0_arm.ipk"

./scripts/package-ipk.sh

scp -i "$SSH_KEY" "$IPK_PATH" "${TV_HOST}:/tmp/axis.ipk"
ssh -i "$SSH_KEY" "$TV_HOST" "
  killall axis 2>/dev/null || true
  luna-send -w 10000 -i luna://com.webos.appInstallService/dev/install '{\"id\":\"${APP_ID}\",\"ipkUrl\":\"/tmp/axis.ipk\",\"subscribe\":true}' | grep -m 1 '\"installed\"'
  rm -f /tmp/axis.ipk /tmp/home_pressed /var/palm/jail/${APP_ID}/tmp/home_pressed
  cp -f /media/developer/apps/usr/palm/applications/${APP_ID}/axis /var/palm/jail/${APP_ID}/media/developer/apps/usr/palm/applications/${APP_ID}/axis 2>/dev/null || true
"
