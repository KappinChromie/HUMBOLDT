# HUMBOLDT: Analytics & Real-time Global Information System
## Master Technical Architecture Specification

---

## 1. System Overview & Core Principles

### 1.1 Project Vision
HUMBOLDT is an open-source, privacy-respecting, single-binary desktop intelligence and planetary observation platform. Named in honor of Alexander von Humboldt, the system synthesizes physical phenomena, infrastructure metrics, natural events, and aggregate global movements into an interactive 3D virtual globe and time-series analytical workstation.

### 1.2 Non-Negotiable Operational Principles
1. **Zero Mandatory Cost & Friend-Friendly Distribution:** Every core observation layer must function out of the box using keyless public endpoints or free community keys. Under no circumstances may a credit-card requirement or paid subscription be introduced into the critical ingestion path.
2. **Strict Privacy & Anti-Surveillance Guardrails:** HUMBOLDT monitors macro infrastructure, natural systems, and aggregate flows. The system categorically forbids:
   - Tracking, profiling, or pinpointing individual private persons.
   - Processing facial recognition or optical automated license plate reader (ALPR) feeds.
   - Aggregating personal dossiers, home addresses, or private individual movement histories.
   - Scraping full-text article bodies (news is constrained strictly to headline, source, timestamp, and outbound URL).
3. **Offline-First Resilience:** In the complete absence of internet access, HUMBOLDT must boot cleanly, display the bundled low-resolution offline base globe, and provide full read/query access to all locally cached time-series data and user-saved snapshots.
4. **Deterministic Engineering ("Build, Don't Interpret"):** All architectural boundaries, schemas, crate ecosystems, and provider routing rules are strictly specified herein. Implementing agents must adhere to these definitions without improvising or substituting dependencies.

---

## 2. Technology Stack & Workspace Architecture

### 2.1 Backend Ecosystem (Rust)
The backend is compiled as a single native executable in Rust (pinned toolchain `1.82+`), embedding the compiled frontend single-page application (SPA) via `rust-embed`.

| Subsystem | Selected Crate | Version | Justification / Role |
| :--- | :--- | :--- | :--- |
| **Async Runtime** | `tokio` | `1.40+` | Multi-threaded runtime with timer, net, and signal support |
| **Web Server** | `axum` | `0.7+` | High-performance routing, static file embedding, WebSocket support |
| **HTTP Middleware** | `tower-http` | `0.5+` | CORS headers, gzip/zstd compression, static asset resolution |
| **HTTP Client** | `reqwest` | `0.12+` | Connection pooling, `rustls-tls` backend (eliminates OpenSSL dependency) |
| **WebSockets (Push)** | `tokio-tungstenite` | `0.24+` | Client connections for live streaming feeds (AISstream, EMSC) |
| **Database Engine** | `sqlx` | `0.8+` | Compile-time checked queries on embedded SQLite with WAL mode |
| **Data Formats** | `serde`, `serde_json` | `1.0+` | Canonical serialization across internal types and JSON APIs |
| **XML / Feed Ingest** | `quick-xml`, `feed-rs` | `0.36+`, `1.5+` | Robust, safe parsing for NOAA, USGS, and RSS syndication feeds |
| **Spatial Indexing** | `geo`, `rstar` | `0.28+`, `0.12+` | Point-in-polygon math, local R-Tree spatial boundary indexes |
| **Orbit Propagation** | `sgp4` | `0.2+` | Local SGP4/SDP4 satellite orbit computation from CelesTrak GP data |
| **Security & Crypto** | `aes-gcm`, `argon2` | `0.10+`, `0.5+` | AES-256-GCM vault encryption; Argon2id password hashing |
| **OS Credentials** | `keyring` | `3.0+` | OS-native secure credential store abstraction |
| **System Tray** | `tray-icon` | `0.19+` | Low-overhead native system tray controller for Windows/Linux |
| **OS Integrations** | `windows-toasts`, `open` | `0.2+`, `5.3+` | Native Win32 toast notifications; default browser dispatch |
| **Logging & Tracing** | `tracing`, `tracing-subscriber` | `0.1+` | Structured logging with level filtering and rolling file appender |

### 2.2 Frontend Ecosystem (TypeScript & Vite)
The frontend is constructed without heavy reactive UI frameworks to ensure maximum rendering throughput and longevity.

