# Changelog

All notable changes to `hex-core-service` are documented in this file.

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
