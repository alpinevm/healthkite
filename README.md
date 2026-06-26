# HealthKite MCP

> Apple HealthKit, exposed honestly as JSON. Free, open-source, agent-native.

This monorepo contains the public-facing components of **HealthKite MCP** — an iOS app that turns your iPhone into a small read-only, authenticated LAN endpoint for your own Apple Health data. Point an MCP-aware agent (Claude Code, Codex, Cursor, etc.) at it and get clean HealthKit-native JSON back without a cloud backend.

The iOS app itself is closed-source (for now); everything an integrator needs to consume it lives here.

## Contents

| Directory | What it is |
| --- | --- |
| [`mcp/`](mcp/) | **`healthkite-mcp`** — Rust MCP server, MIT-licensed. Stdio transport, installed with Cargo. Uses Bonjour/mDNS discovery and TLS-PSK to bridge the iOS app to any MCP-aware agent. |
| [`docs/`](docs/) | Astro Starlight documentation site. Concepts, API reference, MCP integration guide. |

## Quick start

1. Install the MCP server:

   ```bash
   cargo install --git https://github.com/alpinevm/healthkite healthkite-mcp
   ```

   To upgrade an existing install, run the same command with `--force`.

2. Add it to your MCP client config:

   ```json
   {
     "mcpServers": {
       "healthkite-mcp": {
         "command": "healthkite-mcp",
         "env": {
           "HEALTHKITE_TOKEN": "<pairing secret from HealthKite MCP Settings>"
         }
       }
     }
   }
   ```

   If your MCP client cannot find `healthkite-mcp`, either add Cargo's bin directory to `PATH` or use the absolute command path shown by `which healthkite-mcp`.

3. Open the HealthKite MCP iOS app, toggle on **Local LAN Server** in Settings, copy the **Pairing Secret**, and restart your agent. The MCP server discovers the phone over Bonjour; no URL is copied.

Prerequisites: Rust/Cargo plus OpenSSL development libraries available to Cargo (`libssl-dev` and `pkg-config` on Debian/Ubuntu; Homebrew `openssl@3` on macOS if needed).

Full setup guide: [`docs/quickstart.mdx`](docs/quickstart.mdx).

Docs are built with [Astro Starlight](https://starlight.astro.build/) and hosted on Railway: https://docs.healthkite.app/

## Architecture

```
[ Claude Code / Codex / Cursor / ... ]
              │  stdio (JSON-RPC)
              ▼
   healthkite-mcp (Rust, MIT)
              │  Bonjour discovery + TLS-PSK
              ▼
   HealthKite MCP iOS app on your LAN
              │  HealthKit
              ▼
        Apple HealthKit
```

## What's exposed

| Endpoint | What it returns |
| --- | --- |
| `GET /` | Status snapshot |
| `GET /workouts` | Paginated workout summaries |
| `GET /workouts/{uuid}` | Full columnar workout detail |
| `GET /quantity-types` | Catalog of HealthKit quantity types |
| `GET /quantity/{type}` | Standalone quantity series, columnar |
| `GET /sleep` | Reconciled nightly sleep sessions |
| `GET /day-snapshot/{date}` | Single-day overview (workouts + sleep + activity + vitals + body + mobility) |

Wire format is HealthKit-native (`HKQuantityTypeIdentifierHeartRate`, `HKWorkoutActivityTypeRunning`, etc.) — Apple's own vocabulary, no invented schema. Sample series use a [columnar shape](docs/concepts/columnar-encoding.mdx) for ~10× compression vs per-sample-dict JSON.

## Design rules

- **HealthKit-native field names.** Every key is Apple's own identifier.
- **HealthKit-native units.** No conversion at the wire layer.
- **Lossless.** Per-sample resolution, no Min/Avg/Max bucketing.
- **Compact.** Columnar encoding.
- **Read-only.** No HealthKit writes from the app.
- **No backend.** Your iPhone is the API.

## License

The contents of this repository are MIT-licensed. See [`mcp/LICENSE`](mcp/LICENSE).
