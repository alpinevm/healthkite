# Wirebody

> Apple HealthKit, exposed honestly as JSON. Free, open-source, agent-native.

This monorepo contains the public-facing components of **Wirebody** — an iOS app that turns your iPhone into a small read-only HTTP server for your own Apple Health data. Point an MCP-aware agent (Claude Code, Codex, Cursor, etc.) or a script at it and get clean HealthKit-native JSON back.

The iOS app itself is closed-source (for now); everything an integrator needs to consume it lives here.

## Contents

| Directory | What it is |
| --- | --- |
| [`mcp/`](mcp/) | **`wirebody-mcp`** — TypeScript MCP server, MIT-licensed. Stdio transport, runs locally via `npx`. Bridges the LAN endpoints to any MCP-aware agent. |
| [`docs/`](docs/) | Astro Starlight documentation site. Concepts, API reference, MCP integration guide. |

## Quick start

```bash
# In your agent's MCP config (works in Claude Code, Codex, Cursor, Zed, Continue, ...)
{
  "mcpServers": {
    "wirebody": {
      "command": "npx",
      "args": ["-y", "wirebody-mcp"],
      "env": {
        "WIREBODY_URL": "http://<your-iPhone-LAN-IP>:<port>",
        "WIREBODY_TOKEN": "<token from the iOS app's Settings>"
      }
    }
  }
}
```

Open the Wirebody iOS app, toggle on **Local LAN Server** in Settings, copy the URL + token. Restart your agent.

Full setup guide: [`docs/quickstart.mdx`](docs/quickstart.mdx).

Docs are built with [Astro Starlight](https://starlight.astro.build/) and hosted on Railway: https://wirebody-docs-production.up.railway.app/

## Architecture

```
[ Claude Code / Codex / Cursor / ... ]
              │  stdio (JSON-RPC)
              ▼
   wirebody-mcp (Node, MIT)
              │  HTTP + bearer
              ▼
   Wirebody iOS app on your LAN
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
