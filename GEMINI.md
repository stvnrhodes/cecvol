# CECVol Context

## Project Overview

**CECVol** is a home automation project designed to control TV volume, power, and inputs remotely. It consists of two main components:

1.  **Rust Server:** A lightweight HTTP server that interfaces with the TV via HDMI-CEC (on Raspberry Pi) or LG IP Control (over LAN).
2.  **WearOS App:** A standalone Android watch app to control the TV from a wrist.

The system is designed to be compatible with Google Smart Home Actions, exposing a `fulfillment` endpoint.

## Components

### 1. Rust Server (`/`)

The core application logic.

**Key Technologies:**

- **Language:** Rust
- **CEC Interface:** direct `ioctl` calls to `/dev/vchiq` (Raspberry Pi specific).
- **LG IP Control:** TCP with symmetric encryption (AES).
- **Metrics:** Prometheus (`/varz`).

**Key Files:**

- `src/bin/cecvol.rs`: Main entry point. Handles CLI args, HTTP server setup, and routing.
- `src/lib.rs`: Library module declarations.
- `src/cec/`: CEC implementation (hardware interface).
- `src/lgip.rs`: LG IP control implementation.
- `src/action/`: Google Smart Home Action data structures.

**Build & Run:**

```bash
# Build
cargo build --release

# Run (Example with fake CEC for testing on non-Pi)
cargo run -- --use-fake-cec-conn --server-mac-addr "00:11:22:33:44:55"

# Run (LG IP Control example)
cargo run -- \
  --use-lg-ip-control \
  --lg-mac-addr "AA:BB:CC:DD:EE:FF" \
  --lg-keycode "MY_KEY" \
  --server-mac-addr "00:11:22:33:44:55"
```

**Configuration (CLI Args / Env Vars):**

- `--http-addr`: Listen address (default `0.0.0.0:8080`).
- `--server-mac-addr` (Required): MAC address of the server (used for WoL logic).
- `--lg-mac-addr` / `LG_MAC_ADDR`: Target TV MAC.
- `--lg-keycode` / `LG_KEYCODE`: Pairing key for LG TV.

### 2. WearOS App (`/wearos`)

A client application for Android smartwatches.

**Key Technologies:**

- **Language:** Kotlin
- **Platform:** Android (WearOS)
- **Build System:** Gradle

**Key Files:**

- `wearos/app/src/main/AndroidManifest.xml`: App manifest.
- `wearos/app/src/main/java/com/stevenandbonnie/cecvol/presentation/MainActivity.kt`: Main UI logic.

**Build:**

```bash
cd wearos
./gradlew build
```

## Architecture Notes

- **API Design:** The server exposes a `/fulfillment` endpoint that accepts JSON payloads structured like Google Smart Home Action requests (SYNC, QUERY, EXECUTE).
- **Abstraction:** The `tv::TVConnection` trait abstracts the underlying control method (CEC or LG IP), allowing the rest of the application to be agnostic to the specific hardware interface.
- **Wake-on-LAN:** The `wol` module handles sending magic packets to wake devices.
