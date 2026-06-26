# healthkite-mcp

Rust MCP server exposing the **HealthKite MCP** iOS app's Local LAN Server to MCP clients.

The server uses the serverless LAN discovery + authenticated channel design:

- derive discovery/auth material from one shared root secret (`HEALTHKITE_TOKEN` or `HEALTHKITE_ROOT`),
- discover the iOS app with DNS-SD/mDNS using `_healthkite-mcp._tcp.local.`,
- connect only over TLS-PSK,
- never send the shared secret as an HTTP bearer token.

## Install

Install from GitHub with Cargo:

```bash
cargo install --git https://github.com/alpinevm/healthkite healthkite-mcp
```

Upgrade/reinstall:

```bash
cargo install --git https://github.com/alpinevm/healthkite healthkite-mcp --force
```

Install from a local checkout:

```bash
# from the repository root
cargo install --path mcp --force

# or from this mcp/ directory
cargo install --path . --force
```

Prerequisites:

- Rust stable with Cargo.
- OpenSSL development libraries available to Cargo:
  - Debian/Ubuntu: `libssl-dev` and `pkg-config`.
  - macOS: Homebrew `openssl@3` if your system OpenSSL is not discoverable.

Confirm the installed binary:

```bash
healthkite-mcp
```

With no `HEALTHKITE_TOKEN`, the binary exits with a configuration error. That is expected; MCP clients provide the environment variable in their config.

## Configure

Add the server to your MCP client config:

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

If the MCP client cannot find `healthkite-mcp`, either add Cargo's bin directory to `PATH` or use the absolute path shown by:

```bash
which healthkite-mcp
```

Environment:

- `HEALTHKITE_TOKEN` / `HEALTHKITE_ROOT` — shared root secret used for HKDF derivation.
- `HEALTHKITE_SERVICE_TYPE` — optional DNS-SD service type override; default `_healthkite-mcp._tcp.local.`.
- `HEALTHKITE_DISCOVERY_TIMEOUT_MS` — optional mDNS browse timeout; default `3000`.

`HEALTHKITE_URL` is intentionally unsupported; the MCP server always discovers the iOS app by Bonjour/mDNS and authenticates the TCP session with TLS-PSK.

Derived values:

- `discovery_id = HKDF-SHA256(root, info = "healthkite-mcp:discovery:v1")`, first 16 bytes.
- `psk = HKDF-SHA256(root, info = "healthkite-mcp:auth:v1")`, 32 bytes.
- DNS-SD instance label = lowercase base32-no-padding of `discovery_id`.
- PSK identity = the same instance label bytes.

## Tools

| Tool | Input | Output |
| --- | --- | --- |
| `status` | none | `{ name, version, workoutCount, sampleEncoding }` |
| `list_workouts` | `limit?` (1–200, default 50), `offset?` (default 0) | `{ workouts: WorkoutSummary[], limit, offset, total, hasMore }` |
| `get_workout` | `uuid` (string, required) | Full `WorkoutDetail` JSON in HealthKite MCP `columnar-v1` shape |
| `list_quantity_types` | none | `{ types: QuantityTypeInfo[] }` |
| `get_quantity_series` | `type` (string, required), `from?`, `to?`, `limit?` (1–50000, default 5000), `offset?` (default 0) | Standalone quantity series JSON in `columnar-v1` shape |
| `list_sleep_sessions` | `from?`, `to?`, `limit?` (1–365, default 30), `offset?` (default 0) | Reconciled sleep-session page with phase interval columns |
| `get_day_snapshot` | `date` (YYYY-MM-DD, required) | Single-day overview: sleep, workouts, activity totals, heart, body, and mobility |

## Development

```bash
cargo test
cargo build
```


## Errors

HealthKite MCP downstream errors are surfaced as MCP JSON-RPC errors with `error.code = -32600` and `error.data.code` set to one of:

- `Unauthorized`
- `NotFound`
- `BadDate`
- `Unreachable`
- `ServerError`

Invalid tool arguments use JSON-RPC `-32602`; unknown tools/methods use `-32601`.

## License

MIT — see [LICENSE](LICENSE).
