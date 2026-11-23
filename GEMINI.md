# CECVol Context

## Project Overview

**CECVol** is a home automation project designed to control an LG webOS TV's volume, mute, power, inputs, playback, and navigation over LAN. It consists of two main components:

1.  **Rust Server:** A lightweight HTTP server that controls the TV via LG IP Control (TCP + AES-128-CBC) and Wake-on-LAN, serving a web remote UI and Google Smart Home Cloud-to-Cloud fulfillment.
2.  **WearOS App:** A standalone Android watch app to control the TV from a wrist.

## Components

### 1. Rust Server (`/`)

The core application logic.

**Key Technologies:**

- **Language:** Rust
- **LG IP Control:** TCP port 9761 with AES-128-CBC encryption (`src/lgip.rs`).
- **Google Home OAuth 2.0:** Stateless HMAC-SHA256 tokens (`src/oauth.rs`).
- **Metrics:** Prometheus (`/varz`).

**Key Files:**

- `src/bin/cecvol.rs`: Main entry point. Handles CLI args, TV setup, and server startup.
- `src/bin/tvcli.rs`: Small CLI utility to send raw LG IP commands directly to the TV.
- `src/lib.rs`: Library module declarations.
- `src/server.rs`: HTTP routing (`/`, `/manifest.json`, `/varz`, `/fulfillment`, `/oauth/authorize`, `/oauth/token`, `/google/fulfillment`).
- `src/lgip.rs`: LG IP control implementation (with direct Avahi socket `.local` resolution).
- `src/tv.rs`: `TVConnection` trait and in-memory `FakeTV` for local testing.
- `src/oauth.rs`: Stateless OAuth 2.0 server for Google Home Cloud-to-Cloud linking.
- `src/action/`: Google Smart Home Action schemas (`devices.rs`) and intent handler (`fulfillment.rs`).
- `src/index.html`: Embedded web remote UI.

**Build & Run:**

```bash
# Build
cargo build --release

# Run with in-memory fake TV for local testing
cargo run -- --use-fake-tv --server-mac-addr "00:11:22:33:44:55"

# Run against an LG TV over LAN
cargo run -- \
  --lg-mac-addr "AA:BB:CC:DD:EE:FF" \
  --lg-keycode "MY_KEY" \
  --server-mac-addr "00:11:22:33:44:55"
```

**Configuration (CLI Args / Env Vars):**

- `--http-addr`: Listen address (default `0.0.0.0:8080`).
- `--use-fake-tv`: Use an in-memory stateful fake TV instead of connecting to a real LG TV.
- `--lg-addr` / `LG_ADDR`: Target TV hostname or IP (default `LGWebOSTV.local`).
- `--lg-mac-addr` / `LG_MAC_ADDR`: Target TV MAC (required unless `--use-fake-tv`).
- `--lg-keycode` / `LG_KEYCODE`: Pairing key for LG TV (required unless `--use-fake-tv`).
- `--server-mac-addr` / `SERVER_MAC_ADDR` (Required): MAC address of the server (used for WoL).
- `--oauth-client-id` / `OAUTH_CLIENT_ID`, `--oauth-client-secret` / `OAUTH_CLIENT_SECRET`, `--oauth-signing-key` / `OAUTH_SIGNING_KEY`: Enable Google Home OAuth 2.0 endpoints.
- `--google-project-id` / `GOOGLE_PROJECT_ID`, `--oauth-redirect-uri` / `OAUTH_REDIRECT_URIS`: Allowed OAuth redirect URIs.
- `--volume-step-size` / `VOLUME_STEP_SIZE`: Relative volume step size advertised to Google Home (default `1`).

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

- **API Design:** `/fulfillment` (for web UI / WearOS behind a reverse proxy) and `/google/fulfillment` (Bearer-authenticated for Google Home Cloud-to-Cloud) share the same `FulfillmentHandler` supporting `SYNC`, `QUERY`, `EXECUTE`, and `DISCONNECT`.
- **Abstraction:** `tv::TVConnection` abstracts TV control (`LGTV` for real hardware, `FakeTV` for local testing, `MockTV` for unit tests), always reporting live state (`on`, `activityState`, `currentVolume`, `isMuted`).
- **Wake-on-LAN:** The `wol` module handles sending magic packets to wake devices.
