# Installing Helix

This document covers the current installable release shape for Helix.

## Current Release Shape

Release archives include a `helix` launcher plus the Helix API server binary:

- `helix` and `helix-api` on Linux and macOS
- `helix.cmd` and `helix-api.exe` on Windows

Release archives also include the built UI under `ui/dist`. The API server can
serve that static UI when `HELIX_UI_DIST` points at the built directory. The
archive and install helpers create launchers that set `HELIX_UI_DIST`
automatically.

## From Source

```bash
./scripts/setup_local.sh
./scripts/run_local.sh
```

Containerized local run with Postgres:

```bash
./scripts/run_compose.sh
```

Default local addresses:

- API: `http://127.0.0.1:3000`
- UI: `http://127.0.0.1:5173`
- Compose UI/API: `http://127.0.0.1:3000`

The Compose path applies SQL files under `migrations/` on first database
initialization and starts the packaged API with the built UI served by the API
process. It also sets `HELIX_AUTO_MIGRATE=true`, so the API reapplies the
bundled idempotent schema on restart. The resulting `DATABASE_URL` enables
durable Postgres-backed intelligence desk state, automation state, policy config
snapshots, redacted credential metadata, and autopilot guard snapshots. Mutating
operator actions also write durable audit records. Without `DATABASE_URL`, the
API falls back to seeded in-memory state.

## Prebuilt Binaries

GitHub release assets are produced by `.github/workflows/release-binaries.yml`
for the available Linux, macOS, and Windows runners.

Unix-like systems:

```bash
curl -fsSL https://raw.githubusercontent.com/TheDarkLightX/Helix-A-Next-Gen-Event-Driven-Multi-Agent-Platform/main/scripts/install_helix.sh | bash
```

Windows PowerShell:

```powershell
iwr https://raw.githubusercontent.com/TheDarkLightX/Helix-A-Next-Gen-Event-Driven-Multi-Agent-Platform/main/scripts/install_helix.ps1 -OutFile install_helix.ps1
.\install_helix.ps1
```

To install a specific release tag:

```bash
HELIX_VERSION=v2.0.4 ./scripts/install_helix.sh
```

```powershell
$env:HELIX_VERSION = "v2.0.4"
.\install_helix.ps1
```

For shared deployments, start the API with bearer-token auth enabled:

```bash
HELIX_AUTH_REQUIRED=true HELIX_API_TOKEN="$(openssl rand -hex 32)" helix
```

For an installed binary pointed at a fresh Postgres database, enable startup
migrations explicitly:

```bash
DATABASE_URL=postgres://helix:secret@127.0.0.1:5432/helix HELIX_AUTO_MIGRATE=true helix
```

Credential vault writes require Postgres plus a 32-byte hex AES-GCM key:

```bash
export HELIX_ENCRYPTION_KEY="$(openssl rand -hex 32)"
```

The installed UI has an `API_AUTH` token field in the top bar. API requests use
that token as `Authorization: Bearer <token>`.

## Release Gates

The CI workflow runs the release verifier:

```bash
bash scripts/verify_release.sh
```

That gate covers formal core models, deterministic-agent models, Rust core/API
tests, Lean proofs, UI build, and the AssemblyScript SDK build/tests. The CI
workflow installs the ESSO verifier at the pinned commit recorded in the
workflow so the formal backend is not an implicit local-only dependency.

## Remaining Install Work

The current binary packaging is a practical first step, not the final operator
experience. The next install milestone is for the `helix` launcher to select a
storage profile, run migrations, open the browser, and report verification/build
metadata at startup.
