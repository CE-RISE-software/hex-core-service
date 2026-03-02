# Architecture

> Full architecture documentation is published to the project Pages site.
> This file is the source; edit here, publish via CI.

## Overview

The CE-RISE Hex Core Service follows a strict hexagonal (ports and adapters) architecture.
The domain and use-case logic in `crates/core` has no knowledge of HTTP, databases, or any
specific IO provider. All external interactions are mediated through port traits.

See `AGENTS.md §4` for the authoritative architecture diagram and layer dependency rules.

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