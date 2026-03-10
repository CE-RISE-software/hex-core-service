# CE-RISE Hex Core Service

A Rust-based hexagonal core service that validates and orchestrates IO for versioned, digital-passport-like records using externally published model artifacts.

This is the primary deployable microservice for CE-RISE data integrations. It exposes a model-agnostic REST API, resolves validation artifacts from a versioned catalog of model URLs, and dispatches to pluggable outbound IO adapters — all without coupling to any specific HTTP framework or repository provider.

Full technical documentation is published at the project [Pages](https://ce-rise-software.codeberg.page/hex-core-service/) site.

---

## What This Project Provides

- Source code for the `hex-core-service` API, validators, and adapters.
- Containerized service image for deployment.
- OpenAPI-based SDK generation pipeline with dedicated SDK repositories for Go, TypeScript, and Python.

## Service Container

### Pull Image

```bash
docker pull rg.fr-par.scw.cloud/ce-rise-software/hex-core-service:<tag>
```

Use an explicit version tag (for example `v0.0.1`) for stable deployments.

### Start Container

```bash
docker run --rm -p 8080:8080 \
  -e REGISTRY_MODE=catalog \
  -e REGISTRY_CATALOG_URL="https://<catalog-host>/catalog.json" \
  -e IO_ADAPTER_ID=memory \
  -e AUTH_MODE=jwt_jwks \
  -e AUTH_JWKS_URL="https://<idp>/realms/<realm>/protocol/openid-connect/certs" \
  -e AUTH_ISSUER="https://<idp>/realms/<realm>" \
  -e AUTH_AUDIENCE="hex-core-service" \
  rg.fr-par.scw.cloud/ce-rise/hex-core-service:<tag>
```

### Required Runtime Parameters

| Variable | Required | Description |
|---|---|---|
| `REGISTRY_MODE` | Yes | Registry backend (`catalog`) |
| `REGISTRY_CATALOG_URL` | Yes (unless file/json alternatives are used) | URL of catalog JSON with model/version/base_url entries |
| `IO_ADAPTER_ID` | Yes | IO adapter implementation (`memory` or configured HTTP adapter) |
| `AUTH_MODE` | Yes | Authentication mode (`jwt_jwks`, `forward_auth`, `none`) |
| `AUTH_JWKS_URL` | Yes for `jwt_jwks` | JWKS endpoint URL |
| `AUTH_ISSUER` | Yes for `jwt_jwks` | Expected token issuer |
| `AUTH_AUDIENCE` | Yes for `jwt_jwks` | Expected token audience |
| `AUTH_ALLOW_INSECURE_NONE` | Yes for `none` | Must be `true` to allow non-auth mode |

## SDKs

### SDK Source Repositories

- Go SDK: https://codeberg.org/CE-RISE-software/hex-core-sdk-go
- TypeScript SDK: https://codeberg.org/CE-RISE-software/hex-core-sdk-typescript
- Python SDK: https://codeberg.org/CE-RISE-software/hex-core-sdk-python

### Import in Projects

Go (current concrete usage):

```go
import hexsdk "codeberg.org/CE-RISE-software/hex-core-sdk-go"
```

```bash
go get codeberg.org/CE-RISE-software/hex-core-sdk-go
```

TypeScript (npm placeholder, to be finalized when package publication is enabled):

```ts
import { Configuration } from "@ce-rise/hex-core-sdk";
```

```bash
npm install @ce-rise/hex-core-sdk
```

Python (PyPI placeholder, to be finalized when package publication is enabled):

```python
from ce_rise_hex_core_sdk import ApiClient
```

```bash
pip install ce-rise-hex-core-sdk
```

## License

Licensed under the [European Union Public Licence v1.2 (EUPL-1.2)](LICENSE).

---

<a href="https://europa.eu" target="_blank" rel="noopener noreferrer">
  <img src="https://ce-rise.eu/wp-content/uploads/2023/01/EN-Funded-by-the-EU-PANTONE-e1663585234561-1-1.png" alt="EU emblem" width="200"/>
</a>

Funded by the European Union under Grant Agreement No. 101092281 — CE-RISE.  
Views and opinions expressed are those of the author(s) only and do not necessarily reflect those of the European Union or the granting authority (HADEA).
Neither the European Union nor the granting authority can be held responsible for them.

© 2026 CE-RISE consortium.  
Licensed under the [European Union Public Licence v1.2 (EUPL-1.2)](LICENSE).  
Attribution: CE-RISE project (Grant Agreement No. 101092281) and the individual authors/partners as indicated.

<a href="https://www.nilu.com" target="_blank" rel="noopener noreferrer">
  <img src="https://nilu.no/wp-content/uploads/2023/12/nilu-logo-seagreen-rgb-300px.png" alt="NILU logo" height="20"/>
</a>

Developed by NILU (Riccardo Boero — ribo@nilu.no) within the CE-RISE project.
