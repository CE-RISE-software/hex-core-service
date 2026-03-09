# Operations Runbook

This runbook provides operational procedures for running and maintaining the CE-RISE Hex Core Service in production.

## Contents

- [Health and Readiness Checks](#health-and-readiness-checks)
- [Registry Refresh Flow](#registry-refresh-flow)
- [Authentication Operations](#authentication-operations)
- [SHACL Validation Operations](#shacl-validation-operations)
- [OWL Validation Operations](#owl-validation-operations)
- [Metrics Reference](#metrics-reference)
- [Troubleshooting Guide](#troubleshooting-guide)
- [Common Issues](#common-issues)

---

## Health and Readiness Checks

### Liveness Probe

**Endpoint:** `GET /admin/health`

**Purpose:** Determines if the service process is alive and responding to requests.

**Expected Response:**
```json
{
  "status": "ok"
}
```

**HTTP Status:** `200 OK`

**Kubernetes Configuration:**
```yaml
livenessProbe:
  httpGet:
    path: /admin/health
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
  timeoutSeconds: 3
  failureThreshold: 3
```

### Readiness Probe

**Endpoint:** `GET /admin/ready`

**Purpose:** Determines if the service is ready to accept traffic. Returns success only when:
- The artifact registry has been loaded successfully
- At least one model is available
- All required adapters are initialized

**Expected Response (ready):**
```json
{
  "status": "ready",
  "registry_loaded": true,
  "models_available": 5
}
```

**HTTP Status:** `200 OK` (ready) or `503 Service Unavailable` (not ready)

**Kubernetes Configuration:**
```yaml
readinessProbe:
  httpGet:
    path: /admin/ready
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 3
```

### Status Endpoint

**Endpoint:** `GET /admin/status`

**Purpose:** Returns detailed runtime status information.

**Expected Response:**
```json
{
  "uptime_seconds": 3600,
  "registry": {
    "models_loaded": 5,
    "last_refresh": "2024-01-15T10:30:00Z",
    "cache_enabled": false
  },
  "config": {
    "io_adapter_id": "circularise",
    "io_adapter_version": "v1",
    "validators_enabled": ["shacl", "jsonschema"]
  }
}
```

---

## Registry Refresh Flow

The artifact registry can be manually refreshed to discover new models or updated versions without restarting the service.

### Manual Refresh

**Endpoint:** `POST /admin/registry/refresh`

**Authentication:** Requires admin token or network-level protection (mTLS, private subnet).

**Process:**
1. The registry re-loads the configured catalog source (`REGISTRY_CATALOG_URL` or `REGISTRY_CATALOG_FILE`).
2. The registry resolves artifacts (`route.json` required) for each catalog entry.
3. A new index is built in memory.
4. The index is atomically swapped (no downtime).
5. A refresh summary is returned.

If `REGISTRY_CATALOG_JSON` is used, refresh reuses the inline catalog value loaded at startup.

**Expected response shape:**
```json
{
  "refreshed_at": "2026-03-03T18:12:45Z",
  "models_found": 5,
  "errors": []
}
```

**Partial failure example:**
```json
{
  "refreshed_at": "2026-03-03T18:12:45Z",
  "models_found": 4,
  "errors": [
    "product-passport@2.0.0: model not found in registry: product-passport v2.0.0 (https://...)"
  ]
}
```

Behavior details:

- Catalog load/parse failure: refresh returns error and previous index remains active.
- Entry-level fetch/validation failure: refresh succeeds, failing entries are excluded, and errors are listed.
- Duplicate `(model, version)` in catalog: refresh fails and previous index remains active.

### GitOps workflow

For GitOps-managed deployments:

1. Publish/update `catalog.json` in your config repo/object storage.
2. Ensure service points to it via `REGISTRY_CATALOG_URL` or mounted `REGISTRY_CATALOG_FILE`.
3. Trigger `POST /admin/registry/refresh`.
4. Verify with `GET /models`.

This allows model additions/removals/version updates without restarting the service.

### When to Refresh

- After deploying new model versions to the registry
- When models are returning `404 Not Found` errors
- As part of a scheduled maintenance window
- When the `/admin/status` endpoint shows stale registry data

### Automatic Refresh

Automatic refresh is **not** implemented by default. If needed, implement it as:
- A Kubernetes CronJob calling the refresh endpoint
- An external scheduler (cron, Airflow, etc.)
- A sidecar container with a polling loop

**Recommended Interval:** Every 5-15 minutes, depending on how frequently models are updated.

---

## Authentication Operations

For full authentication architecture and integration patterns, see [Authentication](authentication.md).

### Runtime Auth Mode Check

Confirm active auth mode in environment:

```bash
echo "$AUTH_MODE"
```

Expected values:

- `jwt_jwks`
- `forward_auth`
- `none` (isolated non-production only)

### Quick Validation Path Checks

- `jwt_jwks`: verify `AUTH_JWKS_URL`, `AUTH_ISSUER`, `AUTH_AUDIENCE` are set and reachable.
- `forward_auth`: verify gateway injects configured subject/roles/scopes headers.
- `none`: verify `AUTH_ALLOW_INSECURE_NONE=true` is explicitly set.

---

## SHACL Validation Operations

Use this section together with [SHACL Validation](shacl-validation.md), which defines current validation scope and limits.

### Preconditions

- The model/version is present in the active registry index.
- The model artifact folder contains `shacl.ttl`.
- Registry refresh has been executed after catalog/artifact updates.

### Quick Check

1. Confirm model exists:
   ```bash
   curl http://localhost:8080/models
   ```
2. Confirm SHACL artifact is resolvable:
   ```bash
   curl http://localhost:8080/models/{model}/versions/{version}/shacl
   ```
3. Validate payload:
   ```bash
   curl -X POST http://localhost:8080/models/{model}/versions/{version}:validate \
     -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{"payload":{...}}'
   ```

### Interpreting Results

- `passed=true`: all executed validators passed.
- `results[].kind="shacl"`: SHACL adapter result block.
- `results[].violations[]`: path/message/severity entries for SHACL failures.

### Common SHACL Failure Patterns

- Invalid enum values (for example `record_scope` or `relation_type`)
- Invalid timestamp format (non-RFC3339)
- Wrong primitive type (integer/number mismatch)
- Unexpected keys inside closed sections such as `applied_schemas[*]`

---

## OWL Validation Operations

### Runtime Mode

- OWL validation currently runs in embedded profile mode.
- No external reasoner subprocess is required by default deployment.

### Preconditions

- The model/version is present in registry index.
- The model artifact folder contains `owl.ttl`.
- Registry refresh has been executed after artifact/catalog updates.

### Quick Check

1. Confirm OWL artifact resolves:
   ```bash
   curl http://localhost:8080/models/{model}/versions/{version}/owl
   ```
2. Submit payload to validate endpoint:
   ```bash
   curl -X POST http://localhost:8080/models/{model}/versions/{version}:validate \
     -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{"payload":{...}}'
   ```
3. Confirm response contains OWL result block:
   - `results[].kind == "Owl"`

### Error Mapping

- Missing `owl.ttl`: validator is skipped.
- Invalid ontology artifact: validator initialization error.
- Runtime failure in validator execution path: validator execution error.

### Performance Notes

- OWL artifacts can be larger than route/schema artifacts; refresh and in-memory footprint grow accordingly.
- Keep OWL artifacts versioned and immutable where possible to avoid cache churn.
- If OWL validation latency grows, monitor `validation_duration_seconds{validator="owl"}` and scale replicas before increasing request concurrency.

---

## Metrics Reference

**Endpoint:** `GET /admin/metrics` (when `METRICS_ENABLED=true`)

**Format:** Prometheus text exposition format

### Key Metrics

#### Request Metrics
- `http_requests_total{method, path, status}` — Total HTTP requests
- `http_request_duration_seconds{method, path}` — Request latency histogram
- `http_requests_in_flight{method, path}` — Current concurrent requests

#### Validation Metrics
- `validation_requests_total{model, version, result}` — Total validation requests (`result`: `pass`/`fail`)
- `validation_duration_seconds{model, version, validator}` — Validation latency per validator
- `validation_violations_total{model, version, severity}` — Violation counts by severity

#### Registry Metrics
- `registry_models_loaded` — Number of models currently loaded
- `registry_refresh_total{result}` — Total refresh attempts (`result`: `success`/`failure`)
- `registry_refresh_duration_seconds` — Refresh operation latency
- `registry_artifact_fetch_total{model, version, result}` — Artifact fetch attempts

#### IO Adapter Metrics
- `io_adapter_requests_total{operation, result}` — Outbound IO adapter calls
- `io_adapter_request_duration_seconds{operation}` — IO adapter latency
- `io_adapter_errors_total{operation, error_type}` — IO adapter errors

#### Idempotency Metrics
- `idempotency_conflicts_total{model, version}` — Idempotency key conflicts

### Alerting Rules

Recommended Prometheus alert rules:

```yaml
groups:
  - name: hex-core-service
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High HTTP 5xx error rate"

      - alert: RegistryRefreshFailed
        expr: registry_refresh_total{result="failure"} > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Registry refresh failed"

      - alert: IOAdapterDown
        expr: rate(io_adapter_errors_total[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High IO adapter error rate"

      - alert: ServiceNotReady
        expr: up{job="hex-core-service"} == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Service is not ready"
```

---

## Troubleshooting Guide

### API Error Matrix

| HTTP | Typical error body | Likely cause | User action |
|---|---|---|---|
| `401` | auth error response | Missing/invalid token, wrong auth mode config, expired token | Provide valid token, verify auth variables and mode |
| `403` | auth error response | Caller authenticated but denied by policy/scope/role | Request required role/scope or adjust policy |
| `404` | `MODEL_NOT_FOUND` | Model/version not in active registry index | Check catalog entry, trigger `/admin/registry/refresh`, retry |
| `409` | `IDEMPOTENCY_CONFLICT` | Reused `Idempotency-Key` with different payload | Use a new idempotency key for changed payload |
| `422` | `VALIDATION_FAILED` or `NOT_ROUTABLE` | Payload does not satisfy model artifacts or route constraints | Inspect violations, fix payload, retry |
| `500` | `VALIDATOR_ERROR` or `INTERNAL_ERROR` | Server-side validator/runtime issue | Check logs and validator artifacts |
| `502` | `STORE_ERROR` or `REGISTRY_ERROR` | Downstream IO adapter/registry fetch failure | Check downstream service/network health and retry |

### Service Won't Start

**Symptoms:** Container exits immediately or crashes in a loop.

**Diagnosis:**
1. Check logs for configuration errors:
   ```bash
   kubectl logs -f deployment/hex-core-service
   ```
2. Verify all required environment variables are set (see `configuration.md`)
3. Check JWKS URL is reachable:
   ```bash
   curl -v $AUTH_JWKS_URL
   ```
4. Verify registry URL template is valid

**Common Causes:**
- Missing catalog source (`REGISTRY_CATALOG_URL`, `REGISTRY_CATALOG_FILE`, or `REGISTRY_CATALOG_JSON`)
- Invalid `AUTH_JWKS_URL` (unreachable or malformed)
- Missing `IO_ADAPTER_BASE_URL` when using HTTP adapter
- Network policy blocking outbound registry access

### Registry Not Loading Models

**Symptoms:** `/admin/ready` returns `503`, `/models` returns empty list.

**Diagnosis:**
1. Check registry refresh endpoint:
   ```bash
   curl -X POST http://localhost:8080/admin/registry/refresh
   ```
2. Check logs for registry errors
3. Verify registry URLs are accessible from the pod:
   ```bash
   kubectl exec -it deployment/hex-core-service -- wget -O- $REGISTRY_CATALOG_URL
   ```

**Common Causes:**
- Catalog URL/file path is wrong or unreachable
- `REGISTRY_ALLOWED_HOSTS` blocks the registry domain
- `REGISTRY_REQUIRE_HTTPS=true` but registry uses HTTP
- Missing `route.json` in all model repositories

### Validation Always Fails

**Symptoms:** All validation requests return `passed: false`.

**Diagnosis:**
1. Test with a known-good payload (check model documentation)
2. Verify artifact contents:
   ```bash
   curl http://localhost:8080/models/{model}/versions/{version}/schema
   curl http://localhost:8080/models/{model}/versions/{version}/shacl
   ```
3. Check validator logs for parsing errors
4. Ensure payload matches the model's expected structure

**Common Causes:**
- Malformed artifact (invalid JSON Schema or SHACL Turtle)
- Payload is for a different model version
- Validator library incompatibility (check crate versions)

### Authentication Errors

**Symptoms:** All requests return `401 Unauthorized` or `403 Forbidden`.

**Diagnosis:**
1. Verify JWT is valid:
   ```bash
   curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/models
   ```
2. Decode JWT and check claims (use jwt.io)
3. Verify `AUTH_ISSUER` matches token's `iss` claim
4. Verify `AUTH_AUDIENCE` matches token's `aud` claim
5. Check JWKS is being fetched successfully (logs)

**Common Causes:**
- Token expired (`exp` claim in the past)
- Token not yet valid (`nbf` claim in the future)
- `iss` or `aud` mismatch
- JWKS cache stale (wait for `AUTH_JWKS_REFRESH_SECS` or restart)
- Token signed with a key not in JWKS

### IO Adapter Timeout

**Symptoms:** Requests to create/query endpoints time out or return `504 Gateway Timeout`.

**Diagnosis:**
1. Check IO adapter service health directly
2. Review `IO_ADAPTER_TIMEOUT_MS` setting
3. Check network latency between core and adapter
4. Review IO adapter logs for slow queries

**Resolution:**
- Increase `IO_ADAPTER_TIMEOUT_MS` if adapter legitimately needs more time
- Scale IO adapter service if it's overloaded
- Check for network issues (firewalls, DNS resolution)
- Review slow queries with the IO adapter service team

### Idempotency Conflicts

**Symptoms:** Requests return `409 Conflict` with "idempotency conflict" error.

**Diagnosis:**
1. Verify the `Idempotency-Key` is unique per logical operation
2. Check if a previous request with the same key succeeded
3. Review IO adapter's idempotency implementation

**Expected Behavior:**
- Same key + same payload → same result (replay protection)
- Same key + different payload → `409 Conflict` (error)

**Resolution:**
- If the operation already succeeded, the client should accept the conflict
- If the operation failed, use a new `Idempotency-Key`

---

## Common Issues

### High Memory Usage

**Symptoms:** OOM kills, high memory metrics.

**Possible Causes:**
- Large number of models loaded in registry (index is in-memory)
- Large artifact files (especially OpenAPI or OWL)
- Memory leak in a validator or adapter

**Mitigation:**
- Increase memory limits in deployment
- Disable unused validators
- Monitor for leaks with profiling tools
- Consider implementing artifact streaming for large files

### Stale Artifact Cache

**Symptoms:** Service serves old model versions after registry update.

**Diagnosis:**
1. Check if `REGISTRY_CACHE_ENABLED=true`
2. Verify `REGISTRY_CACHE_TTL_SECS` is appropriate

**Resolution:**
- Manually refresh: `POST /admin/registry/refresh`
- Lower `REGISTRY_CACHE_TTL_SECS`
- Disable cache entirely if models update frequently

### Log Volume Too High

**Symptoms:** High disk usage from logs, log aggregation costs.

**Mitigation:**
- Adjust `LOG_LEVEL` to `info` or `warn` (avoid `debug` in production)
- Configure log sampling in high-traffic environments
- Ensure `Authorization` headers are redacted (should be automatic)
- Use structured logging to enable efficient filtering

---

## Emergency Procedures

### Rollback Procedure

If a deployment causes issues:

1. Roll back to previous image tag:
   ```bash
   kubectl set image deployment/hex-core-service \
     hex-core-service=<registry>/<namespace>/hex-core-service:<previous-version>
   ```

2. Verify rollback:
   ```bash
   kubectl rollout status deployment/hex-core-service
   curl http://localhost:8080/admin/health
   ```

### Cache Clear (if implemented)

**Endpoint:** `POST /admin/cache/clear`

Clears only the artifact cache, not business data. Use when:
- Artifacts are served stale despite refresh
- Suspected cache corruption

**Note:** This endpoint is optional and may not be implemented in all deployments.