* **Language & Build Tool:** TypeScript `5.6+` compiled via Vite `5.4+` into static web assets.
* **3D Globe Engine:** CesiumJS (`v1.122+`) utilizing local bundled assets. No external CDN scripts or stylesheet references are permitted.
* **Analytical Graphing:** Apache ECharts (`v5.5+`) for multi-series rollups, relative alignment overlays, and native client-side canvas/SVG PNG exports.
* **Design System & Styling:** Native CSS variables, fixed 4px geometric spacing scale, dark-first instrument terminal theme (`#0D1117` canvas, `#161B22` panels, `#30363D` borders, `#58A6FF` primary telemetry accent).
* **Icons:** Bundled Lucide SVG icon set.

### 2.3 Workspace Directory Structure
```text
humboldt/
├── Cargo.toml                       # Top-level workspace manifest
├── rust-toolchain.toml              # Pinned toolchain specification
├── docker-compose.yml               # Linux headless server definition
├── Dockerfile                       # Multi-stage container build
├── installer/
│   ├── humboldt.iss                 # Inno Setup Windows installer script
│   └── humboldt.service             # systemd unit for Linux servers
├── crates/
│   ├── humboldt-core/               # Observation schema, traits, error types, config structs
│   ├── humboldt-storage/            # SQLite connection pool, migrations, retention sweeps
│   ├── humboldt-security/           # AES-256-GCM vault, OS keyring bridge, Argon2id auth
│   ├── humboldt-geo/                # R-Tree index, bundled boundary polygons, PIP engine
│   ├── humboldt-collectors/         # Ingest adapters (Failover, Spread, and Merge routers)
│   ├── humboldt-news/               # GDELT DOC/GEO client, RSS feed aggregator, hierarchy fallback
│   ├── humboldt-alerts/             # Rules evaluation engine, desktop toast, Discord webhook
│   └── humboldt-server/             # Axum HTTP routes, WebSocket hub, system tray, browser launcher
├── web/
│   ├── package.json
│   ├── vite.config.ts               # Builds into ../crates/humboldt-server/dist
│   └── src/
│       ├── main.ts                  # Application entry point & layout controller
│       ├── globe/                   # CesiumJS initialization, 3-tier basemaps, entity layers
│       ├── components/              # Layer rail, time slider, context panel, search bar
│       ├── charts/                  # ECharts managers, snapshot comparison visualizer
│       └── services/                # WebSocket client, API bridge, settings synchronization
└── fixtures/                        # Mock JSON/XML responses for fully offline test suites
```

---

## 3. Application Lifecycle, Runtime & OS Integration

### 3.1 Windows Subsystem & Execution (Zero-Console Desktop Mode)
* **Compilation Directive:** The main server executable must compile with `#![windows_subsystem = "windows"]`. Launching the application from the desktop, file explorer, or Start menu must never spawn a command prompt (`cmd.exe`) or terminal console window.
* **Single-Instance Mutex:** Upon initialization, the executable requests a named Win32 mutex: `Global\HumboldtAppMutex`.
  - If the mutex is held by another running process, the newly launched binary reads the active port from `%APPDATA%\Humboldt\.active_port`, invokes the default browser targeting that local URL via `open`, and immediately terminates with exit code `0`.
* **Dynamic Port Allocation:**
  - Default Listening Port: `8374`.
  - Fallback Ports (tried sequentially on `EADDRINUSE`): `8375`, `8376`, `8377`, `8378`.
  - The successfully bound port is written to `%APPDATA%\Humboldt\.active_port`.
  - If all candidate ports are exhausted, a native Win32 message box displays an explicit port allocation error before exit.
* **Automated Browser Launch:** Upon server socket binding, the application dispatches an asynchronous call to open the system default browser navigated to `http://127.0.0.1:<port>`.
* **System Tray Controller:** A persistent icon is created in the Windows taskbar notification area:
  - Tooltip: `HUMBOLDT (Port: <port>)`
  - Context Menu Actions:
    1. `Open Humboldt` (launches/focuses browser tab).
    2. `Copy Local URL` (copies `http://127.0.0.1:<port>` to clipboard).
    3. `Quit HUMBOLDT` (gracefully stops collectors, executes WAL checkpoint, and exits).

### 3.2 Linux & Headless Server Mode
* **Headless Detection:** If the application detects the absence of a display server (`DISPLAY` and `WAYLAND_DISPLAY` environment variables are empty), it disables tray-icon creation and browser launching.
* **Server Binding:** Honors the `--lan` CLI flag or `HUMBOLDT_BIND=0.0.0.0` environment variable to expose services across local network interfaces.
* **Docker Packaging:** Distributed as a multi-stage Alpine/scratch container exposing port `8374`, mounting data to a persistent volume at `/data`.

