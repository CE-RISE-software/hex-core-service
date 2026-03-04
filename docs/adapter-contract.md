# Adapter Contract

This document specifies the integration contract for IO adapters, validators, and enrichers.
All adapters integrate via port traits defined in `crates/core/src/ports/outbound/`.

---

## Overview

The hex-core-service uses **port traits** to define contracts between the core and external adapters.
Adapters implement these traits to provide pluggable functionality without coupling the core to specific implementations.

### Adapter Types

| Adapter Type | Port Trait | Purpose |
|--------------|------------|---------|
| IO Adapter | `RecordStorePort` | Read/write business records to external storage |
| Validator | `ValidatorPort` | Validate payloads against model artifacts |
| Enricher | `EnricherPort` | Enrich records with external data (optional) |
| Registry | `ArtifactRegistryPort` | Resolve versioned model artifacts |

---

## RecordStorePort (IO Adapter Contract)

### Trait Definition

```rust
// crates/core/src/ports/outbound/record_store.rs
#[async_trait::async_trait]
pub trait RecordStorePort: Send + Sync {
    async fn write(
        &self,
        ctx:    &SecurityContext,
        record: Record,
    ) -> Result<RecordId, StoreError>;

    async fn read(
        &self,
        ctx: &SecurityContext,
        id:  &RecordId,
    ) -> Result<Record, StoreError>;

    async fn query(
        &self,
        ctx:    &SecurityContext,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError>;
}
```

### Method Specifications

#### `write`

**Purpose:** Persist a new or updated record.

**Parameters:**
- `ctx` — Security context containing user identity, roles, and access token
- `record` — Complete record with ID, model, version, and payload

**Returns:** 
- `Ok(RecordId)` — The persisted record's ID (may be generated or echoed)
- `Err(StoreError)` — Storage failure, conflict, or authorization error

**Requirements:**
- Must support idempotency via `Idempotency-Key` (implementation-specific)
- Must validate user authorization before persisting
- Should preserve record metadata (model, version)
- Must return `StoreError::IdempotencyConflict` if key is reused with different payload

**Security:**
- Adapter receives `SecurityContext::raw_token` and must forward it to backend services
- Adapter must never log or persist the access token

#### `read`

**Purpose:** Retrieve a single record by ID.

**Parameters:**
- `ctx` — Security context
- `id` — Record identifier

**Returns:**
- `Ok(Record)` — The requested record
- `Err(StoreError::NotFound)` — Record does not exist or user lacks access
- `Err(StoreError)` — Other storage error

**Requirements:**
- Must enforce authorization (user can only read records they have access to)
- Should be fast (single lookup, not a scan)

#### `query`

**Purpose:** Search for records matching filter criteria.

**Parameters:**
- `ctx` — Security context
- `filter` — JSON query expression (adapter-specific format)

**Returns:**
- `Ok(Vec<Record>)` — Matching records (may be empty)
- `Err(StoreError)` — Storage error or invalid filter

**Requirements:**
- Must enforce authorization (filter results to user's scope)
- Filter format is adapter-specific; document in adapter README
- Should support pagination (implementation-specific)
- May return empty results if no matches found

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
    
    #[error("idempotency conflict")]
    IdempotencyConflict,
    
    #[error("unauthorized")]
    Unauthorized,
    
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("internal: {0}")]
    Internal(String),
}
```

### Implementation Examples

- `crates/io-memory` — In-memory HashMap (for testing and local development)
- `crates/io-http` — HTTP client to external IO Adapter Service

Versioned IO Adapter OpenAPI contract (current source of truth for HTTP paths/methods):

- `crates/io-http/src/io_adapter_openapi.json`

---

## ValidatorPort (Validator Contract)

### Trait Definition

```rust
// crates/core/src/ports/outbound/validator.rs
#[async_trait::async_trait]
pub trait ValidatorPort: Send + Sync {
    fn kind(&self) -> ValidatorKind;

