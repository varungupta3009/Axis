# Axis: Native webOS Home

Axis is a native spatial home screen for LG webOS TVs written in **Rust**, **SDL2**, and **OpenGL ES 2.0**.

It removes ads, recommendation rows, sponsored banners, and web runtime overhead. It shows your apps, HDMI inputs, a clock, and local weather at a locked **60 FPS** with **~10.4 MB RSS** on an OLED pitch-black canvas.

> Tested on **LG OLED TVs running webOS 22, 23, 24, and 25 (Rockhopper 7.x, 8.x, 9.x, and 10.3.1)**.
> Inspired by the spatial layout of [`webos-custom-home`](https://github.com/zzeppieri/webos-custom-home) by [@zzeppieri](https://github.com/zzeppieri).

---

## Why Native?

The original `webos-custom-home` proved that a 4-directional spatial launcher fits TV remote navigation well. However, LG Web Application Manager (Chromium 108) uses significant RAM and CPU on embedded TV hardware.

Axis replaces the Chromium runtime with a compiled ARMv7 binary:

| Metric | Web App (WAM / Chromium) | Axis (Rust + GLES 2.0) | Improvement |
|---|:---:|:---:|:---:|
| **Memory Footprint (RSS)** | ~156 MB | **~10.4 MB** | **>14x lighter** |
| **Idle CPU / GPU** | 1.8% to 4.2% | **0.0%** (kernel event sleep) | **Zero idle load** |
| **Frame Rate** | 45 to 55 FPS | **Locked 60 FPS** (VSync) | **Consistent** |
| **Input Latency** | 60 to 120 ms | **< 16 ms** (direct SDL2 scan) | **Immediate** |
| **Package Size** | ~18 MB (JS bundles + assets) | **~673 KB** binary (~1.0 MB `.ipk`) | **>17x smaller** |

---

## Key Capabilities

- Uses a 4-directional spatial layout: **Up** (Gaming and Inputs), **Down** (Streaming), **Left** (Media and Music), and **Right** (Utilities).
- Protects OLED panels with a `#000000` canvas (0.000 nits), `±2px` pixel jitter every 150 seconds, and full compatibility with native webOS `screenmanager` screensavers (`SDL_VIDEO_ALLOW_SCREENSAVER=1`).
- Includes a dedicated **OLED Screen Off** tile that turns off the panel immediately through `tvpowerd` while background audio or tasks continue.
- Animates screen transitions and tile scrolling with critically damped spring physics and custom GLSL fragment shaders.
- Shows an analog and digital clock (12h or 24h), live weather from [Open-Meteo](https://open-meteo.com/) without an API key, and an optional **System Telemetry HUD**.
- Supports tile reordering on a long-press of **OK**, an on-screen settings modal, automatic cleanup of uninstalled apps, and automatic icon extraction for newly installed apps.

---

## Navigation and Controls

| Remote Button | Action |
|---|---|
| **Arrow Keys (↑ / ↓ / ← / →)** | Move between categories or select tiles |
| **OK (Short Press)** | Launch the selected app or switch to the selected HDMI input |
| **OK (Long Press)** | Enter tile reorder mode (arrow keys move the tile, OK saves) |
| **Back Button** | Return to the center Home screen or cancel reorder mode |
| **Home Button** | Return immediately to the center Home screen |

---

## Building from Source

Axis cross-compiles to ARMv7 Linux (`armv7-unknown-linux-gnueabi`) with [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild). This repository includes symbol stubs in `sysroot/lib/` for webOS system libraries (`libSDL2`, `libSDL2_image`, `libSDL2_ttf`, `libGLESv2`, `libEGL`, and `libluna-service2`).

### Prerequisites

1. Install [Rust](https://rustup.rs/) and add the ARMv7 target:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add armv7-unknown-linux-gnueabi
   ```

2. Install Zig and `cargo-zigbuild`:
   ```bash
   brew install zig
   cargo install cargo-zigbuild
   ```

### Compile the Release Binary

Compile the stripped ARMv7 binary:

```bash
cargo zigbuild --release --target armv7-unknown-linux-gnueabi.2.28
```

The output binary path is `target/armv7-unknown-linux-gnueabi/release/axis`.

---

## Packaging and Installation

For a full walkthrough from a stock TV to boot and Home-button integration, read [docs/TUTORIAL.md](docs/TUTORIAL.md).

### 1. Build the webOS Package (`.ipk`)

Build the `.ipk` archive:

```bash
./scripts/package-ipk.sh
```

This script compiles the binary and writes `dist/org.axis.native_1.0.0_arm.ipk`.

### 2. Install on the TV

If your TV has root SSH enabled through Homebrew Channel, run the installer script:

```bash
TV_HOST="root@192.168.1.100" SSH_KEY="$HOME/.ssh/id_ed25519" ./scripts/install-on-tv.sh
```

Alternatively, install the package with `@webos-tools/cli`:

```bash
ares-install --device tv dist/org.axis.native_1.0.0_arm.ipk
```

---

## Configuration

Axis reads its configuration file on the TV at `/home/root/.config/axis/config.json` (inside the webOS app jail at `/var/palm/jail/org.axis.native/home/root/.config/axis/config.json`). On first launch, Axis creates this file from `assets/config.default.json`.

```json
{
  "version": 1,
  "clock24": false,
  "temp_unit": "fahrenheit",
  "location_name": "San Francisco",
  "latitude": 37.7749,
  "longitude": -122.4194,
  "bg_animated": true,
  "card_animations": true,
  "bg_theme": "classic",
  "bg_density": "normal",
  "bg_speed": "normal",
  "show_telemetry": false
}
```

You can change visual preferences, clock format, temperature units, font family, and hidden apps from the on-screen **Launcher Settings** modal. Edit `location_name`, `latitude`, and `longitude` in `config.json` (or `assets/config.default.json` before packaging) to set your weather location.

---

## Credits

- Original spatial launcher concept and web version by [@zzeppieri](https://github.com/zzeppieri) in [`webos-custom-home`](https://github.com/zzeppieri/webos-custom-home).
- Native Rust, SDL2, and OpenGL ES 2.0 implementation by [Varun Gupta](https://github.com/varungupta3009).
- Weather data from [Open-Meteo](https://open-meteo.com/).
- Root tooling from the [webOS Homebrew Project](https://github.com/webosbrew).

---

## License

This project uses the [MIT License](LICENSE).
