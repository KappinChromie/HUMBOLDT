# HUMBOLDT Operational Runbook

This runbook provides deployment, administration, configuration, and troubleshooting procedures for **HUMBOLDT: Analytics & Real-time Global Information System**.

---

## 1. Environment Variables & CLI Options

HUMBOLDT can be configured entirely via environment variables, CLI flags, or the in-app Settings modal:

| Variable | Default | Description |
| :--- | :--- | :--- |
| `HUMBOLDT_HEADLESS` | `0` | Disables the system tray icon and browser auto-launch. Recommended for Docker and Linux servers. |
| `HUMBOLDT_DATA_DIR` | Platform default | Absolute path for application data, SQLite database, logs, and cache files. |
| `HUMBOLDT_BIND_HOST`| `127.0.0.1` | Network interface to bind to (`0.0.0.0` binds to all LAN interfaces). |
| `HUMBOLDT_PORT` | `8374` | Starting port for Axum HTTP daemon hunting range. |
| `HUMBOLDT_PORT_END` | `8378` | Ending port for hunting range. |
| `HUMBOLDT_LAN_MODE` | `0` | Enables LAN remote access with Argon2id session authentication. |
| `RUST_LOG` | `info` | Logging filter directives (e.g., `info,humboldt_server=debug,humboldt_collectors=trace`). |

---

## 2. Linux Service Deployment (systemd)

HUMBOLDT includes a pre-configured production systemd service unit located at `installer/humboldt.service`.

### 2.1 Installation
```bash
# 1. Create dedicated service user and persistent directory
sudo useradd -r -s /bin/false -d /var/lib/humboldt humboldt
sudo mkdir -p /var/lib/humboldt
sudo chown -R humboldt:humboldt /var/lib/humboldt

# 2. Copy production binary
sudo cp target/release/humboldt /usr/local/bin/humboldt
sudo chmod +x /usr/local/bin/humboldt

# 3. Install and reload systemd unit
sudo cp installer/humboldt.service /etc/systemd/system/humboldt.service
sudo systemctl daemon-reload

# 4. Enable and start service
sudo systemctl enable --now humboldt.service
```

### 2.2 Operational Commands
```bash
# Check service health and status
sudo systemctl status humboldt.service

# View live streaming journal logs
sudo journalctl -u humboldt.service -f

# Gracefully restart daemon
sudo systemctl restart humboldt.service
```

---

## 3. Docker & Container Operations

### 3.1 Docker Compose Deployment
HUMBOLDT provides a production multi-stage `Dockerfile` and `docker-compose.yml`:

```bash
# Build and launch in detached mode
docker compose up -d --build

# Inspect container health
docker compose ps

# Tail runtime logs
docker compose logs -f humboldt

# Stop service
docker compose down
```

### 3.2 Volume Persistence
Application state, encrypted vault secrets, and historical SQLite observations persist inside the `humboldt_data` named Docker volume mounted at `/data`.

---

## 4. Vault & Secrets Administration

HUMBOLDT stores external API keys and sensitive webhook URLs inside an **AES-256-GCM** encrypted vault in SQLite.

### 4.1 Setting Secrets via Web Interface
1. Click the **Settings (⚙)** button in the top navigation bar.
2. Under **Notification & Alert Routing**, paste your Discord webhook URL into the input field.
3. Click **Encrypt & Save**.

### 4.2 Setting Secrets via REST API
```bash
# Store Discord Webhook URL
curl -X POST http://127.0.0.1:8374/api/v1/vault \
  -H "Content-Type: application/json" \
  -d '{"key_name": "DISCORD_WEBHOOK_URL", "secret_value": "https://discord.com/api/webhooks/..."}'

# Store Optional Open-Meteo Commercial Key
curl -X POST http://127.0.0.1:8374/api/v1/vault \
  -H "Content-Type: application/json" \
  -d '{"key_name": "OPENMETEO_API_KEY", "secret_value": "your-api-key"}'
```

---

## 5. LAN Security & Authentication

When HUMBOLDT is deployed on a homelab server or LAN workstation, remote devices connecting across your local network must authenticate.

