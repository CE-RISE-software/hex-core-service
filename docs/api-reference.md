# API Reference

## Base URL

```text
https://<host>/
```

## Authentication

All endpoints except `GET /admin/health` require authentication.

```http
Authorization: Bearer <access_token>
```

Auth modes and integration patterns are described in [Authentication](authentication.md).

## End-to-End Operation Examples

### Validate

```text
POST /models/{model}/versions/{version}:validate
```

Example request:

```bash
curl -X POST "https://<host>/models/re-indicators-specification/versions/0.0.3:validate" \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "payload": {
      "id": "record-001",
      "name": "example"
    }
  }'
```

Example response (`200`):

```json
{
  "passed": true,
  "results": []
}
```

### Create

```text
POST /models/{model}/versions/{version}:create
```

Example request:

```bash
curl -X POST "https://<host>/models/re-indicators-specification/versions/0.0.3:create" \
  -H "Authorization: Bearer <token>" \
  -H "Idempotency-Key: 7f8d4d5e-1fcb-4eab-a4bb-2af7ca0f7f12" \
  -H "Content-Type: application/json" \
  -d '{
    "payload": {
      "id": "record-001",
      "name": "example"
    }
  }'
```

Example response (`200`):

```json
{
  "id": "record-001",
  "model": "re-indicators-specification",
  "version": "0.0.3",
  "payload": {
    "id": "record-001",
    "name": "example"
  }
}
```

### Query

```text
POST /models/{model}/versions/{version}:query
```

Example request:

```bash
curl -X POST "https://<host>/models/re-indicators-specification/versions/0.0.3:query" \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "where": [
        { "field": "id", "op": "eq", "value": "record-001" },
        { "field": "payload.record_scope", "op": "eq", "value": "product" }
      ],
      "sort": [
        { "field": "created_at", "direction": "desc" }
      ],
      "limit": 25,
      "offset": 0
    }
  }'
```

Example response (`200`):

```json
{
  "records": [
    {
      "id": "record-001",
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "payload": {
        "id": "record-001",
        "name": "example"
      }
    }
  ]
}
```

Supported query operators:

- `eq`
- `ne`
- `in`
- `contains`
- `exists`
- `gt`
- `gte`
- `lt`
- `lte`

Field path rules:

- Storage/root fields: `id`, `model`, `version`, `created_at`, `updated_at`
- Payload fields under `payload`, for example `payload.record_scope`
- Array positions may be addressed with brackets, for example `payload.applied_schemas[0].schema_url`

Query v1 constraints:

- `where` is AND-only
- no OR groups or nested boolean trees
- no backend-specific raw query fragments

## Public Introspection

### List available models

```text
GET /models
```

Example response (`200`):

```json
{
  "models": [
    {
      "id": "re-indicators-specification",
      "version": "0.0.3"
    }
  ]
}
```

### Get model artifacts

```text
GET /models/{model}/versions/{version}/schema
GET /models/{model}/versions/{version}/shacl
GET /models/{model}/versions/{version}/owl
```

Returns raw artifact content when available.

### OpenAPI document

```text
GET /openapi.json
```

## Admin Endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/admin/health` | Liveness probe |
| `GET` | `/admin/version` | Service/OpenAPI version probe |
| `GET` | `/admin/models/count` | Number of currently indexed models |
| `GET` | `/admin/ready` | Readiness probe |
| `GET` | `/admin/status` | Runtime status |
| `GET` | `/admin/metrics` | Prometheus metrics (`METRICS_ENABLED=true`) |
| `POST` | `/admin/registry/refresh` | Reload registry catalog/artifacts |

Version response shape:

```json
{
  "service": "hex-core-service",
  "service_version": "0.1.0",
  "openapi_version": "0.0.2"
}
```

Model count response shape:

```json
{
  "models_count": 3
}
```

Refresh response shape:

```json
{
  "refreshed_at": "2026-03-03T18:12:45Z",
  "models_found": 3,
  "errors": []
}
```

## Error Format

All errors use a JSON envelope:

```json
{
  "code": "MODEL_NOT_FOUND",
  "message": "Model re-indicators-specification v0.0.3 not found in registry",
  "details": null
}
```

Common mappings:

| Code | HTTP |
|---|---|
| `MODEL_NOT_FOUND` | `404` |
| `VALIDATION_FAILED` | `422` |
| `IDEMPOTENCY_CONFLICT` | `409` |
| `STORE_ERROR` | `502` |
| `REGISTRY_ERROR` | `502` |
| `VALIDATOR_ERROR` | `500` |
| `INTERNAL_ERROR` | `500` |
