# CE-RISE Hex Core Service

A Rust-based hexagonal core service that validates and orchestrates IO for versioned, digital-passport-like records using externally published model artifacts.

This is the primary deployable microservice for CE-RISE data integrations. It exposes a model-agnostic REST API, resolves validation artifacts from a versioned URL registry, and dispatches to pluggable outbound IO adapters — all without coupling to any specific HTTP framework or repository provider.

Full technical documentation is published at the project Pages site.

---

## What this repository contains

- `crates/core` — domain types, port traits, and use-case implementations (no I/O, no HTTP)
- `crates/registry` — artifact registry resolution from versioned URL templates
- `crates/api` — REST inbound adapter (axum)
- `crates/validator-jsonschema` / `crates/validator-shacl` — pluggable validators
- `crates/io-memory` / `crates/io-http` — IO adapter implementations
- `crates/cli` — command-line interface
- `docs/` — source for the Pages documentation site
- `.github/workflows/` — mirror and release automation for GitHub / Zenodo archival
- `.forgejo/workflows/` — primary CI/CD on Codeberg (lint, test, build, release, pages)

## Status

Early development — architecture and port contracts are being established.

## Quick start

```sh
cp .env.example .env
# edit .env with your registry URL and adapter config
cargo build --release
cargo run -p api
```

## Running tests

```sh
# Unit and contract tests
cargo test

# Full suite including integration tests
cargo test --features integration-tests
```

## Configuration

All runtime configuration is via environment variables. See `.env.example` for the full reference. Key variables:

| Variable | Description |
|----------|-------------|
| `REGISTRY_URL_TEMPLATE` | URL template for resolving model artifacts |
| `IO_ADAPTER_ID` | IO adapter to use (`memory`, `circularise`, etc.) |
| `AUTH_JWKS_URL` | Keycloak JWKS endpoint for JWT validation |
| `SERVER_PORT` | HTTP bind port (default `8080`) |

## License

Licensed under the [European Union Public Licence v1.2 (EUPL-1.2)](LICENSE).

## Contributing

This repository is maintained on [Codeberg](https://codeberg.org/CE-RISE-software/hex-core-service) — the canonical source of truth. The GitHub repository is a read mirror used for release archival and Zenodo integration. Issues and pull requests should be opened on Codeberg.

---

<a href="https://europa.eu" target="_blank" rel="noopener noreferrer">
  <img src="https://ce-rise.eu/wp-content/uploads/2023/01/EN-Funded-by-the-EU-PANTONE-e1663585234561-1-1.png" alt="EU emblem" width="200"/>
</a>

Funded by the European Union under Grant Agreement No. 101092281 — CE-RISE.  
Views and opinions expressed are those of the author(s) only and do not necessarily reflect those of the European Union or the granting authority (HADEA).  
Neither the European Union nor the granting authority can be held responsible for them.

© 2025 CE-RISE consortium.  
Licensed under the [European Union Public Licence v1.2 (EUPL-1.2)](LICENSE).  
Attribution: CE-RISE project (Grant Agreement No. 101092281) and the individual authors/partners as indicated.

<a href="https://www.nilu.com" target="_blank" rel="noopener noreferrer">
  <img src="https://nilu.no/wp-content/uploads/2023/12/nilu-logo-seagreen-rgb-300px.png" alt="NILU logo" height="20"/>
</a>

Developed by NILU (Riccardo Boero — ribo@nilu.no) within the CE-RISE project.
