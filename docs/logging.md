# Logging & Diagnostics Guide

VuIO is designed to run cleanly by default. Standard startup displays a concise terminal card containing essential server status while routing detailed background execution traces to rolling log files.

---

## Default Log Files

Detailed logs (`INFO` level and above, including errors and warnings) are automatically preserved in rotating log files on the local filesystem.

| Platform | Default Log Location |
|---|---|
| **Windows** | `[exe dir]\config\logs\vuio.log` |
| **Linux** | `~/.local/state/vuio/vuio.log` (or `/var/log/vuio/vuio.log` via systemd) |
| **macOS** | `~/Library/Logs/vuio/vuio.log` (or `./config/logs/vuio.log`) |
| **Docker** | `/data/logs/vuio.log` (or `/config/logs/vuio.log`) |

---

## Verbose Console Logs

If you want to view real-time debug traces directly on standard output, use one of the following approaches:

### 1. Command-Line Flag (`--debug`)

```bash
./vuio --debug /path/to/media
```

### 2. Environment Variable (`RUST_LOG`)

Fine-tune tracing severity per module using standard Rust `env_logger` directives:

```bash
# General debug output
RUST_LOG=debug ./vuio /path/to/media

# Target specific modules
RUST_LOG=vuio_core::ssdp=trace,vuio_cast=debug ./vuio /path/to/media
```

---

## Custom Log Destinations & Levels

Configure log levels and output destinations directly from the CLI:

### Custom Log File Path

```bash
./vuio --log-file /var/log/custom-vuio.log /path/to/media
```

### Custom Log Level

Supported levels: `off`, `error`, `warn`, `info`, `debug`, `trace`.

```bash
./vuio --log-level debug /path/to/media
```

---

## Log Streaming HTTP Endpoint (`/logs`)

VuIO includes a built-in log scraping endpoint optimized for pull-based ingestion agents such as **Grafana Loki**, **Grafana Alloy**, and **Vector**:

- **Endpoint**: `GET /logs`
- **Query Parameter**: `limit` (default: `100`, maximum: `5000`)
- **Query Example**:
  ```bash
  curl http://localhost:8080/logs?limit=50
  ```
- **Response**: `200 OK` with raw plaintext log lines.

---

## Related Documentation

- [Monitoring & Observability Guide](monitoring.md)
- [Configuration Reference](configuration.md)
- [API Reference](api.md)
