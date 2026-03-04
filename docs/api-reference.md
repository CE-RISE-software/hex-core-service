# API Reference

---

## Base URL

```
https://<host>/
```

---

## Authentication

All endpoints except `/admin/health` require a valid Keycloak-issued Bearer token:

```
Authorization: Bearer <access_token>
```

---

## Model Operations

### Validate a payload

```
POST /models/{model}/versions/{version}:validate
```

See [SHACL Validation](shacl-validation.md) for SHACL-specific execution details, supported constraints, and result semantics.

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Authorization` | Yes | Bearer token |

**Body**

```json
{
  "payload": { ... }
}
```

**Response `200`**

```json
{
  "passed": true,
  "results": []
}
```

---

### Create a record

```
POST /models/{model}/versions/{version}:create
```

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Authorization` | Yes | Bearer token |
| `Idempotency-Key` | Yes | Client-generated unique key |

**Body**

```json
{
  "payload": { ... }
}
```

**Response `200`**

```json
{
  "id": "record-abc123",
  "model": "product-passport",
  "version": "1.2.0",
  "payload": { ... }
}
```

---

### Query records

```
POST /models/{model}/versions/{version}:query
```

**Headers**

| Header | Required | Description |
|--------|----------|-------------|
| `Authorization` | Yes | Bearer token |

**Body**

```json
{
  "filter": { ... }
}
```

**Response `200`**

```json
{
  "records": [ ... ]
}
```

---

## Public Introspection

### List available models

```
GET /models
```

**Response `200`**

```json
{
  "models": [
    { "id": "product-passport", "version": "1.2.0" }
  ]
}
```

---

### Get artifact

```
GET /models/{model}/versions/{version}/schema
GET /models/{model}/versions/{version}/shacl
GET /models/{model}/versions/{version}/owl
GET /models/{model}/versions/{version}/route
```

Returns the raw artifact text. `404` if the artifact is not present for that model version.

---

### OpenAPI document

```
GET /openapi.json
```

---

## Admin Endpoints

All `/admin/*` endpoints require token validation and/or network-level access controls.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/admin/health` | Liveness probe — always `200` if the process is up |
| `GET` | `/admin/ready` | Readiness probe — `200` when registry index is loaded |
| `GET` | `/admin/status` | Runtime status: models indexed, uptime |
| `GET` | `/admin/metrics` | Prometheus metrics (requires `METRICS_ENABLED=true`) |
| `POST` | `/admin/registry/refresh` | Re-discover artifacts and atomically reload index |
| `GET` | `/admin/config` | Redacted configuration dump (optional) |
| `POST` | `/admin/cache/clear` | Clear artifact cache only (optional) |

### Refresh response

```json
{
  "refreshed_at": "2026-03-03T18:12:45Z",
  "models_found": 3,
  "errors": [
    "model-a@1.0.0: artifact fetch failed for ... (https://...)"
  ]
}
```

Notes:

- `models_found` is the number of successfully indexed model/version entries after refresh.
- `errors` contains per-entry resolution errors that did not prevent other entries from loading.
- `refreshed_at` is an RFC3339 UTC timestamp.

---

## Error Format

All errors return a JSON body:

```json
{
  "code": "MODEL_NOT_FOUND",
  "message": "Model product-passport v1.2.0 not found in registry",
  "details": null
}
```

| Code | HTTP Status |
|------|-------------|
| `MODEL_NOT_FOUND` | 404 |
| `NOT_ROUTABLE` | 422 |
| `VALIDATION_FAILED` | 422 |
| `IDEMPOTENCY_CONFLICT` | 409 |
| `STORE_ERROR` | 502 |
| `REGISTRY_ERROR` | 502 |
| `VALIDATOR_ERROR` | 500 |
| `INTERNAL_ERROR` | 500 |