### 5.1 Setting the LAN Password
```bash
# Set LAN admin password
curl -X POST http://127.0.0.1:8374/api/v1/auth/password \
  -H "Content-Type: application/json" \
  -d '{"password": "YourStrongPassword123"}'
```

### 5.2 Enabling LAN Access
```bash
curl -X POST http://127.0.0.1:8374/api/v1/auth/mode \
  -H "Content-Type: application/json" \
  -d '{"enabled": true}'
```

### 5.3 Remote Client Authentication Flow
Remote clients accessing `http://<server-ip>:8374` will receive HTTP `401 Unauthorized`. To log in:
```bash
curl -c cookies.txt -X POST http://<server-ip>:8374/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"password": "YourStrongPassword123"}'
```
* The response sets a session cookie: `Set-Cookie: humboldt_session=<token>; HttpOnly; SameSite=Strict; Path=/`.
* Modern browsers will store and transmit this cookie automatically across plain HTTP LAN connections.

### 5.4 Brute-Force Rate Limiting Recovery
Failed login attempts incur escalating IP-based delays ($1\text{s}, 2\text{s}, 4\text{s}, \dots, 60\text{s}$). If an operator accidentally triggers an HTTP `429 Too Many Requests` lock, wait 60 seconds for the rate limiter bucket to cool down before retrying.

---

## 6. Alerts & Notification Routing

### 6.1 Configuring Alert Rules
Alert rules evaluate incoming observations in real-time. Rules can be configured in the UI or via REST API:

```bash
# Example: Alert on earthquakes magnitude > 6.0
curl -X POST http://127.0.0.1:8374/api/v1/alerts/rules \
  -H "Content-Type: application/json" \
  -d '{
    "id": "quake-major",
    "name": "Major Seismic Event (M6.0+)",
    "enabled": true,
    "layer": "quake",
    "metric_name": "magnitude",
    "operator": "GreaterThan",
    "threshold": 6.0,
    "cooldown_seconds": 1800,
    "notify_desktop": true,
    "notify_discord": true
  }'
```

### 6.2 Testing Notification Channels
```bash
# Test Native Windows Toast
curl -X POST http://127.0.0.1:8374/api/v1/alerts/test-toast \
  -H "Content-Type: application/json" \
  -d '{"title": "HUMBOLDT Test", "message": "Toast notifications operational."}'

# Test Discord Webhook Embed
curl -X POST http://127.0.0.1:8374/api/v1/alerts/test-discord \
  -H "Content-Type: application/json" \
  -d '{}'
```

---

## 7. Storage Maintenance & Database Pruning

HUMBOLDT enforces an automated **10 GB disk cap**. The background retention worker runs every 10 minutes to clean old observations and generate hourly rollups.

### 7.1 Manual Maintenance Endpoints
Operators can manually trigger storage reclamation through the UI Settings modal or REST API:

```bash
# 1. Clear Ephemeral Caches (purges dead-reckoning tracks & news cache)
curl -X POST http://127.0.0.1:8374/api/v1/storage/clear-cache

# 2. Delete All Analytical Snapshots
curl -X POST http://127.0.0.1:8374/api/v1/storage/delete-snapshots

# 3. Compact Database (executes SQLite VACUUM to return disk pages to the OS)
curl -X POST http://127.0.0.1:8374/api/v1/storage/compact
```

---

## 8. Backup & Disaster Recovery

### 8.1 Online SQLite Backup
Because HUMBOLDT operates in SQLite WAL mode, hot backups can be taken safely without stopping the service:

```bash
# Online hot backup using sqlite3 CLI
sqlite3 /var/lib/humboldt/data/humboldt.db ".backup /backup/humboldt-$(date +%F).db"
```

### 8.2 Restoring from Backup
```bash
# 1. Stop service
sudo systemctl stop humboldt.service

# 2. Restore database file
sudo cp /backup/humboldt-2026-11-10.db /var/lib/humboldt/data/humboldt.db
sudo chown humboldt:humboldt /var/lib/humboldt/data/humboldt.db

# 3. Restart service
sudo systemctl start humboldt.service
```