### 3.3 Application Paths
* **Windows:**
  - Base Directory: `%APPDATA%\Humboldt\`
  - Database File: `%APPDATA%\Humboldt\data\humboldt.db`
  - Cache Storage: `%LOCALAPPDATA%\Humboldt\cache\`
  - Logs Directory: `%LOCALAPPDATA%\Humboldt\logs\`
* **Linux (XDG Base Directory Specification):**
  - Base Configuration: `~/.config/humboldt/`
  - Data Storage: `~/.local/share/humboldt/data/`
  - Cache Storage: `~/.cache/humboldt/`
  - Logs Directory: `~/.local/state/humboldt/logs/`

---

## 4. Security Architecture, Key Vault & LAN Access

### 4.1 Encrypted Credential Vault
All user-entered API keys (e.g., NASA FIRMS, OpenWeatherMap, US EIA, Cesium ion) and webhook targets (Discord) are secured in an encrypted SQLite table:

```sql
CREATE TABLE vault_items (
    key_name TEXT PRIMARY KEY,
    encrypted_value BLOB NOT NULL,
    nonce BLOB NOT NULL,
    updated_at INTEGER NOT NULL
);
```

* **Cipher:** AES-256-GCM. Plaintext credentials are never written to disk.
* **Master Key Management:**
  - *Windows:* Derived from the Windows Credential Manager / DPAPI under the credential target `Humboldt_Master_Vault_Key`.
  - *Linux Desktop:* Resolved via the Secret Service API (`libsecret` / `keyring` crate).
  - *Headless Linux / Docker:* Read from a 256-bit cryptographically random keyfile (`.vault.key`, file permissions `0600`) generated in the data folder on initial boot, or injected via the `HUMBOLDT_MASTER_KEY` environment variable.
* **In-Memory Credential Scrubbing:** Credentials are decrypted strictly in memory at the moment an outbound request is generated. Decrypted values are wrapped in zeroizing containers (`zeroize` crate) and are never exposed via REST API responses, WebSocket messages, debug traces, or diagnostic dumps.

### 4.2 LAN Mode Security & Authentication
* **Default Network Isolation:** Binds strictly to loopback (`127.0.0.1`).
* **LAN Activation Gating:** The LAN toggle in the GUI settings cannot be engaged until a master access password has been configured.
* **Password Hashing:** Passwords are hashed using Argon2id with memory-hard parameters ($m=65536 \text{ KiB}, t=3 \text{ iterations}, p=4 \text{ lanes}$).
* **Session Lifecycle:**
  - Authentication produces an encrypted, signed session cookie (`humboldt_session`).
  - Cookie flags: `HttpOnly`, `SameSite=Strict`, `Path=/`.
  - WebSocket handshakes check and validate this session cookie; unauthenticated socket upgrades are rejected with HTTP 401.
  - Failed authentication attempts trigger escalating exponential rate limits (1s, 2s, 4s, 8s, up to 60s max lockout) to prevent LAN-based automated brute-forcing.
* **Unencrypted LAN Warning:** Because issuing self-signed TLS certificates introduces untrusted root authority warnings in client browsers, LAN mode operates over standard HTTP. The GUI settings display an explicit informational banner noting that LAN mode is designed exclusively for trusted home networks.

---

## 5. Storage Engine, Data Models & Retention Policies

### 5.1 SQLite Database Configuration
The embedded storage subsystem utilizes SQLite via `sqlx`, operated under strict high-throughput settings:
- `PRAGMA journal_mode = WAL;` (Concurrent read transactions alongside background writer task).
- `PRAGMA synchronous = NORMAL;` (Ensures durability while minimizing I/O bottlenecks).
- `PRAGMA busy_timeout = 5000;` (Mitigates transient table lock contentions).
- `PRAGMA cache_size = -64000;` (64 MB in-memory page cache).
- `PRAGMA foreign_keys = ON;`

### 5.2 Core Telemetry Schema
All physical, environmental, and spatial observations are ingested into a unified data structure:

```sql
CREATE TABLE observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,          -- Unix epoch milliseconds (UTC)
    layer TEXT NOT NULL,                 -- 'aircraft', 'ship', 'satellite', 'quake', 'weather', etc.
    provider TEXT NOT NULL,              -- Slug: 'opensky', 'usgs', 'noaa_swpc', etc.
    entity_id TEXT,                      -- Normalized ID: ICAO24, MMSI, NORAD-ID, Station ID
    latitude REAL,                       -- WGS84 Latitude (-90.0 to 90.0)
    longitude REAL,                      -- WGS84 Longitude (-180.0 to 180.0)
    altitude_m REAL,                     -- Altitude / Elevation in meters
    metric_name TEXT,                    -- e.g., 'kp_index', 'temp_c', 'gauge_height_m'
    metric_value REAL,                   -- Numeric scalar for charting
    attributes TEXT,                     -- Serialized JSON payload of layer-specific attributes
    is_interpolated INTEGER NOT NULL DEFAULT 0 -- 1 if dead-reckoned / estimated, else 0
);

