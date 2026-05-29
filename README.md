# Afrasyab

Afrasyab is a private Telegram bot that saves links and files to your Google Drive. Forward a YouTube link, SoundCloud track, document, or any Telegram file in a direct message; the bot downloads it and uploads it to a folder you choose on your own Google account.

Access is invite-only: a super-admin allowlists Telegram users by ID. Each user connects Google Drive through a one-time browser OAuth flow.

**Status:** v1 implemented — design in [spec](docs/superpowers/specs/2026-05-17-afrasyab-design.md); validate with the [smoke checklist](#v1-smoke-checklist-manual) before release.

## Features

- Download from **YouTube** and **SoundCloud** (via [yt-dlp](https://github.com/yt-dlp/yt-dlp)) with format choice (video / audio / best)
- Download **playlists** (configurable cap, default 25 items)
- Download **direct HTTP** file URLs
- Download **Telegram files** (documents, video, audio, photos, voice messages) from chat and upload them to Drive
- Upload to an **app-created Google Drive folder** (default **Afrasyab**; rename or create another via `/folder`)
- **Live status updates** in Telegram for each job
- **Parallel jobs** per user with a global worker limit
- **Encrypted** OAuth token storage in SQLite

## v1 smoke checklist (manual)

Run after deploy (or against a staging bot) to confirm end-to-end behavior:

1. Super-admin: `/adduser <telegram_user_id>`
2. User: `/start` → complete OAuth (creates **Afrasyab** folder on Drive automatically)
3. Send a YouTube link → choose **Video** → confirm the file appears in Drive
4. Send a direct **PDF** URL → confirm upload
5. Send a **Telegram document** under 20 MB → confirm upload
6. Send a **playlist** URL (e.g. three items) → confirm three files (within `MAX_PLAYLIST_ITEMS`)
7. **Non-allowlisted** user: message the bot → confirm access is denied
8. Revoke or invalidate the user’s Google token → user runs `/connect` → confirm re-link works

**CI parity (local or in CI):** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings` — should pass.

## Releasing

1. Ensure `main` CI is green.
2. Tag a semver release and push:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. GitHub Actions (`.github/workflows/release.yml`) will:
   - Bump `[workspace.package] version` in `Cargo.toml` on `main` to match the tag
   - Push `ghcr.io/amir-yaghoubi/afrasyab:<version>` and `:latest` (linux/amd64)
   - Create a GitHub Release with two Linux binary archives (amd64, arm64) and `SHA256SUMS`

**Docker (production):**

```bash
docker pull ghcr.io/amir-yaghoubi/afrasyab:0.2.0
```

**Bare binary installs** (Linux amd64/arm64 only) require [yt-dlp](https://github.com/yt-dlp/yt-dlp), **ffmpeg**, and **deno** on `PATH`. Production deploys should use the Docker image (includes those tools).

**Repository settings:** Actions must be allowed to write contents and packages (Settings → Actions → General).

## Limitations

- **DMs only** — the bot does not operate in groups.
- **Telegram file size:** standard Bot API allows downloads up to **~20 MB**. Larger files require sending a direct link instead (local Bot API server is a possible future improvement).
- **File size:** direct HTTP links, yt-dlp, and Drive uploads are capped at **2 GB** by default (`MAX_FILE_BYTES`). Uploads use resumable chunked transfers (bounded RAM via `DRIVE_UPLOAD_CHUNK_BYTES`, default 16 MiB).
- **Allowlist only** — unknown users cannot use the bot.
- **Single super-admin** — only one Telegram account can run admin commands.

## Tech stack

| Component | Technology |
|-----------|------------|
| Language / runtime | Rust, Tokio |
| Telegram | teloxide |
| HTTP (OAuth) | axum |
| Database | SQLite (sqlx) — jobs, users, OAuth state, pending UI |
| Downloads | yt-dlp (subprocess), reqwest (direct URLs) |
| Cloud storage | Google Drive API v3 |

## Prerequisites

- **Rust** 1.78+ (edition 2021)
- **Docker** and Docker Compose (optional, for production image)
- **yt-dlp** and **ffmpeg** (included in the application Docker image)
- A **Telegram bot token** ([@BotFather](https://t.me/BotFather))
- A **Google Cloud** project with OAuth 2.0 credentials and Google Drive API enabled
- A **VPS** (or similar) with a public HTTPS domain for OAuth callbacks

## Quick start (development)

### 1. Clone and configure

```bash
git clone <repository-url> afrasyab
cd afrasyab
cp .env.example .env
# Edit .env — see Configuration below
```

### 2. Prepare data directory

```bash
mkdir -p data
```

Migrations run automatically on startup. The database file is created at `data/afrasyab.db` when using the default `DATABASE_URL`.

### 3. Run the bot

```bash
# From the repo root so `.env` is loaded automatically
cargo run -p afrasyab-app --bin afrasyab
```

The process starts:

- Telegram updates via `TELEGRAM_MODE` — `polling` (default) or `webhook`
- OAuth HTTP server on `HTTP_BIND` (default `0.0.0.0:8080`)
- Background job workers claiming jobs from SQLite

### 4. Allowlist your Telegram account

Send your numeric Telegram user ID to the super-admin, or set yourself as super-admin and run:

```
/adduser <your_telegram_user_id>
```

Get your ID from [@userinfobot](https://t.me/userinfobot) or similar.

### 5. Onboard in Telegram

1. Open a DM with your bot.
2. `/start` → connect Google → complete browser OAuth.
3. Optional: `/folder` → rename the upload folder or create a new one.
4. Forward a link or file to test.

## Configuration

Copy `.env.example` to `.env` for local development. In production, set these in your environment or secrets manager.

| Variable | Required | Description |
|----------|----------|-------------|
| `TELEGRAM_BOT_TOKEN` | Yes | Bot token from BotFather |
| `SUPER_ADMIN_TELEGRAM_ID` | Yes | Numeric ID of the admin user |
| `GOOGLE_CLIENT_ID` | Yes | OAuth 2.0 client ID |
| `GOOGLE_CLIENT_SECRET` | Yes | OAuth 2.0 client secret |
| `PUBLIC_BASE_URL` | Yes | Public HTTPS base URL, e.g. `https://afrasyab.example.com` (no trailing slash) |
| `TOKEN_ENCRYPTION_KEY` | Yes | 32-byte key (base64 or hex) for encrypting stored refresh tokens |
| `DATABASE_URL` | Yes | SQLite URL, e.g. `sqlite:data/afrasyab.db?mode=rwc` |
| `HTTP_BIND` | No | Bind address for HTTP server (default `0.0.0.0:8080`) |
| `TELEGRAM_MODE` | No | `polling` (default) or `webhook` |
| `TELEGRAM_WEBHOOK_SECRET` | If webhook | Secret for `X-Telegram-Bot-Api-Secret-Token` (1–256 chars, `[A-Za-z0-9_-]`) |
| `DOMAIN` | Compose only | Hostname for Traefik `Host()` rule; must match `PUBLIC_BASE_URL` host |
| `ACME_EMAIL` | Compose only | Let's Encrypt registration email |
| `RUST_LOG` | No | Log filter (default `info`). Job lifecycle (`queued`, `claimed`, `downloading`, …) logs at `info` — enough for `docker compose logs -f app`. |
| `MAX_PLAYLIST_ITEMS` | No | Max playlist entries (default `25`) |
| `MAX_CONCURRENT_JOBS` | No | Global worker cap (default `4`) |
| `MAX_FILE_BYTES` | No | Max file size for direct HTTP, yt-dlp, and Drive upload (default `2147483648`, 2 GiB) |
| `DRIVE_UPLOAD_CHUNK_BYTES` | No | Resumable upload chunk size (default `16777216`, 16 MiB; must be ≥ 256 KiB and a multiple of 256 KiB) |
| `TMPDIR` | No | Temp download directory (default system temp) |

### Google Cloud setup

1. Create a project in [Google Cloud Console](https://console.cloud.google.com/).
2. Enable **Google Drive API**.
3. Configure **OAuth consent screen** (External or Internal; add test users if External).
4. Create **OAuth 2.0 Client ID** (Web application).
5. Add authorized redirect URI:

   ```
   https://<your-domain>/oauth/google/callback
   ```

6. Scope needed: `https://www.googleapis.com/auth/drive.file` only (per-file access to folders and files the app creates — no broader Drive scope required).

### Telegram setup

1. Create a bot with [@BotFather](https://t.me/BotFather).
2. Copy the token into `TELEGRAM_BOT_TOKEN`.
3. Users interact only via **private chat** with the bot.

## Development

### Repository layout

```
afrasyab/
├── crates/
│   ├── domain/       # Core types and logic
│   ├── storage/      # SQLite (persistent + ephemeral state)
│   ├── core/         # Config + AppState
│   ├── downloader/   # yt-dlp + HTTP
│   ├── drive/        # Google Drive
│   ├── telegram/     # Bot handlers
│   ├── oauth/        # OAuth HTTP routes
│   └── app/          # Main binary
├── migrations/       # sqlx SQL migrations
├── docker-compose.yml
├── Dockerfile
└── docs/superpowers/specs/
```

### Common commands

```bash
# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Unit and integration tests
cargo test --all-features

# Update sqlx query metadata (after SQL changes)
cargo sqlx prepare --workspace
```

### Writing tests

- **Unit tests** live next to logic in each crate (`domain`, `downloader`, etc.).
- **Integration tests** use in-memory SQLite (`sqlite::memory:`); Google Drive and Telegram are mocked via traits.
- **yt-dlp** tests assert constructed CLI arguments without hitting the network.
- Live network tests are `#[ignore]` and run manually when needed.

### Adding a migration

```bash
sqlx migrate add <description>
sqlx migrate run
cargo sqlx prepare --workspace
```

## Deployment (production)

### Overview

Run on a VPS with a public domain and HTTPS. **Traefik** terminates TLS (Let's Encrypt) and proxies to the app on port `8080`. The app serves OAuth, `/health`, and (in webhook mode) `POST /telegram/webhook`.

Local development uses `cargo run` with default **`TELEGRAM_MODE=polling`** — no Traefik required.

### Prerequisites

1. DNS **A/AAAA** for your hostname → VPS public IP.
2. Firewall allows inbound **80** and **443**.
3. Google OAuth redirect URI: `https://<your-domain>/oauth/google/callback`.
4. `.env` with all required variables (see `.env.example`).

Production example (hostname must match in both `PUBLIC_BASE_URL` and `DOMAIN`):

```bash
PUBLIC_BASE_URL=https://afrasyab.example.com
DOMAIN=afrasyab.example.com
ACME_EMAIL=admin@example.com
TELEGRAM_MODE=webhook
TELEGRAM_WEBHOOK_SECRET=<openssl rand -hex 32>
# ... TELEGRAM_BOT_TOKEN, Google OAuth, TOKEN_ENCRYPTION_KEY, etc.
```

### Deploy with Docker Compose

```bash
mkdir -p data
docker compose up -d --build
```

Compose stack:

- **`traefik`** — ports `80`/`443`, ACME HTTP-01, routes `Host(${DOMAIN})` → app
- **`app`** — Afrasyab binary; **not** exposed on the host (internal `8080` only)
- **`./data`** → `/data` (SQLite at `afrasyab.db`)
- **host `/tmp`** → `/tmp` (per-job downloads under `afrasyab/<job_id>/`; deleted when the job finishes)

Verify:

```bash
curl -fsS "https://${DOMAIN}/health"
# ok
```

### Post-deploy

1. `docker compose logs -f app` — confirm webhook registration in webhook mode.
2. DM the bot as super-admin: `/listusers`.
3. `/adduser <telegram_user_id>` for each user.
4. Each user: `/start` → Google OAuth → test a link.

### Upgrades

```bash
git pull
docker compose build app
docker compose up -d app
# Migrations run automatically on app startup
```

## Admin guide

| Command | Example | Description |
|---------|---------|-------------|
| `/adduser` | `/adduser 123456789` | Grant access |
| `/removeuser` | `/removeuser 123456789` | Revoke access |
| `/listusers` | `/listusers` | Show allowlisted IDs |

Only the Telegram account matching `SUPER_ADMIN_TELEGRAM_ID` can run these commands.

## User guide

| Command | Description |
|---------|-------------|
| `/start` | Onboarding and connection status |
| `/connect` | Re-link Google if tokens expired |
| `/folder` | Rename upload folder or create a new one |
| `/status` | View recent jobs |
| `/help` | Usage help |

**Saving content:** forward or send a link, playlist, or file in DM. For YouTube/SoundCloud links, tap a format button when prompted.

## Troubleshooting

| Problem | What to check |
|---------|----------------|
| OAuth redirect error | Redirect URI in Google Console matches `{PUBLIC_BASE_URL}/oauth/google/callback` exactly |
| "No access" | User not in allowlist — super-admin runs `/adduser` |
| "Run /connect" | Token revoked or expired — user re-authenticates |
| Download failed | yt-dlp/ffmpeg/Deno in container; URL supported; check app logs |
| File too large (Telegram) | Bot API 20 MB limit — send a direct URL instead |
| Upload failed | Drive quota; run `/folder` if folder missing; user granted `drive.file` scope |

### VPS scratch disk & YouTube runtime

Job downloads use the container temp directory (`/tmp` on the host when using the default Compose mount). Keep **at least ~2 GiB free** on host `/tmp` for concurrent jobs:

```bash
df -h /tmp
```

Optional env vars (in `.env` or Compose `environment`):

| Variable | Default | Meaning |
|----------|---------|---------|
| `JOB_SCRATCH_MIN_FREE_MB` | `512` | Workers pause claiming jobs below this free space |
| `JOB_SCRATCH_RESUME_FREE_MB` | `1024` | Workers resume after free space reaches this |

When scratch space is low, logs show `scratch disk pressure: workers paused` and jobs stay **Queued** until space recovers (`scratch disk pressure cleared: workers resumed`).

After pulling image changes, rebuild so **Deno** and `/etc/yt-dlp.conf` are present:

```bash
docker compose build --no-cache app && docker compose up -d app
docker compose exec app deno --version
docker compose exec app yt-dlp --version
```

## License

TBD — add license file before public release.

## Related documents

- [Design specification](docs/superpowers/specs/2026-05-17-afrasyab-design.md)
- [SQLite storage design](docs/superpowers/specs/2026-05-17-afrasyab-sqlite-design.md)
- Implementation plan: `docs/superpowers/plans/2026-05-17-afrasyab.md`
