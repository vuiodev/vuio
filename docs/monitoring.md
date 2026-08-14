# Monitoring & Observability Guide

VuIO is built with native high-availability, observability, and metrics endpoints designed for integration with Kubernetes orchestration, Prometheus, Grafana, and Loki.

---

## Health & Readiness Probes

### Liveness Probe (`GET /healthz`)

A lightweight health check that confirms the HTTP server is responsive and accepting connections.

- **Endpoint**: `GET /healthz`
- **Response**: `200 OK`
  ```json
  {
    "status": "healthy"
  }
  ```

### Readiness Probe (`GET /readyz`)

A deep probe that validates database connectivity and readiness to serve media index queries.

- **Endpoint**: `GET /readyz`
- **Response**:
  - `200 OK` if the SQLite database is reachable and healthy:
    ```json
    {
      "status": "ready"
    }
    ```
  - `503 Service Unavailable` if database access fails.

---

## Metrics Endpoints

### Prometheus Exposition Format (`GET /metrics`)

Exports real-time server telemetry formatted for Prometheus scrapers.

- **Endpoint**: `GET /metrics`
- **Content-Type**: `text/plain; version=0.0.4`
- **Query**:
  ```bash
  curl http://localhost:8080/metrics
  ```

### JSON Metrics Telemetry (`GET /metrics/json`)

Returns comprehensive runtime metrics in structured JSON for dashboards or custom telemetry collectors.

- **Endpoint**: `GET /metrics/json`
- **Content-Type**: `application/json`
- **Response Example**:
  ```json
  {
    "web_handler_metrics": {
      "browse_requests": 142,
      "cache_hits": 128,
      "cache_misses": 14,
      "cache_hit_rate_percent": 90.14,
      "average_response_time_ms": 1.2,
      "gigabytes_transferred": 14.85,
      "database_backend": "sqlite"
    }
  }
  ```

---

## DLNA Browse Caching Architecture

To achieve sub-millisecond response times even on media directories containing 1,000+ items, VuIO implements an automatic, thread-safe SOAP response cache:

- **Cache Key Signature**: The cache stores fully rendered XML responses mapped to a unique composite key:
  $$\text{Key} = (\text{ObjectID}, \text{StartingIndex}, \text{RequestedCount}, \text{ClientProfile}, \text{UpdateID})$$
- **Instant Response**: Subsequent directory navigations or scrolls from TVs and DLNA renderers are served directly from memory without SQLite queries, filesystem lookups, or string allocations.
- **Automatic Invalidation**: Whenever a background rescan or real-time filesystem event modifies the library, the global `UpdateID` counter increments, instantly invalidating stale cache entries.

---

## Related Documentation

- [Logging & Diagnostics Guide](logging.md)
- [Kubernetes Deployment Guide](kubernetes.md)
- [API Reference](api.md)