CREATE INDEX idx_obs_layer_time ON observations (layer, timestamp DESC);
CREATE INDEX idx_obs_entity_time ON observations (entity_id, timestamp DESC);
CREATE INDEX idx_obs_metric_time ON observations (metric_name, timestamp DESC);
CREATE INDEX idx_obs_geo ON observations (latitude, longitude) WHERE latitude IS NOT NULL;
```

### 5.3 Snapshot & Analysis Schema
```sql
CREATE TABLE snapshots (
    id TEXT PRIMARY KEY,                 -- UUIDv4 identifier
    name TEXT NOT NULL,                  -- User-defined label (e.g., 'KC Temp Oct 2026')
    created_at INTEGER NOT NULL,         -- Unix epoch timestamp
    layer TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    query_entity TEXT,                   -- Target location name, boundary ID, or station
    start_time INTEGER NOT NULL,         -- Epoch start of snapshot range
    end_time INTEGER NOT NULL,           -- Epoch end of snapshot range
    resolution TEXT NOT NULL,            -- 'raw', '1h_rollup', '1d_rollup'
    metadata_json TEXT NOT NULL,         -- Serialized provider provenance, unit, coordinates
    data_points BLOB NOT NULL            -- Gzipped, serialized array of (timestamp, value) tuples
);
```

### 5.4 Retention Classes & Automated Pruning Policy
Disk utilization is constrained by a strict ceiling defaulting to 10 GB (configurable in UI: 1 GB – 50 GB). A pruning sweep runs every 10 minutes:

| Retention Class | Layers Assigned | Raw Data Retention | Downsampling & Rollup Strategy |
| :--- | :--- | :--- | :--- |
| **Ephemeral** | Local News, GDELT Articles, Active Alerts | In-memory RAM buffer + SQLite 2 hours | Never summarized; purged permanently after 2 hours. |
| **Live Tracking** | Aircraft, AIS Ships, Satellites | 2 hours at full frequency | Downsampled to 1 position/min after 2 hours. Purged completely after 48 hours. |
| **Metric Series** | Weather, Power Grid, River Levels, Buoys, Kp Index, Air Quality | 14 days at full ingest resolution | Rolled up to 1-hour averages from 14 to 90 days. Rolled up to 1-day averages from 90 to 365 days. |
| **Discrete Events** | Earthquakes, Wildfires / Thermal Points, Solar Flares | 180 days full event logs | Preserved unaggregated. Purged oldest first only when disk cap breaches 90%. |
| **User Snapshots** | Saved Graphs & Analytical Sets | Permanent | Exempt from automated purging sweeps. Survives all retention limits. |

* **Reclamation Triggers:**
  - If database size exceeds 90% of configured cap ($> 9.0 \text{ GB}$ on a 10 GB limit), automated pruning runs aggressively: Live Tracking data is truncated from 48h to 24h, followed by Metric Series rollups $> 180 \text{ days}$.
  - Storage Management actions available in UI:
    1. `Clear Caches`: Truncates ephemeral news, tile caches, and dead-reckoned tracking points.
    2. `Delete All Snapshots`: Purges all saved analytical snapshots.
    3. `Compact Database`: Executes `VACUUM;` to return disk space to the OS.

---

## 6. Provider Registry, Ingestion Framework & Routing Modes

### 6.1 Routing Modes
The ingest scheduler routes requests through three distinct operational paradigms:
1. **Failover Mode:** Queries the highest-priority enabled provider. If an HTTP 429, timeout, or repeated 5xx error occurs, traffic seamlessly cascades to the secondary provider, returning to primary upon recovery.
2. **Spread Mode:** Splits uniform queries (e.g., current city weather) across multiple interchangeable providers in proportion to their remaining API quotas to prevent throttling.
3. **Merge Mode:** Queries all active providers covering a spatial bounding box simultaneously and merges the results into a unified entity state, deduplicating records by normalized entity IDs (e.g., merging ADS-B feeds by ICAO24 hex address, or AIS feeds by MMSI).

### 6.2 Provider Registry Matrix

| Layer | Provider Slug | Auth Type | Ingest Cadence & Rate Limits | Storable | Routing Mode | Attribution Requirement |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Aircraft** | `adsb_lol` | Keyless (opt. key) | Viewport-driven bbox; 5–10s poll | Yes (48h) | Merge | Non-commercial community credit |
| **Aircraft** | `airplanes_live` | Keyless | Max 1 req/sec; radius $\le$ 250 NM | Yes (48h) | Merge | Attribution link to airplanes.live |
| **Aircraft** | `adsb_fi` | Keyless | Max 1 req/sec; non-commercial | Yes (48h) | Merge | Mandatory attribution with link |
| **Aircraft** | `opensky` | Free Account (OAuth2) | 4,000 credits/day; 1 cr/bbox query | Yes (48h) | Failover / Detail | Academic/research attribution |
| **Ships (AIS)** | `aisstream` | Free Key | WebSocket push; 3 conn cap; bbox sub | Yes (48h) | Merge | Visible credit to AISstream.io |
| **Ships (Baltic)** | `digitraffic_fi` | Keyless | 60 req/min REST / Live MQTT-WS | Yes (48h) | Regional Adapter | CC BY 4.0 Digitraffic |
| **Satellites** | `celestrak` | Keyless | Strict $\ge$ 2h cadence; CSV/JSON format | Yes (Orbits) | Primary Local Prop | CelesTrak citation required |
| **Earthquakes** | `usgs_quakes` | Keyless | 60s poll on GeoJSON summary feeds | Yes | Failover (Primary) | Public domain (USGS) |
| **Earthquakes** | `emsc_seismic` | Keyless | Live WebSocket stream push | Yes | Merge (Live Push) | CC BY 4.0 EMSC |
| **Wildfires** | `nasa_firms` | Free Key (`MAP_KEY`) | 1,000 req/day; 15m viewport bbox poll | Yes | Primary | NASA Open Data Policy |
| **Space Weather** | `noaa_swpc` | Keyless | 60s plasma/flux; 5m Kp index | Yes | Primary | Public domain (NOAA SWPC) |
| **Rivers / Hydro** | `usgs_water` | Keyless | 15m poll cadence per bbox | Yes | Regional (US) | Public domain (USGS NWIS) |
| **Rivers / Hydro** | `open_meteo_flood`| Keyless | 10k req/day; GloFAS model | Yes | Regional (Global) | CC BY 4.0 Open-Meteo |
| **Marine Buoys** | `noaa_ndbc` | Keyless | 15m poll for active stations | Yes | Primary | Public domain (NOAA NDBC) |
| **Power Grid** | `us_eia` | Free Key | 5,000 rows/req; 30m poll | Yes | Regional (US) | Public domain (EIA Open Data) |
| **Power Grid** | `uk_carbon_intensity`| Keyless | 30m generation mix poll | Yes | Regional (UK) | CC BY 4.0 UK National Grid ESO |
| **Internet Health** | `caida_ioda` | Keyless | 5–15m poll for BGP & active outages | Yes | Primary | CAIDA Open Research Data |
| **Weather** | `open_meteo` | Keyless | 10k calls/day; 600/min | Yes | Spread (Primary) | CC BY 4.0 Open-Meteo |
| **Weather** | `nws_weather` | Keyless (opt. key) | Generous fair use; 5s backoff | Yes | Regional (US) | Public domain (NWS) |
| **Weather** | `met_norway` | Keyless (Opt-in) | Off by default; requires user contact | Yes | Opt-in | CC BY 4.0 MET Norway |
| **Weather** | `openweathermap` | Free Key | 60 calls/min; 1,000,000 calls/mo | Yes | Spread (Fallback) | Attribution to OpenWeatherMap |
| **Radar** | `rainviewer` | Keyless | Past radar; zoom cap 7; 100 req/m | Live-only | Primary (Global) | RainViewer non-commercial |
| **Radar** | `noaa_mrms` | Keyless | 1 km CONUS base reflectivity WMS | Live-only | Regional (US) | Public domain (NOAA MRMS) |
| **Air Quality** | `openaq` | Free Key | 60 req/min; 2,000/hr | Yes | Spread | OpenAQ Station Attribution |
| **Global News** | `gdelt_doc` | Keyless | ~1 req/sec polite limit | Cache (2h) | Primary | GDELT Project Fair Use |
| **Local News** | `google_news_rss` | Keyless | Fair use; 15m local cache | Cache (2h) | Hierarchy Fallback | Google News Syndication |

### 6.3 Politeness, User-Agent & Rate Control
* **Canonical User-Agent:** Outbound requests transmit the header:
  `User-Agent: HUMBOLDT/1.0 (+https://github.com/humboldt/humboldt)`
* **Contact Details Protection:** No personal email address is compiled into the binary. For providers requiring contact info (MET Norway), the setting is disabled by default; enabling it prompts the user to enter their own contact handle inside the GUI Settings panel.
* **Token Bucket Quota Engine:** Requests are metered via in-memory token buckets tracking 5 concurrent sliding windows: per-second, per-minute, per-hour, per-day, and per-month.
* **Backoff Strategy:** Encountering HTTP 429 or connection loss invokes exponential backoff ($T_{\text{backoff}} = 2^n \times \text{jitter}$, capped at 15 minutes).

---

## 7. Geographic Hierarchy, Boundary Index & News Engine

### 7.1 Offline Boundary Geometry Engine
To eliminate dependency on rate-limited external reverse-geocoders, all administrative boundary lookups run locally via `rstar` (R-Tree spatial index):

```text
Globe Click Coordinate (Latitude, Longitude)
                      │
                      ▼
        Local R-Tree Spatial Index
                      │
   ├── Level 0: Natural Earth Admin 0 Polygons (Country)
   │
   ├── Level 1: Natural Earth Admin 1 Polygons (State / Province)
   │
   ├── Level 2: US Census TIGER / Regional Polygons (County / District)
   │
   └── Level 3: Census Places / GeoNames Points (>5,000 pop, within 25 km radius)
```

* **Geometry Packaging:** Simplified via Douglas-Peucker ($\epsilon = 0.001$), serialized into a compact binary boundary file bundled directly into the executable ($< 45 \text{ MB}$).
* **Execution Constraint:** Resolution from coordinates to full hierarchy (`Country → State → County → City`) executes in $< 5 \text{ ms}$ completely offline.

### 7.2 News Aggregation & Hierarchy Fallback Algorithm
When an administrative entity is selected:
1. **Level 3 (City/Town Query):**
   - Ingests articles from GDELT DOC API: `query="{CityName}" sourcelang:eng`.
   - Checks configured curated RSS feeds for that specific metropolitan area.
2. **Fallback Execution:**
   - If fewer than 3 verified articles are returned within the past 48 hours, the query automatically cascades to Level 2 (County / District), then Level 1 (State/Province).
   - If GDELT produces 0 results for a small municipality, the engine invokes Google News RSS (`q="{CityName}+{StateName}"`).
3. **Provenance & Local Proximity Labeling:**
   - Articles in the Context Panel are tagged with explicit breadcrumbs:
     `[Direct: Kansas City]`, `[Regional: Jackson County]`, or `[Statewide: Missouri]`.
   - Content is strictly constrained to: Title, Publication Timestamp, Source Domain, and Link URL.

---

## 8. 3-Tier Cartography, 3D Globe & Map Tiles

```text
┌────────────────────────────────────────────────────────────────────────┐
│                   3-TIER MAP RESOLUTION ARCHITECTURE                   │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 3: Downloadable Offline PMTiles Extracts                          │
│         Street-level zoom, building footprints, zero network calls     │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 2: Free-Key Online Providers (Cesium ion / MapTiler / Stadia)     │
│         3D OSM buildings, global terrain meshes, satellite aerials     │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 1: Bundled Offline Basemap (Natural Earth 1:50m raster + vectors) │
│         Zero-token, zero-CDN fallback active on first launch           │
└────────────────────────────────────────────────────────────────────────┘
```

### 8.1 Tier 1: Bundled Offline Basemap
- Shipped inside the application binary assets.
- Provides low-resolution raster imagery and administrative outlines.
- Operates indefinitely with zero network access up to regional zoom levels.

### 8.2 Tier 2: Free-Key Online Providers
- **Cesium ion:** Free community token unlocks Cesium World Terrain and global 3D OpenStreetMap building primitives.
- **MapTiler Cloud:** Free token provides high-resolution satellite imagery and OpenStreetMap vector road networks up to 100,000 requests/month.
- **Stadia Maps:** Free token provides high-contrast cartographic vector basemaps up to 200,000 credits/month.
- Integrated setup guide in Settings includes direct registration links and a real-time `Test Key` validation button.

### 8.3 Tier 3: Downloadable Offline Map Packs (PMTiles)
- In-app map pack manager allowing users to download regional OpenStreetMap extracts packaged as single-file `.pmtiles` archives (e.g., state, metro area, or country extracts).
- Rust backend acts as a local HTTP range server (`/api/v1/tiles/{region}/{z}/{x}/{y}.pbf`), rendering street-level vectors and road networks with zero third-party network traffic.

---

## 9. Graphing, Analytical Engine & Snapshot Comparison

### 9.1 Time-Series Charting Architecture
The right context panel embeds Apache ECharts to render responsive time-series visualizations for all scalar telemetry layers (temperature, water gauge height, power grid generation, geomagnetic Kp index, river discharge).

### 9.2 Snapshot Freezing Mechanism
- Clicking `Save Snapshot` extracts the active time-series dataset from the current view.
- Serializes the underlying timestamps, scalar readings, geographic identifiers, and provider provenance into an immutable record in the `snapshots` table.
- Snapshots are permanently retained and exempt from automated storage cleanup sweeps.

### 9.3 Dual Comparison Modes
The comparison visualizer allows comparing two datasets using two alignment modes:

```text
Mode 1: Calendar Date Alignment
Series A: Oct 01 ────────► Oct 31, 2026
Series B: Oct 01 ────────► Oct 31, 2025
(X-axis represents absolute calendar time; identifies year-over-year deviations)

Mode 2: Relative Day-of-Period Alignment
Series A: Day 01 [Oct 01] ────────► Day 30 [Oct 30]
Series B: Day 01 [Nov 01] ────────► Day 30 [Nov 30]
(X-axis normalized to Elapsed Days; reveals cyclical shifts between consecutive months)
```

### 9.4 Statistical Variance Engine
Overlaid comparisons automatically compute delta statistics:
- $\Delta \text{Mean} = \overline{x}_B - \overline{x}_A$
- $\Delta \text{Max} = \max(B) - \max(A)$
- $\Delta \text{Min} = \min(B) - \min(A)$
- Normalized Root-Mean-Square Deviation (NRMSD):
  $$\text{NRMSD} = \frac{\sqrt{\frac{1}{N}\sum_{i=1}^N (y_{B,i} - y_{A,i})^2}}{\max(A) - \min(A)}$$
- Exports: Single-click visual export to high-resolution PNG or raw data export to CSV.

---

## 10. Rules Engine & Alert Notification Subsystem

### 10.1 Alert Rule Definition
Alerts are evaluated in-memory against streaming observations via an event bus:

```rust
pub struct AlertRule {
    pub id: String,                         // UUIDv4
    pub name: String,                       // User-defined label (e.g., 'Major Quake')
    pub enabled: bool,
    pub layer: LayerType,
    pub metric_name: String,                // 'magnitude', 'kp_index', 'gauge_height_m'
    pub operator: ComparisonOp,             // GreaterThan, LessThan, Equal
    pub threshold: f64,                     // 5.0, 6.0, 15.2
    pub region_boundary_id: Option<String>, // Optional spatial polygon filter
    pub cooldown_seconds: u32,              // Anti-spam timer (default: 3600s)
    pub last_triggered: Option<i64>,        // Epoch millis of last dispatch
    pub notify_desktop: bool,               // Windows Toast flag
    pub notify_discord: bool,               // Discord Webhook flag
}
```

### 10.2 Notification Channels
* **Windows Desktop Notifications:** Dispatched via native Win32 Toast APIs. Registered during installer execution under the Application User Model ID (AUMID) `Humboldt.Desktop`. Toasts display without requiring an active browser tab.
* **Discord Webhook Client:** Generates rich embedded webhook cards featuring:
  - Embed color matched to severity (Yellow: Advisory, Orange: Watch, Red: Warning).
  - Field values: Observed Reading, Threshold, Event Location, Provider Attribution.
  - Deep-link button directing to the local instance's coordinates.
* **Spam Suppression:** Enforces per-rule cooldown periods (default: 60 minutes) and configurable quiet hours.

---

## 11. User Interface & Design Specifications

### 11.1 Layout Grid
```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ TOP BAR: [Logo: HUMBOLDT] [Port: 8374] [Health: Nominal] [Time: UTC] [Settings]       │
├─────────┬──────────────────────────────────────────────────────────────┬───────────────┤
│ LAYER   │                                                              │ CONTEXT PANEL │
│ RAIL    │                                                              │ [News / Graph]│
│         │                                                              │               │
│ [x] Air │                                                              │ Location:     │
│ [x] Sea │                      CESIUM 3D GLOBE                         │ Kansas City   │
│ [x] Sat │                                                              ├───────────────┤
│ [ ] Qk  │                                                              │ Top News:     │
│ [x] Wth │                                                              │ • Headline 1  │
│ [ ] Met │                                                              │ • Headline 2  │
│         │                                                              ├───────────────┤
│         │                                                              │ Graph / Comp: │
│         │                                                              │ [  ECharts  ] │
├─────────┴──────────────────────────────────────────────────────────────┴───────────────┤
│ TIMELINE BAR: [Play/Pause] [◀◀] [▶▶] [Speed: 1x] ───○──────────────────────── [Live]   │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 11.2 Design Rules & Terminal Aesthetic
* **No "Vibe Coding" Visual Artifacts:** Avoid glassmorphism, heavy drop shadows, neon rainbow gradients, and emojis as primary UI icons.
* **Color Palette:**
  - Base Background: `#0D1117`
  - Panel Surface: `#161B22`
  - Dividers & Outlines: `#30363D`
  - Text Primary: `#C9D1D9`
  - Text Secondary / Dim: `#8B949E`
  - Accent / Telemetry Focus: `#58A6FF`
  - Warning Accent: `#D29922`
  - Critical / Alert Accent: `#F85149`
* **Typography:** System monospace and neutral clean sans-serif (`Inter`, `SF Pro`, `Segoe UI`, `JetBrains Mono`).

---

## 12. Milestone Roadmap & Verifiable Definition of Done

### Milestone 1: Workspace Scaffolding, Core Server & 3D Globe Shell
* **Objectives:** Rust workspace initialization, Axum HTTP server, static asset embedding with `rust-embed`, Win32 `#![windows_subsystem = "windows"]` configuration, dynamic port assignment (8374–8378), system tray integration, and CesiumJS initialization with Tier 1 bundled offline tiles.
* **Acceptance Criteria:**
  - Running the binary spawns zero terminal or console windows on Windows.
  - Automatically launches the default browser at `http://127.0.0.1:<port>`.
  - Displays a responsive 3D globe using bundled offline assets.
  - System tray icon allows opening the app, copying the active URL, or cleanly terminating the process.

### Milestone 2: SQLite Storage Engine, Encrypted Vault & Keyless Telemetry Ingest
* **Objectives:** SQLite WAL database setup, automated migrations, AES-256-GCM vault subsystem, and keyless background collectors for USGS Earthquakes and CelesTrak Satellites (with SGP4 propagation).
* **Acceptance Criteria:**
  - Satellite orbital paths and recent earthquake events persist to SQLite and render as interactive entities on the globe.
  - All unit/integration tests pass fully offline against mock responses in `/fixtures`.

### Milestone 3: Multi-Provider Ingest Framework & Live Observation Bus
* **Objectives:** Implement the ingestion router supporting Failover, Spread, and Merge modes. Complete adapters for ADS-B feeds (adsb.lol, airplanes.live), AISstream, NOAA SWPC, Open-Meteo, US EIA, and CAIDA IODA.
* **Acceptance Criteria:**
  - Aircraft, vessels, space weather gauges, and electrical grid mix stream concurrently to the frontend over WebSockets.
  - Simulating network dropouts triggers graceful circuit-breaker backoffs without crashing background workers.

### Milestone 4: Spatial Geometry Engine, News Aggregation & Hierarchy Fallback
* **Objectives:** Preprocessed R-Tree index (`rstar`) bundling Natural Earth boundaries and Census places. Ingest clients for GDELT DOC/GEO 2.0 and Google News RSS (`feed-rs`) with hierarchical fallback.
* **Acceptance Criteria:**
  - Clicking any location on the globe resolves `Country → State → County → City` in $< 5 \text{ ms}$ offline.
  - Context panel populates local headlines with accurate proximity provenance tags (`[Direct]`, `[Regional]`, `[Statewide]`).

### Milestone 5: Historical Analytics, Graphing & Snapshot Comparison Engine
* **Objectives:** Apache ECharts integration, time-series historical rollup queries, snapshot freezing, and dual-mode comparison engine (Calendar alignment vs. Relative day-of-period alignment).
* **Acceptance Criteria:**
  - User can freeze a scalar telemetry series into a snapshot.
  - Overlaid comparison view displays statistical divergence ($\Delta \text{Mean}$, $\Delta \text{Max}$, NRMSD).
  - Snapshot visual can be exported as a standalone PNG or CSV.

### Milestone 6: Rules Engine, Notifications & LAN Security
* **Objectives:** In-memory rule evaluation engine, native Win32 Toast notification bridge, Discord webhook dispatcher, Argon2id LAN mode authentication, and storage reclamation tools (`Clear Caches`, `Delete Snapshots`).
* **Acceptance Criteria:**
  - Seismic or space weather events matching a configured threshold trigger desktop toast notifications and Discord rich embeds.
  - LAN mode prompts remote devices for a password, issuing secure session cookies.

### Milestone 7: Final Packaging, Distribution & Documentation
* **Objectives:** Inno Setup Windows installer script (`humboldt.iss`) with AUMID toast registration, multi-stage Linux Docker build, systemd service unit, attribution page, and complete documentation.
* **Acceptance Criteria:**
  - Windows installer installs cleanly to `%LOCALAPPDATA%\Programs\Humboldt`, adds Start/Desktop shortcuts, and configures toast identity.
  - Docker container runs headlessly on Linux servers mounting persistent storage to `/data`.
  - In-app attribution screen correctly acknowledges all data providers under their respective licenses.