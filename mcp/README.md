# wirebody-mcp

[![npm](https://img.shields.io/npm/v/wirebody-mcp.svg)](https://www.npmjs.com/package/wirebody-mcp) [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

MCP server that exposes the **Wirebody** iOS app's Local LAN Server to agents like Claude Code, Codex, Cursor, and any other MCP client.

It is a read-only proxy: every tool call becomes an HTTP `GET` against the iPhone's LAN server, with bearer-token auth, and returns the response JSON verbatim. No state, no caching, no schema translation.

## What this is for

You have Wirebody open on your iPhone. The app exposes a small HTTP API on your home Wi-Fi (`GET /workouts`, `GET /workouts/{uuid}`, etc.) emitting **HealthKit-native columnar JSON**. With this MCP server installed, your agent can:

- Browse your workouts (`list_workouts`)
- Pull the full sample-by-sample detail for a specific workout (`get_workout`)
- Discover standalone HealthKit quantity types (`list_quantity_types`)
- Export standalone quantity series such as heart rate, steps, weight, mobility metrics, and vitals (`get_quantity_series`)
- List reconciled sleep sessions with Apple sleep phase intervals (`list_sleep_sessions`)
- Fetch the same one-day overview shown by the iOS Day view (`get_day_snapshot`)
- Ask follow-up questions: "summarize this run", "what was my zone-2 time", "compare to last week"

The agent gets the same byte-identical payload the iOS app emits via Share / Copy / Push.

## Install + Configure

The canonical agent-config block (works in Claude Code, Codex, Cursor, Continue, Zed, and other MCP-aware harnesses):

```json
{
  "mcpServers": {
    "wirebody": {
      "command": "npx",
      "args": ["-y", "wirebody-mcp"],
      "env": {
        "WIREBODY_URL": "http://192.168.1.244:5606",
        "WIREBODY_TOKEN": "<bearer token from the app>"
      }
    }
  }
}
```

- **`WIREBODY_URL`** (required) — the URL shown in the Wirebody Settings → Local LAN Server row.
- **`WIREBODY_TOKEN`** (optional) — the bearer token shown next to the URL. Omit only if you've turned off **Require auth** in the app.

The iOS app must be foregrounded with the LAN server toggle ON for tools to work. iOS suspends background apps, so the server stops the moment you switch apps or lock the phone.

## Tools

| Tool | Input | Output |
| --- | --- | --- |
| `status` | none | `{ name, version, workoutCount, sampleEncoding }` |
| `list_workouts` | `limit?` (1–200, default 50), `offset?` (default 0) | `{ workouts: WorkoutSummary[], limit, offset, total, hasMore }` |
| `get_workout` | `uuid` (string, required) | Full `WorkoutDetail` JSON (sprint-11 `columnar-v1` shape: hoisted `sources`, per-stream parallel `t`/`v` arrays) |
| `list_quantity_types` | none | `{ types: QuantityTypeInfo[] }` with category, preferred unit, aggregation style, readable sample count, and first/last sample dates |
| `get_quantity_series` | `type` (string, required), `from?`, `to?`, `limit?` (1–50000, default 5000), `offset?` (default 0) | Standalone quantity series JSON in `columnar-v1` shape |
| `list_sleep_sessions` | `from?`, `to?`, `limit?` (1–365, default 30), `offset?` (default 0) | Reconciled sleep-session page with phase interval columns |
| `get_day_snapshot` | `date` (YYYY-MM-DD, required) | Single-day overview: sleep, workouts, activity totals, heart, body, and mobility; same numbers as the iOS Day view |

## Wire format

`get_workout` returns:

```json
{
  "uuid": "...",
  "workoutActivityType": "HKWorkoutActivityTypeRunning",
  "sampleEncoding": "columnar-v1",
  "startDate": "...", "endDate": "...", "duration": 1953.68,
  "totalDistance": { "value": 4855, "unit": "m" },
  "averageHeartRate": { "value": 164.8, "unit": "count/min" },
  "src": 0,
  "sources": [
    { "id": 0,
      "device": { "name": "Apple Watch", "model": "Watch", ... },
      "sourceRevision": { "source": { "name": "Cliff's Apple Watch", "bundleIdentifier": "..." },
                          "version": "26.4", "productType": "Watch6,11", "operatingSystemVersion": "26.4.0" } }
  ],
  "samples": {
    "HKQuantityTypeIdentifierHeartRate": {
      "unit": "count/min",
      "t":  [0, 5, 10, ...],     // seconds offset from workout.startDate
      "v":  [142, 144, 145, ...] // sample values in `unit`
    }
  },
  "statistics": { ... },
  "workoutEvents": [ ... ]
}
```

Field names use Apple's HealthKit identifier vocabulary verbatim. A 30-minute run is typically ~80–100 KB.

`get_quantity_series` returns the same columnar encoding for standalone HealthKit quantities:

```json
{
  "type": "HKQuantityTypeIdentifierHeartRate",
  "unit": "count/min",
  "from": "2026-05-04T00:00:00Z",
  "to": "2026-05-11T00:00:00Z",
  "sampleEncoding": "columnar-v1",
  "sources": [
    { "id": 0, "device": { "...": "..." }, "sourceRevision": { "...": "..." } }
  ],
  "samples": {
    "t": [0, 60, 120],
    "v": [72, 76, 75]
  },
  "sampleCount": 3,
  "limit": 5000,
  "offset": 0,
  "total": 3,
  "hasMore": false
}
```

`list_sleep_sessions` returns paginated nightly sessions. Wirebody reconciles overlapping `HKCategorySample` sleep records into sessions using a 2-hour gap threshold, then emits phase intervals keyed by Apple's sleep phase names:

```json
{
  "from": "2026-04-11T00:00:00Z",
  "to": "2026-05-11T00:00:00Z",
  "sampleEncoding": "columnar-v1",
  "sources": [
    { "id": 0, "device": { "...": "..." }, "sourceRevision": { "...": "..." } }
  ],
  "sessions": [
    {
      "startDate": "2026-05-10T05:23:00Z",
      "endDate": "2026-05-10T13:42:00Z",
      "inBedDuration": 29940,
      "asleepDuration": 27300,
      "awakeDuration": 1820,
      "src": 0,
      "phases": {
        "inBed": { "t": [0], "dur": [29940] },
        "asleepCore": { "t": [120, 4800], "dur": [3000, 12600] },
        "asleepDeep": { "t": [600], "dur": [2400] },
        "asleepREM": { "t": [12600], "dur": [2400] },
        "awake": { "t": [3120], "dur": [60] }
      }
    }
  ],
  "limit": 30,
  "offset": 0,
  "total": 47,
  "hasMore": true
}
```

`get_day_snapshot` returns the same aggregate model used by the Wirebody iOS Day view. The `date` input is interpreted in the iPhone's local timezone and values stay in HealthKit-native units:

```json
{
  "date": "2026-05-11",
  "timezone": "America/Chicago",
  "sampleEncoding": "columnar-v1",
  "sources": [],
  "sleep": [],
  "workouts": [],
  "activity": {
    "steps": 12438,
    "distanceMeters": 9265.4,
    "activeEnergyKcal": 742.3,
    "exerciseMinutes": 62,
    "standHours": 11,
    "flightsClimbed": 14
  },
  "heart": {
    "restingBPM": 57,
    "averageBPM": 78.4,
    "hrvSDNN": null,
    "vo2max": 44.14
  },
  "body": null,
  "mobility": null
}
```

## Errors

The MCP surfaces meaningful tool errors so agents can recover or prompt the user:

| Error | Cause |
| --- | --- |
| `Unauthorized` | `WIREBODY_TOKEN` missing or wrong. |
| `NotFound` | UUID does not match a workout. |
| `Unreachable` | iOS app is closed/backgrounded, LAN server is off, or wrong URL. |
| `ServerError` | Other 5xx from the iOS app. |

## Development

Requires Node ≥20.

```bash
git clone https://github.com/alpinevm/wirebody-mcp.git
cd wirebody-mcp
npm install
npm run build
npm test
```

Local stdio smoke test against the iPhone:

```bash
printf '%s\n' \
'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0.0.0"}}}' \
'{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
'{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"status","arguments":{}}}' \
| WIREBODY_URL='http://<ip>:<port>' \
  WIREBODY_TOKEN='<token>' \
  node dist/index.js
```

## License

MIT — see [LICENSE](LICENSE).
