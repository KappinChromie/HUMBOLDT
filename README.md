# HUMBOLDT

> **Analytics & Real-time Global Information System**  
> *A self-contained, privacy-respecting, single-binary planetary intelligence workstation.*

[![License: MIT / Apache 2.0](https://img.shields.io/badge/License-MIT%2FApache%202.0-blue.svg)](#license)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Docker-brightgreen.svg)](#installation)
[![Offline First](https://img.shields.io/badge/Offline%20First-Tier%201%20Natural%20Earth-orange.svg)](#cartography)
[![Privacy Guarantee](https://img.shields.io/badge/Privacy-Zero%20Telemetry%20Leakage-red.svg)](#security-and-privacy)

---

## 1. Overview

**HUMBOLDT** is a local-first, privacy-respecting desktop intelligence and planetary observation platform. Designed as a self-contained single binary with zero external telemetry dependencies, HUMBOLDT ingests and correlates real-time environmental, infrastructure, orbital, and geopolitical data streams into an interactive 3D virtual globe and time-series analytical workstation.

Unlike cloud SaaS dashboards that transmit client coordinates and IP addresses to centralized telemetric collectors, HUMBOLDT processes all geospatial indexes, coordinate projections, SGP4 orbital mechanics, and statistical divergence comparisons **entirely on your local machine**.

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TOP BAR: [Logo: HUMBOLDT] [Port: 8374] [Health: Nominal] [Time: UTC] [Attribution]    │
├─────────┬──────────────────────────────────────────────────────────────┬───────────────┤
│ LAYER   │                                                              │ CONTEXT PANEL │
│ RAIL    │                                                              │ [News / Graph]│
│         │                                                              │               │
│ [x] Air │                                                              │ Location:     │
│ [x] Sea │                      CESIUM 3D GLOBE                         │ Kansas City   │
│ [x] Sat │                                                              ├───────────────┤
│ [x] Qk  │                                                              │ Top News:     │
│ [x] Wth │                                                              │ • Headline 1  │
│ [x] Met │                                                              │ • Headline 2  │
│         │                                                              ├───────────────┤
│         │                                                              │ Graph / Comp: │
│         │                                                              │ [  ECharts  ] │
├─────────┴──────────────────────────────────────────────────────────────┴───────────────┤
│ TIMELINE BAR: [Play/Pause] [◀◀] [▶▶] [Speed: 1x] ───○──────────────────────── [Live]   │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Capabilities

* **3D Planetary Cartography:** Offline-capable 3D virtual globe powered by CesiumJS, bundling low-resolution Natural Earth raster imagery and vector geometry.
* **Live Ingestion Feeds:**
  * **Seismic Activity:** USGS Earthquake Hazards Program (real-time M1.0+ and M4.5+ events).
  * **Orbital Mechanics:** CelesTrak active satellite orbits propagated locally via SGP4 geodetic models.
  * **Aviation Tracking:** Multi-provider ADS-B streams (airplanes.live, adsb.lol) with Dead Reckoning.
  * **Maritime AIS:** Vessel positions, course over ground, and speed from community AIS hubs.
  * **Space Weather:** NOAA Space Weather Prediction Center planetary geomagnetic Kp index.
  * **Power Grids & Renewable Mix:** National Grid ESO fuel mix (wind, solar, nuclear, gas).
  * **Macro Internet Outages:** CAIDA IODA macro-outage metrics and BGP drop alerts.
  * **Atmospheric Weather:** Open-Meteo hourly surface metrics (temperature, wind, barometric pressure).
* **Spatial Geometry Engine:** Offline R-Tree index (`rstar`) indexing countries, states, and Census places to resolve geographical hierarchy (`Country → State → County → City`) in $< 1 \text{ ms}$.
* **News Hierarchy Fallback:** Hierarchical news matching combining GDELT DOC 2.0 and Google News RSS feeds with provenance tagging (`[Direct]`, `[Regional]`, `[Statewide]`).
* **Analytical Snapshot & Comparison Visualizer:** Freeze live time-series into SQLite Gzip-compressed snapshots. Compare baseline periods against historical anomalies using dual-mode alignment (Calendar vs. Relative Day-of-Period) with divergence statistics ($\Delta\text{Mean}$, $\Delta\text{Max}$, $\Delta\text{Min}$, NRMSD).
* **Alerts & Notifications:** In-memory rule evaluation engine triggering native Win32 Desktop Toast notifications (registered under AUMID `Humboldt.Desktop`) and Discord webhook cards with rich embeds.
* **LAN Security & Remote Access:** Argon2id session authentication for trusted home network access with rate-limited brute-force protection and plain-HTTP-friendly cookies.
* **Self-Governing Storage:** Automatic WAL-mode SQLite storage with hourly rollups, downsampling (1 pt/min after 2 hours), and emergency FIFO pruning upon reaching 90% of the 10 GB disk cap.

---

## 3. Quick Start

### Windows (Desktop Workstation)
1. Download `humboldt-setup-x64.exe` from the latest release.
2. Run the installer. It installs without requiring Administrator privileges to `%LOCALAPPDATA%\Programs\Humboldt`.
3. Launch **HUMBOLDT** from your Start Menu. A system tray icon appears, and your default web browser automatically opens to `http://127.0.0.1:8374`.

### Linux / Server (Headless Daemon)
HUMBOLDT automatically detects headless environments and disables browser auto-launch and system tray initialization:

```bash
# Run standalone
export HUMBOLDT_HEADLESS=1
export HUMBOLDT_DATA_DIR=/var/lib/humboldt
export HUMBOLDT_PORT=8374
./humboldt
```

### Docker
Run HUMBOLDT as an isolated, unprivileged container with persistent storage:

```bash
docker run -d \
  --name humboldt \
  --restart unless-stopped \
  -p 8374:8374 \
  -v humboldt_data:/data \
  -e HUMBOLDT_HEADLESS=1 \
  -e HUMBOLDT_LAN_MODE=1 \
  ghcr.io/humboldt/humboldt:latest
```

### Docker Compose
```yaml
services:
  humboldt:
    image: ghcr.io/humboldt/humboldt:latest
    container_name: humboldt
    restart: unless-stopped
    ports:
      - "8374:8374"
    volumes:
      - humboldt_data:/data
    environment:
      - HUMBOLDT_HEADLESS=1
      - HUMBOLDT_DATA_DIR=/data
      - HUMBOLDT_PORT=8374
      - HUMBOLDT_LAN_MODE=1

volumes:
  humboldt_data:
    driver: local
```

---

## 4. Architecture & Crate Topology

HUMBOLDT is built in Rust using a modular workspace layout for performance, memory safety, and cross-platform reliability:

| Crate | Purpose |
| :--- | :--- |
| `crates/humboldt-core` | Canonical data contracts, domain models, configuration, error types, snapshot math. |
| `crates/humboldt-storage` | SQLite connection pooling (WAL mode), migrations, retention sweeper, and rollups. |
| `crates/humboldt-security` | AES-256-GCM vault, OS keyring bridge, Argon2id LAN password hashing, session tokens. |
| `crates/humboldt-collectors` | Telemetry collectors, SGP4 orbital mechanics, and resilient Ingest Router (`TokenBucket`, `CircuitBreaker`). |
| `crates/humboldt-geo` | Sub-millisecond R-Tree spatial index, Douglas-Peucker simplified boundary polygons. |
| `crates/humboldt-news` | GDELT DOC 2.0 & Google News RSS feeds with geographic fallback. |
| `crates/humboldt-alerts` | Rules engine, Win32 toast notifications (`winrt-notification`), and Discord webhook dispatcher. |
| `crates/humboldt-server` | Axum HTTP server, static asset embedding (`rust-embed`), WebSocket event bus, tray icon, Win32 launcher. |
| `web/` | Vite 5 SPA frontend built with TypeScript, CesiumJS, and Apache ECharts. |

For full architectural blueprints, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 5. Security & Privacy Guarantees

* **Zero Outbound Telemetry:** HUMBOLDT never sends usage metrics, crash reports, crash dumps, or user telemetry to developers or third parties.
* **Encrypted Vault Subsystem:** API credentials and Discord webhook URLs are encrypted using **AES-256-GCM** before touching disk. Master encryption keys are derived from the OS credential store (Windows Credential Manager / Linux secret service) or an encrypted local keyfile.
* **Webhook URL Redaction:** Webhook URLs are stripped and redacted (`[REDACTED]`) from all application error logs and diagnostic displays.
* **Plain HTTP LAN Compatibility:** Cookies issued for LAN authentication deliberately omit the `Secure` flag so modern browsers on private subnets (`192.168.x.x`, `10.x.x.x`) do not reject authentication sessions over plain HTTP.

---

## 6. Attribution & Open Data Licenses

HUMBOLDT synthesizes planetary intelligence exclusively from public domain datasets, open-access research APIs, and cooperative community exchanges:

* **USGS Earthquake Hazards Program:** Public Domain (U.S. Government Work)
* **CelesTrak / Space-Track:** Open Orbital Data / Public Domain
* **NOAA SWPC:** Public Domain (NOAA Space Weather Prediction Center)
* **adsb.lol & airplanes.live:** Creative Commons Attribution 4.0 International (CC BY 4.0)
* **AISstream.io:** Community AIS Exchange / Community Fair Use
* **Open-Meteo:** Creative Commons Attribution 4.0 International (CC BY 4.0)
* **National Grid ESO:** Open Government Licence v3.0 / UK Open Data
* **CAIDA IODA:** Creative Commons Attribution 4.0 International (CC BY 4.0)
* **GDELT Project:** Open Research Data (Fair Use / Public Domain)
* **Natural Earth:** Public Domain Cartographic Vector and Raster Datasets

Detailed provider records and direct outbound links can be inspected via the in-app **Attribution (⚖)** modal or the `GET /api/v1/attribution` endpoint.

---

## 7. License

Licensed under either of:
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
