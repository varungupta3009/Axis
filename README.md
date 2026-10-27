# Axis: Native webOS Home

A high-performance native spatial home screen for LG webOS TVs written in **Rust**, **SDL2**, and **OpenGL ES 2.0**.

No ads, no recommendation clutter, no sponsored rows, and no web browser bloat. Just your apps, inputs, a clock, and live weather rendered at a locked **60 FPS** with **~10 MB RSS** on an OLED-friendly pitch-black canvas.

![Axis Native Home — greeting, clock, animated constellations, and weather widget](docs/main-page.png)

<p align="center">
  <img src="docs/home-animation.gif" width="49%" alt="Animated constellation background on the home screen">
  <img src="docs/navigation.gif" width="49%" alt="Navigating between screens with the remote">
</p>

> Proven on **LG OLED TVs (webOS 22 / 23 / 24, Rockhopper 9.2.1 / 8.x / 7.x)**.
> Based on the spatial layout of [`webos-custom-home`](https://github.com/zzeppieri/webos-custom-home) by [@zzeppieri](https://github.com/zzeppieri).

---

## Why Native?

The original `webos-custom-home` proved that a 4-directional spatial launcher is the ideal TV interface. However, running inside LG's Web Application Manager (Chromium 108 webview) carries significant overhead on embedded TV hardware.

Axis replaces the entire web runtime with a compiled native ARM binary:

| Metric | Web App (WAM / Chromium) | Axis (Rust + GLES 2.0) | Improvement |
|---|:---:|:---:|:---:|
| **Memory Footprint (RSS)** | ~156 MB | **~10.4 MB** | **>14x lighter** |
| **Idle CPU / GPU** | 1.8% – 4.2% | **0.0%** (kernel event sleep) | **Zero waste** |
| **Frame Rate** | 45 – 55 FPS (micro-stutter) | **Locked 60 FPS** (VSync) | **Rock solid** |
| **Input Latency** | 60 – 120 ms | **< 16 ms** (direct SDL2 scan) | **Instant** |
| **Binary Size** | ~18 MB (JS bundles + assets) | **~1.7 MB** (stripped ARMv7 binary) | **>10x smaller** |

---

## Features

- **4-Directional Spatial Hub**:
  - **Up ↑**: Gaming & Inputs (HDMI 1, HDMI 2, GeForce NOW, Steam)
  - **Down ↓**: Streaming Services (Netflix, Prime Video, Disney+, Apple TV, HBO Max)
  - **Left ←**: Media & Music (YouTube, Spotify, Twitch, Media Player)
  - **Right →**: Utilities & Tools (OLED Screen Off, TV Settings, Browser, Homebrew)
- **OLED Burn-in Protection**:
  - **Pure Pitch Black (#000000)**: Consumes 0.000 nits on OLED panels.
  - **Subpixel Jitter**: Micro-shifts static UI elements every 60 seconds to prevent pixel wear.
  - **Instant Screen Off**: Dedicated tile triggers deep hardware panel standby via `tvpowerd` without suspending running background tasks.
- **Built-in Spring Physics & Shaders**:
  - Critically damped spring simulation for screen transitions and tile movement.
  - Custom GLSL fragment shaders for analytical anti-aliased rounded rectangles, outer glow drops, and dynamic starfield constellations.
- **Widgets**:
  - Analog and digital clock with 12h/24h toggle.
  - Live local weather forecasts powered by [Open-Meteo](https://open-meteo.com/) (no API key required).
  - Optional **System Telemetry HUD** displaying real-time memory usage (RSS), display mode, and frame timing.
- **Interactive Reordering & App Discovery**:
  - Long-press **OK** on any tile to enter interactive reorder mode.
  - Built-in Settings menu to reorder categories, toggle animations, select starfield themes, or customize coordinates.
  - Automatically discovers newly installed apps from webOS `applicationManager`.

---

## Navigation & Controls

| Remote Button | Action |
|---|---|
| **Arrow Keys (↑ / ↓ / ← / →)** | Navigate across categories or select tiles |
| **OK (Short Press)** | Launch selected app or switch to selected HDMI input |
| **OK (Long Press / Hold)** | Enter tile reorder mode (arrows move tile, OK saves) |
| **Back Button** | Return to center Home screen |
| **Home Button** | Immediately returns to center Home screen from anywhere |

---

## Building from Source

Axis cross-compiles to ARMv7 Linux (`armv7-unknown-linux-gnueabi`) using [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild). The repository includes pre-configured sysroot stubs for webOS system libraries (`libSDL2`, `libGLESv2`, `libEGL`, `libluna-service2`).

### Prerequisites

1. Install [Rust](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add armv7-unknown-linux-gnueabi
   ```

2. Install Zig and `cargo-zigbuild`:
   ```bash
   brew install zig # or your package manager
   cargo install cargo-zigbuild
   ```

### Compile Release Binary

```bash
cargo zigbuild --release --target armv7-unknown-linux-gnueabi.2.28
```

The optimized, stripped binary will be located at:
`target/armv7-unknown-linux-gnueabi/release/axis`

---

## Packaging & Installation

### 1. Build webOS Package (`.ipk`)

Run the automated packaging script:

```bash
./scripts/package-ipk.sh
```

This compiles the binary and packages the assets, icons, fonts, and metadata into a valid webOS control/data archive:
`dist/org.axis.native_1.0.0_arm.ipk`

### 2. Install on TV

If your TV is rooted and accessible via SSH:

```bash
TV_HOST="root@192.168.1.100" SSH_KEY="~/.ssh/id_ed25519" ./scripts/install-on-tv.sh
```

Or deploy manually via the webOS CLI:

```bash
ares-install --device tv dist/org.axis.native_1.0.0_arm.ipk
```

---

## Configuration

Axis stores its configuration on the TV at `/home/root/.config/axis/config.json`. If no configuration exists, it automatically generates one from `assets/config.default.json`.

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

All settings can also be modified in real-time from the on-screen **Settings** menu.

---

## Credits & Acknowledgments

- **Concept, Design & Original Web Version**: [@zzeppieri](https://github.com/zzeppieri) and contributors of [`webos-custom-home`](https://github.com/zzeppieri/webos-custom-home).
- **Native Rust Architecture & Optimization**: [Varun Gupta](https://github.com/varungupta3009).
- **Weather API**: Powered by [Open-Meteo](https://open-meteo.com/).
- **Homebrew Ecosystem**: Special thanks to the [webOS Homebrew Project](https://github.com/webosbrew).

---

## License

This project is licensed under the [MIT License](LICENSE).