    async fn validate(
        &self,
        artifacts: &ArtifactSet,
        payload:   &serde_json::Value,
    ) -> Result<ValidationResult, ValidatorError>;
}
```

### Method Specifications

#### `kind`

**Purpose:** Identifies the validator type for reporting.

**Returns:** `ValidatorKind` enum variant:
- `ValidatorKind::JsonSchema`
- `ValidatorKind::Shacl`
- `ValidatorKind::Owl`

**Requirements:**
- Must be a constant value (no I/O)
- Used in `ValidationResult` to identify which validator produced each result

#### `validate`

**Purpose:** Validate a payload against model artifacts.

**Parameters:**
- `artifacts` — Resolved model artifacts (may contain schema, SHACL, OWL, etc.)
- `payload` — JSON payload to validate

**Returns:**
- `Ok(ValidationResult)` — Validation outcome with violations (if any)
- `Err(ValidatorError)` — Validator setup or execution error

**Requirements:**
- Must return `passed: true` only if no violations found
- Must populate `violations` with all detected issues
- Must include `path` (JSON pointer or similar) for each violation
- Should skip validation gracefully if required artifact is absent
- Must not throw exceptions; return structured errors

**Behavior when artifact is missing:**
Validators should return `Ok(ValidationResult { passed: true, violations: [] })` and log a warning if the required artifact is absent. The orchestrator in `ValidateUseCaseImpl` already skips validators when artifacts are unavailable.

### ValidationResult Structure

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationResult {
    pub kind:       ValidatorKind,
    pub passed:     bool,
    pub violations: Vec<ValidationViolation>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationViolation {
    pub path:    Option<String>,  // JSON pointer, e.g. "/properties/name"
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Error,   // Validation failure
    Warning, // Non-blocking issue
    Info,    // Informational
}
```

### Validation Orchestration

The core orchestrates validators as follows:

1. Resolve `ArtifactSet` for `(model, version)`
2. For each configured validator:
   - Skip if required artifact is absent
   - Call `validator.validate(artifacts, payload)`
   - Collect `ValidationResult`
3. Merge results into `ValidationReport`
4. Set `ValidationReport::passed = true` only if **all** validators pass

**Preferred validator:** SHACL (richer constraint checking)

### Implementation Examples

- `crates/validator-jsonschema` — JSON Schema Draft 2020-12
- `crates/validator-shacl` — SHACL Turtle validation (preferred)
- `crates/validator-owl` — OWL ontology validation (optional)

---

## EnricherPort (Enricher Contract)

### Trait Definition

```rust
// crates/core/src/ports/outbound/enricher.rs
#[async_trait::async_trait]
pub trait EnricherPort: Send + Sync {
    async fn enrich(
        &self,
        ctx:    &SecurityContext,
        record: &Record,
    ) -> Result<serde_json::Value, EnricherError>;
}
```

### Method Specifications

#### `enrich`

**Purpose:** Augment a record with additional data from external sources.

**Parameters:**
- `ctx` — Security context (for authorization and token passthrough)
- `record` — The record to enrich

**Returns:**
- `Ok(serde_json::Value)` — Enriched payload (merged with or replacing original)
- `Err(EnricherError)` — Enrichment failed

**Requirements:**
- Must be idempotent (same input → same output)
- May call external APIs (product databases, certification registries, etc.)
- Should time out gracefully if external service is slow
- Must forward `SecurityContext::raw_token` if external service requires it
- Should log external failures but not crash

**Use Case:** The `EnrichUseCase` reads a record, calls the enricher, and writes back the enriched payload.

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum EnricherError {
    #[error("external service unavailable")]
    ServiceUnavailable,
    
    #[error("timeout")]
    Timeout,
    
    #[error("unauthorized")]
    Unauthorized,
    
    #[error("internal: {0}")]
    Internal(String),
}
```

### Implementation Notes

- Enrichers are **optional**; the core works without them
- Enrichment is triggered via `POST /models/{model}/versions/{version}:enrich`
- Enrichers must support `Idempotency-Key` to avoid duplicate side effects

---

## ArtifactRegistryPort (Registry Contract)

### Trait Definition

```rust
// crates/core/src/ports/outbound/registry.rs
#[async_trait::async_trait]
pub trait ArtifactRegistryPort: Send + Sync {
    async fn resolve(
        &self,
        model: &ModelId,
        ver:   &ModelVersion,
    ) -> Result<ArtifactSet, RegistryError>;

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, RegistryError>;

    async fn refresh(&self) -> Result<RefreshSummary, RegistryError>;
}
```

### Method Specifications

#### `resolve`

**Purpose:** Retrieve all artifacts for a specific model version.

**Parameters:**
- `model` — Model identifier (e.g., `product-passport`)
- `ver` — Version string without leading 'v' (e.g., `1.2.0`)

**Returns:**
- `Ok(ArtifactSet)` — All available artifacts
- `Err(RegistryError::NotFound)` — Model version does not exist
- `Err(RegistryError)` — Registry unavailable or invalid

**Requirements:**
- Must fetch from the configured URL template
- Must populate all available artifacts (`route`, `schema`, `shacl`, `owl`, `openapi`)
- Missing optional artifacts should be `None`, not an error
- Missing `route.json` should return `NotFound`

#### `list_models`

**Purpose:** Return all discovered models.

**Returns:**
- `Ok(Vec<ModelDescriptor>)` — List of `{model, version}` pairs

**Requirements:**
- Must reflect the current in-memory index
- Used by `GET /models` endpoint

#### `refresh`

**Purpose:** Re-discover models and atomically swap the index.

**Returns:**
- `Ok(RefreshSummary)` — Summary of refresh operation
- `Err(RegistryError)` — Refresh failed

**Requirements:**
- Must re-fetch all model artifacts
- Must build a new index in memory
- Must atomically swap the index (no downtime)
- Must return errors per model (not fail entirely if one model fails)

### ArtifactSet Structure

```rust
#[derive(Debug, Clone, Default)]
pub struct ArtifactSet {
    pub route:      Option<serde_json::Value>,  // required for dispatch
    pub schema:     Option<String>,             // JSON Schema text
    pub shacl:      Option<String>,             // SHACL Turtle text
    pub owl:        Option<String>,             // OWL Turtle text
    pub openapi:    Option<String>,             // OpenAPI YAML/JSON text
}

