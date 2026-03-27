# Changelog

All notable changes to `hex-core-service` are documented in this file.

## [0.0.9] - 03-27-2026

### Changed
- CLI binaries are now attached directly to Codeberg release pages as release assets instead of being available only as workflow artifacts.

### Fixed
- Removed the unsupported `route` artifact concept from the core, registry, API, and documentation.
- Validation-only model entries now work without any extra dispatch artifact requirement.
- Replaced ad hoc record ID generation with UUID v4 identifiers.
- Implemented actual canonical query filtering in the in-memory adapter instead of returning all records.
- Removed placeholder admin endpoints for config dump and cache clearing.
- Cleaned stale documentation wording left behind by earlier scaffolding.

## [0.0.8] - 03-17-2026

### Fixed
- Release of CLI workflow.
- Removed unecessary version field from CITATION.cff.

## [0.0.7] - 03-16-2026

### Fixed
- router.rs](/home/riccardo/code/CE-RISE-software/hex-core-service/crates/api/src/router.rs) was using brace-style path params in the actual Axum router:
  - `"/models/{model}/versions/{version}/schema"`
- every parameterized model-version route was treated as a literal path and never matched
- result was framework `404`
- fixed by switching to colon-style path params: `"/models/:model/versions/:version/schema"`


## [0.0.6] - 03-16-2026

### Changed
- Replaced legacy catalog `base_url` entries with explicit per-artifact references (`route_url`, `schema_url`, `shacl_url`, `owl_url`, `openapi_url`).
- Removed the assumption that all model artifacts must live under one inferred base directory.
- Updated registry loading so models can be declared from heterogeneous artifact publication locations.
- Clarified that `route_url` is only required for routable operations, while validation-only model entries may publish only the artifacts they need.
- Updated README, configuration, deployment, onboarding, adapter, SHACL, and runbook docs to the new catalog contract.

## [0.0.5] - 03-11-2026

### Changed
- Formalized the canonical JSON query dialect for record store backends in the adapter contract.
- Constrained the OpenAPI `QueryRequest.filter` schema to the shared query structure (`where`, `sort`, `limit`, `offset`).
- Documented supported query operators and field path rules for backend adapter alignment.

## [0.0.4] - 03-11-2026

### Added
- New endpoint `GET /admin/models/count` returning the number of currently indexed registry models.
- OpenAPI path and response schema for `admin/models/count`.

## [0.0.3] - 03-11-2026

### Added
- New endpoint `GET /admin/version`
- Version response schema


## [0.0.2] - 03-11-2026

### Fixed
- Service tests.
- Deployment pipeline.


## [0.0.1] - 03-11-2026

### Added
- Hexagonal Rust workspace with core domain, port traits, and use-case implementations.
- Catalog-backed artifact registry with refresh support and URL-based artifact resolution.
- Validator adapters for JSON Schema, SHACL, and OWL.
- IO adapters: in-memory store and external HTTP adapter.
- REST API adapter with model operation endpoints, admin endpoints, idempotency key propagation, and auth mode support.
- Documentation site (`mdBook`) with architecture, API, configuration, deployment, authentication, validation, and runbook pages.
- CI on push for formatting and workspace test execution.
- Release workflow for CLI binaries and container image publication.
- SDK sync workflow on OpenAPI changes for Go, TypeScript, and Python external SDK repositories.

### Changed
- Container release publication now uses version and `latest` tags on release tags (`v*.*.*`), without commit-SHA image tags.
