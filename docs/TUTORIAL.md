# Tutorial: Install and Configure Axis on an LG webOS TV

This guide explains how to compile, install, and configure Axis (`org.axis.native`) on an LG webOS TV. Every change stays inside user-writable paths (`/media/developer` and `/var/lib/webosbrew`), and you can revert to the stock LG Home screen at any time.

## Contents

1. [Choose Developer Mode or Root](#1-choose-developer-mode-or-root)
2. [Prepare Access on the TV](#2-prepare-access-on-the-tv)
3. [Install Build Tools on Your Computer](#3-install-build-tools-on-your-computer)
4. [Build and Package Axis](#4-build-and-package-axis)
5. [Deploy Axis to the TV](#5-deploy-axis-to-the-tv)
6. [Customize Weather, Apps, and Icons](#6-customize-weather-apps-and-icons)
7. [Configure Boot and Home Button Takeover (Root Only)](#7-configure-boot-and-home-button-takeover-root-only)
8. [Uninstall and Revert to Stock](#8-uninstall-and-revert-to-stock)

---

## 1. Choose Developer Mode or Root

| Feature | LG Developer Mode | Root (Homebrew Channel) |
|---|---|---|
| Sideload and launch `org.axis.native` | Yes | Yes |
| Automatic app discovery and icon staging | Requires `hbchannel` exec | Yes |
| Intercept the physical Home button | No | Yes (`lginputhook` + watcher) |
| Boot directly into Axis | No | Yes (`init.d` hook) |

Developer Mode lets you sideload and open Axis like a normal TV app. Root access with the [webOS Homebrew Channel](https://github.com/webosbrew/webos-homebrew-channel) lets Axis replace the stock LG Home screen on boot and on every Home button press.

---

## 2. Prepare Access on the TV

### Root Path (Recommended)

1. Root your TV and install the [webOS Homebrew Channel](https://github.com/webosbrew/webos-homebrew-channel) using the method supported for your webOS firmware.
2. Open **Homebrew Channel** on the TV and enable **Root SSH** and **Startup Scripts (`init.d`)**.
3. Assign a static DHCP reservation to the TV on your router.
4. Copy your SSH public key to the TV:
   ```bash
   ssh-copy-id root@<TV_IP>
   ```
5. Test SSH login from your computer:
   ```bash
   ssh root@<TV_IP> "uname -a"
   ```

### Developer Mode Path

1. Install **Developer Mode** from the LG Content Store and sign in with an LG Developer account.
2. Enable **Dev Mode Status** and **Key Server** inside the Developer Mode app.
3. Install `@webos-tools/cli` (`npm install -g @webos-tools/cli`) and register your TV with `ares-setup-device`.

---

## 3. Install Build Tools on Your Computer

1. Install Rust and add the 32-bit ARMv7 target:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add armv7-unknown-linux-gnueabi
   ```
2. Install Zig, `cargo-zigbuild`, and Python 3:
   ```bash
   brew install zig python3
   cargo install cargo-zigbuild
   ```

---

## 4. Build and Package Axis

1. Clone the repository:
   ```bash
   git clone https://github.com/varungupta3009/Axis.git
   cd Axis
   ```
2. Build the `.ipk` package:
   ```bash
   ./scripts/package-ipk.sh
   ```
   This command compiles `target/armv7-unknown-linux-gnueabi/release/axis` and creates `dist/org.axis.native_1.0.0_arm.ipk`.

---

## 5. Deploy Axis to the TV

### Deploy Over Root SSH

Run the bundled installer script:

```bash
TV_HOST="root@<TV_IP>" SSH_KEY="$HOME/.ssh/id_ed25519" ./scripts/install-on-tv.sh
```

Launch Axis on the TV to test it:

```bash
ssh root@<TV_IP> "luna-send -n 1 luna://com.webos.applicationManager/launch '{\"id\":\"org.axis.native\"}'"
```

### Deploy With `ares-install` (Developer Mode)

Install and launch the package with the webOS CLI:

```bash
ares-install --device tv dist/org.axis.native_1.0.0_arm.ipk
ares-launch --device tv org.axis.native
```

---

## 6. Customize Weather, Apps, and Icons

On first launch, Axis writes its configuration file inside the app jail at:

```text
/var/palm/jail/org.axis.native/home/root/.config/axis/config.json
```

### Weather Location

1. Open the configuration file over SSH (or edit `assets/config.default.json` before you build the `.ipk`).
2. Update `location_name`, `latitude`, `longitude`, and `temp_unit` (`"celsius"` or `"fahrenheit"`).
3. Restart Axis or reboot the TV to load the updated coordinates.

### App Categories and Icons

- Press and hold **OK** on any tile for 550 ms to reorder tiles within a category or move a tile to another direction.
- Open **Launcher Settings** (in the **Right** category) to toggle 12h/24h time, constellation themes, motion speed, font family, or hidden apps.
- On rooted TVs, Axis queries `luna://com.webos.applicationManager/listLaunchPoints` at startup, removes uninstalled apps, and stages icons for newly installed apps into `/tmp/axis_icons/`.
- To bundle custom PNG icons, put them in `assets/icons/`, reference `"assets/icons/<name>.png"` in `assets/config.default.json`, and run `./scripts/package-ipk.sh` again.

---

## 7. Configure Boot and Home Button Takeover (Root Only)

On webOS, pressing the Home button or closing a foreground app activates `com.webos.app.home`. You can redirect this flow to `org.axis.native` without modifying read-only system partitions.

1. Create `/var/lib/webosbrew/init.d/60-axis-launcher` on the TV with a power-state gated foreground watcher:
   ```sh
   #!/bin/sh
   (
     sleep 4
     luna-send -n 1 luna://com.webos.applicationManager/launch '{"id":"org.axis.native"}' >/dev/null 2>&1
     luna-send -i luna://com.webos.applicationManager/getForegroundAppInfo '{"subscribe":true}' 2>/dev/null | \
     while IFS= read -r line; do
       case "$line" in
         *'"appId":"com.webos.app.home"'*)
           PSTATE=$(luna-send -n 1 luna://com.webos.service.tvpower/power/getPowerState '{}' 2>/dev/null)
           if echo "$PSTATE" | grep -Eq '"state"[[:space:]]*:[[:space:]]*"Active"' && ! echo "$PSTATE" | grep -q '"processing"'; then
             luna-send -n 1 luna://com.webos.applicationManager/launch '{"id":"org.axis.native"}' >/dev/null 2>&1
           fi
           ;;
       esac
     done
   ) &
   ```
   > **Important:** Always check `getPowerState` for `"state":"Active"` before calling `applicationManager/launch`. Without that check, background app exits during `"Screen Saver"` or `"Active Standby"` will wake the display.

2. Make the startup script executable:
   ```bash
   ssh root@<TV_IP> "chmod +x /var/lib/webosbrew/init.d/60-axis-launcher"
   ```

3. *(Optional)* For instant `<35 ms` Home-button return when Axis is already open on an outer category screen, install [`org.webosbrew.inputhook`](https://github.com/webosbrew/inputhook) and bind the Home key (`773`, and `987` on newer remotes) to write `/proc/uptime` to `/tmp/axis_home_seq` (`chmod 644 /tmp/axis_home_seq`) and launch `org.axis.native` when `/tmp/axis_visible` is not `"1"`.

---

## 8. Uninstall and Revert to Stock

1. Remove the startup script (and any `lginputhook` Home keybind if configured):
   ```bash
   ssh root@<TV_IP> "rm -f /var/lib/webosbrew/init.d/60-axis-launcher && pkill -f getForegroundAppInfo || true"
   ```
2. Uninstall `org.axis.native`:
   ```bash
   ssh root@<TV_IP> "luna-send -n 1 luna://com.webos.appInstallService/dev/remove '{\"id\":\"org.axis.native\"}'"
   ```
3. Reboot the TV to return to the stock LG Home screen.
