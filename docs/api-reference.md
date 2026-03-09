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
      "id": "record-001"
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
GET /models/{model}/versions/{version}/route
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
| `GET` | `/admin/ready` | Readiness probe |
| `GET` | `/admin/status` | Runtime status |
| `GET` | `/admin/metrics` | Prometheus metrics (`METRICS_ENABLED=true`) |
| `POST` | `/admin/registry/refresh` | Reload registry catalog/artifacts |

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
| `NOT_ROUTABLE` | `422` |
| `IDEMPOTENCY_CONFLICT` | `409` |
| `STORE_ERROR` | `502` |
| `REGISTRY_ERROR` | `502` |
| `VALIDATOR_ERROR` | `500` |
| `INTERNAL_ERROR` | `500` |
