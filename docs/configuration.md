# Configuration Reference

All runtime configuration is via environment variables. No config files are required.
See `.env.example` for a ready-to-copy template.

---

## Registry

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `REGISTRY_MODE` | Yes | `catalog` | Registry backend mode. Current API wiring supports `catalog`. |
| `REGISTRY_CATALOG_JSON` | Cond. | — | Inline JSON catalog content (string). |
| `REGISTRY_CATALOG_FILE` | Cond. | — | Local path to catalog JSON file. |
| `REGISTRY_CATALOG_URL` | Cond. | — | HTTP(S) URL to catalog JSON file. |
| `REGISTRY_ALLOWED_HOSTS` | Recommended | — | Comma-separated allowed hostnames (e.g. `codeberg.org`) |
| `REGISTRY_REQUIRE_HTTPS` | Recommended | `true` | Reject non-HTTPS registry URLs |
| `REGISTRY_CACHE_ENABLED` | No | `false` | Enable artifact caching |
| `REGISTRY_CACHE_TTL_SECS` | No | `300` | Cache TTL in seconds |
| `REGISTRY_ARTIFACT_MAP_ROUTE` | No | `route.json` | Filename override for route artifact |
| `REGISTRY_ARTIFACT_MAP_SCHEMA` | No | `schema.json` | Filename override for JSON Schema artifact |
| `REGISTRY_ARTIFACT_MAP_SHACL` | No | `shacl.ttl` | Filename override for SHACL artifact |
| `REGISTRY_ARTIFACT_MAP_OWL` | No | `owl.ttl` | Filename override for OWL artifact |
| `REGISTRY_ARTIFACT_MAP_OPENAPI` | No | `openapi.json` | Filename override for OpenAPI artifact |

### Catalog source selection

Exactly one of the following should be set:

- `REGISTRY_CATALOG_JSON`
- `REGISTRY_CATALOG_FILE`
- `REGISTRY_CATALOG_URL`

If none is set, startup fails.

### Catalog format

Accepted JSON shapes:

```json
[
  {
    "model": "re-indicators-specification",
    "version": "0.0.3",
    "base_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/"
  }
]
```

or

```json
{
  "models": [
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "base_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/"
    }
  ]
}
```

Rules:

- Multiple versions for the same model are allowed.
- Duplicate `(model, version)` entries are rejected.
- `base_url` (or `url`) must point to the artifact folder containing `route.json`.
- If `model` or `version` is omitted, the registry attempts to infer them from CE-RISE Codeberg URL patterns.

For SHACL behavior and artifact expectations (`shacl.ttl`), see [SHACL Validation](shacl-validation.md).

### Refresh behavior

`POST /admin/registry/refresh` re-loads the catalog source each time:

- `REGISTRY_CATALOG_URL`: re-downloads latest JSON from that URL.
- `REGISTRY_CATALOG_FILE`: re-reads the file from disk.
- `REGISTRY_CATALOG_JSON`: reuses in-memory inline catalog unless changed by process restart or runtime replacement API.

The in-memory index swap is atomic.
If the catalog cannot be loaded/parsed, refresh returns an error and the previous index remains active.
If individual model entries fail artifact resolution, refresh succeeds with per-entry errors and loads only successful entries.

## IO Adapter

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `IO_ADAPTER_ID` | Yes | — | Adapter identifier (`memory` or `http` in current API wiring) |
| `IO_ADAPTER_VERSION` | Yes | — | Adapter version (e.g. `v1`) |
| `IO_ADAPTER_BASE_URL` | Cond. | — | Base URL for the HTTP IO Adapter Service |
| `IO_ADAPTER_TIMEOUT_MS` | No | `5000` | Request timeout in milliseconds |

Notes:

- `IO_ADAPTER_ID=memory`: in-process memory store (dev/test).
- `IO_ADAPTER_ID=http`: enables `crates/io-http`; requires `IO_ADAPTER_BASE_URL`.

## Auth

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `AUTH_MODE` | No | `jwt_jwks` | Auth provider mode: `jwt_jwks`, `forward_auth`, or `none` |
| `AUTH_JWKS_URL` | Cond. | — | JWKS endpoint URL (`AUTH_MODE=jwt_jwks`) |
| `AUTH_ISSUER` | Cond. | — | Expected JWT `iss` (`AUTH_MODE=jwt_jwks`) |
| `AUTH_AUDIENCE` | Cond. | — | Expected JWT `aud` (`AUTH_MODE=jwt_jwks`) |
| `AUTH_JWKS_REFRESH_SECS` | No | `3600` | JWKS key refresh interval seconds (`AUTH_MODE=jwt_jwks`) |
| `AUTH_FORWARD_SUBJECT_HEADER` | No | `x-auth-subject` | Subject header name (`AUTH_MODE=forward_auth`) |
| `AUTH_FORWARD_ROLES_HEADER` | No | `x-auth-roles` | Comma-separated roles header (`AUTH_MODE=forward_auth`) |
| `AUTH_FORWARD_SCOPES_HEADER` | No | `x-auth-scopes` | Space-separated scopes header (`AUTH_MODE=forward_auth`) |
| `AUTH_FORWARD_TENANT_HEADER` | No | — | Tenant header name (`AUTH_MODE=forward_auth`) |
| `AUTH_FORWARD_TOKEN_HEADER` | No | — | Header containing raw token to propagate (`AUTH_MODE=forward_auth`) |
| `AUTH_ALLOW_INSECURE_NONE` | No | `false` | Must be `true` to allow `AUTH_MODE=none` (unsafe, non-production) |
| `AUTH_NONE_SUBJECT` | No | `dev-anonymous` | Subject injected in `AUTH_MODE=none` |
| `AUTH_NONE_ROLES` | No | — | Comma-separated roles injected in `AUTH_MODE=none` |
| `AUTH_NONE_SCOPES` | No | — | Space-separated scopes injected in `AUTH_MODE=none` |
| `AUTH_NONE_TENANT` | No | — | Optional tenant injected in `AUTH_MODE=none` |

Notes:

- `jwt_jwks` is for direct bearer JWT validation in this service.
- `forward_auth` is for deployments where an upstream proxy/gateway already authenticated the caller and injects identity headers.
- `none` is only for isolated dry runs and requires `AUTH_ALLOW_INSECURE_NONE=true`.
- Detailed integration guidance: [Authentication](authentication.md).

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

## OWL Validation Mode

OWL validation is enabled through the `hex-validator-owl` adapter in API wiring.

- Runtime mode: embedded profile checks (no external OWL subprocess required).
- Activation condition: validator executes when `owl.ttl` is present in resolved artifacts.
- Missing `owl.ttl`: validator skips gracefully and returns `passed=true` with no violations.
- Invalid `owl.ttl`: mapped to validator initialization error.
- Runtime execution fault: mapped to validator execution error.

Operationally this keeps deployment simple (no extra binaries), but the current path is profile-oriented and not a full generic OWL reasoner.
