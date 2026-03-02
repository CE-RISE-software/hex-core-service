# Configuration Reference

All runtime configuration is via environment variables. No config files are required.
See `.env.example` for a ready-to-copy template.

---

## Registry

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `REGISTRY_MODE` | Yes | — | Must be `url` |
| `REGISTRY_URL_TEMPLATE` | Yes | — | URL template with `{model}` and `{version}` placeholders |
| `REGISTRY_ALLOWED_HOSTS` | Recommended | — | Comma-separated allowed hostnames (e.g. `codeberg.org`) |
| `REGISTRY_REQUIRE_HTTPS` | Recommended | `true` | Reject non-HTTPS registry URLs |
| `REGISTRY_CACHE_ENABLED` | No | `false` | Enable artifact caching |
| `REGISTRY_CACHE_TTL_SECS` | No | `300` | Cache TTL in seconds |
| `REGISTRY_ARTIFACT_MAP_ROUTE` | No | `route.json` | Filename override for route artifact |
| `REGISTRY_ARTIFACT_MAP_SCHEMA` | No | `schema.json` | Filename override for JSON Schema artifact |
| `REGISTRY_ARTIFACT_MAP_SHACL` | No | `shacl.ttl` | Filename override for SHACL artifact |
| `REGISTRY_ARTIFACT_MAP_OWL` | No | `owl.ttl` | Filename override for OWL artifact |
| `REGISTRY_ARTIFACT_MAP_OPENAPI` | No | `openapi.json` | Filename override for OpenAPI artifact |

## IO Adapter

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `IO_ADAPTER_ID` | Yes | — | Adapter identifier (`memory`, `circularise`, `postgres`, …) |
| `IO_ADAPTER_VERSION` | Yes | — | Adapter version (e.g. `v1`) |
| `IO_ADAPTER_BASE_URL` | Cond. | — | Base URL for the HTTP IO Adapter Service |
| `IO_ADAPTER_TIMEOUT_MS` | No | `5000` | Request timeout in milliseconds |

## Auth

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AUTH_JWKS_URL` | Yes | — | Keycloak JWKS endpoint URL |
| `AUTH_ISSUER` | Yes | — | Expected JWT `iss` value |
| `AUTH_AUDIENCE` | Yes | — | Expected JWT `aud` value |
| `AUTH_JWKS_REFRESH_SECS` | No | `3600` | JWKS key refresh interval in seconds |

## Server

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_HOST` | No | `0.0.0.0` | Bind address |
| `SERVER_PORT` | No | `8080` | Bind port |
| `SERVER_REQUEST_MAX_BYTES` | No | `1048576` | Max request body size (1 MiB) |

## Observability

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LOG_LEVEL` | No | `info` | Tracing filter (e.g. `debug`, `info,tower_http=warn`) |
| `METRICS_ENABLED` | No | `false` | Expose `/admin/metrics` (Prometheus format) |