impl ArtifactSet {
    pub fn is_routable(&self) -> bool {
        self.route.is_some()
    }
}
```

### Catalog Entry URL Format

Catalog entries must provide a concrete artifact base URL per `(model, version)`, for example:
```
https://codeberg.org/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/
```

The running service reads catalog entries from one of:

- `REGISTRY_CATALOG_JSON`
- `REGISTRY_CATALOG_FILE`
- `REGISTRY_CATALOG_URL`

### Artifact Filename Defaults

| Artifact | Filename | Required |
|----------|----------|----------|
| Route definition | `route.json` | **Yes** |
| JSON Schema | `schema.json` | No |
| SHACL shapes | `shacl.ttl` | No |
| OWL ontology | `owl.ttl` | No |
| OpenAPI spec | `openapi.json` | No |

Override via `REGISTRY_ARTIFACT_MAP_<KEY>` environment variables.

### Resolution Behavior

1. On startup, fetch `route.json` for each model
2. Silently skip missing optional artifacts
3. Mark model as non-routable if `route.json` is absent
4. Cache artifacts only if `REGISTRY_CACHE_ENABLED=true` (default: disabled)
5. Refresh index via `POST /admin/registry/refresh`

### Implementation Example

- `crates/registry` — catalog-backed artifact registry with URL fetch helper

---

## Security Requirements

All adapters must adhere to these security rules:

1. **Token Passthrough:** Forward `SecurityContext::raw_token` to backend services as `Authorization: Bearer <token>`
2. **No Token Logging:** Never log or persist access tokens
3. **Authorization:** Enforce user-level access control where applicable
4. **HTTPS Only:** Use HTTPS for all external calls (override with `REGISTRY_REQUIRE_HTTPS=false` only in dev)
5. **Timeouts:** Always set request timeouts to prevent indefinite hangs

---

## Testing Requirements

All adapter implementations must include:

1. **Unit tests** — Trait methods with mocked dependencies
2. **Contract tests** — Known-good and known-bad inputs
3. **Integration tests** — Against real or wiremocked external services
4. **Error handling tests** — Network failures, timeouts, malformed responses

### Example Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_write_success() {
        // Arrange: create adapter and valid record
        // Act: call write()
        // Assert: returns Ok(RecordId)
    }

    #[tokio::test]
    async fn test_write_idempotency_conflict() {
        // Arrange: write once, then retry with different payload
        // Act: call write() with same key
        // Assert: returns Err(StoreError::IdempotencyConflict)
    }

    #[tokio::test]
    async fn test_validate_pass() {
        // Arrange: valid payload and artifacts
        // Act: call validate()
        // Assert: returns Ok(ValidationResult { passed: true, ... })
    }

    #[tokio::test]
    async fn test_validate_fail() {
        // Arrange: invalid payload and artifacts
        // Act: call validate()
        // Assert: returns Ok(ValidationResult { passed: false, violations: [...] })
    }
}
```

---

## Adapter Development Checklist

When implementing a new adapter:

- [ ] Define a new crate in `crates/<adapter-name>/`
- [ ] Depend on `crates/core` (ports and domain types only)
- [ ] Implement the appropriate port trait (`RecordStorePort`, `ValidatorPort`, etc.)
- [ ] Add unit tests with 100% coverage of trait methods
- [ ] Add contract tests with known-good and known-bad inputs
- [ ] Document adapter-specific configuration in adapter's `README.md`
- [ ] Add integration tests (wiremock or testcontainers)
- [ ] Document error handling behavior
- [ ] Add adapter to main `Cargo.toml` workspace
- [ ] Update deployment guide with adapter setup instructions
- [ ] Add adapter to `README.md` list of available adapters

---

## Support

For questions about adapter contracts:

- Review existing implementations in `crates/io-memory`, `crates/validator-jsonschema`
- Open an issue on Codeberg: https://codeberg.org/CE-RISE-software/hex-core-service/issues
- Contact: ribo@nilu.no
