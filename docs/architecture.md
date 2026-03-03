# Architecture

## Overview

The CE-RISE Hex Core Service follows a strict hexagonal (ports and adapters) architecture.
The domain and use-case logic in `crates/core` has no knowledge of HTTP, databases, or any
specific IO provider. All external interactions are mediated through port traits.

The following diagram illustrates the hexagonal architecture pattern used in this service:

```text
                    ┌──────────────────────────────────────┐
                    │           INBOUND ADAPTERS           │
                    │  REST API (axum)  │  CLI  │  Tests   │
                    └──────────┬──────────────────────────-┘
                               │  calls inbound port traits
                    ┌──────────▼───────────────────────────-┐
                    │             CORE (crate)              │
                    │  ┌────────────────────────────────┐   │
                    │  │   Use Cases (implementations)  │   │
                    │  │  ValidateUseCase               │   │
                    │  │  RecordUseCase                 │   │
                    │  │  EnrichUseCase (opt.)          │   │
                    │  └────────────┬───────────────────┘   │
                    │               │  calls outbound port traits
                    └──────────────-│─────────────────────--┘
                                    │
              ┌─────────────────────┼───────────────────────┐
              │                     │                       │
   ┌──────────▼──────┐  ┌───────────▼──────┐  ┌────────────▼──────┐
   │ ArtifactRegistry│  │  ValidatorPort   │  │  RecordStorePort  │
   │  (URL registry) │  │  (SHACL / JSON   │  │  (HTTP IO adapter │
   │                 │  │   Schema / OWL)  │  │   memory / db)    │
   └─────────────────┘  └──────────────────┘  └───────────────────┘
```

### Layer dependency rules

The architecture enforces strict dependency constraints to maintain separation of concerns:

| Layer | Allowed dependencies | Forbidden |
|-------|---------------------|-----------|
| `core/domain` | `std`, `serde`, `thiserror` | Everything else |
| `core/ports` | `core/domain`, `async-trait` | Any I/O |
| `core/usecases` | `core/domain`, `core/ports` | Any I/O, HTTP, DB |
| `registry` | `core/ports`, `reqwest`, `tokio` | `core/usecases` |
| `api` | `core/ports`, `axum`, `tower`, `jsonwebtoken` | Direct DB/IO |
| `validator-*` | `core/ports`, validator-specific libs | HTTP, DB |
| `io-*` | `core/ports`, adapter-specific libs | Core use cases |

These rules ensure that:
- The core domain remains pure and testable
- Business logic has no coupling to infrastructure
- Adapters can be swapped without affecting the core
- Dependencies flow inward toward the domain

## Layers

- **Domain** (`crates/core/src/domain`) — entities, value objects, error types. No I/O.
- **Ports** (`crates/core/src/ports`) — inbound use-case traits and outbound adapter traits.
- **Use cases** (`crates/core/src/usecases`) — orchestration logic implementing inbound ports.
- **Registry** (`crates/registry`) — URL-based artifact resolution implementing `ArtifactRegistryPort`.
- **REST adapter** (`crates/api`) — axum HTTP server implementing the inbound interface.
- **IO adapters** (`crates/io-memory`, `crates/io-http`) — `RecordStorePort` implementations.
- **Validators** (`crates/validator-jsonschema`, `crates/validator-shacl`) — `ValidatorPort` implementations.

## Key design decisions

TODO: document ADRs here as decisions are made.
