I# Deployment

This guide covers packaging, containerization, and deployment of the CE-RISE Hex Core Service.

## Contents

- [Local Development](#local-development)
- [Container Image](#container-image)
- [Image Tags and Versioning](#image-tags-and-versioning)
- [Environment Configuration](#environment-configuration)
- [Container Registry](#container-registry)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Release Process](#release-process)

---

## Local Development

### Using Docker Compose

The repository includes a `docker-compose.yml` for local development with minimal dependencies:

```bash
# Copy environment template
cp .env.example .env

# Edit .env with your configuration
# For local development, use the in-memory adapter:
# IO_ADAPTER_ID=memory

# Start the service
docker-compose up

# Or run in the background
docker-compose up -d

# View logs
docker-compose logs -f

# Stop the service
docker-compose down
```

The compose setup includes:
- Hex Core Service with `io-memory` adapter
- Mock registry (wiremock) for artifact resolution
- No external dependencies required

### Running Natively

For faster development iteration:

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and run
cargo build --release
cargo run -p api

# Or use cargo-watch for auto-reload
cargo install cargo-watch
cargo watch -x 'run -p api'
```

---

## Container Image

### Multi-Stage Dockerfile

The service uses a multi-stage build to produce a minimal runtime image:

**Stage 1 — Builder:**
- Base: `rust:1-slim`
- Builds the `api` binary from source
- Includes all necessary build dependencies

**Stage 2 — Runtime:**
- Base: `debian:bookworm-slim`
- Contains only the compiled binary and runtime dependencies
- No source code, no build tools
- No proprietary adapter binaries

### Building Locally

```bash
# Build the image
docker build -t hex-core-service:local .

# Run the image
docker run -p 8080:8080 \
  -e REGISTRY_MODE="catalog" \
  -e REGISTRY_CATALOG_URL="https://config.example.org/hex-core/catalog.json" \
  -e IO_ADAPTER_ID="memory" \
  -e AUTH_JWKS_URL="https://keycloak.example.com/realms/cerise/protocol/openid-connect/certs" \
  -e AUTH_ISSUER="https://keycloak.example.com/realms/cerise" \
  -e AUTH_AUDIENCE="hex-core-service" \
  hex-core-service:local
```

### Image Contents

The runtime image includes:
- `/usr/local/bin/hex-core-service` — the main binary
- Minimal dynamic library dependencies (libc, OpenSSL)
- No proprietary code or adapters
- Optional: read-only mount point for artifact cache (when `REGISTRY_CACHE_ENABLED=true`)

**Image Size:** Approximately 50-80 MB (compressed)

---

## Image Tags and Versioning

### Tag Strategy

The CI/CD pipeline produces the following immutable tags:

| Tag Format | Example | Purpose |
|------------|---------|---------|
| `<version>` | `v1.2.0` | Semantic version from git tag |
| `<git-sha>` | `a1b2c3d` | Short commit SHA (always unique) |
| `latest` | `latest` | Most recent release on `main` |

### `latest` Tag Policy

**Important:** The `latest` tag is **not** a rolling tag for the `main` branch.

- `latest` tracks the most recent **tagged release** (e.g., `v1.2.0`)
- `latest` is **never** published from:
  - Feature branches
  - Pre-release tags (e.g., `v1.2.0-rc1`)
  - Untagged commits
- `latest` always points to a stable, tested release

### Tag Examples

After releasing version `v1.2.0` from `main` at commit `a1b2c3d`, the following tags are pushed:

```
<registry>/<namespace>/hex-core-service:v1.2.0
<registry>/<namespace>/hex-core-service:a1b2c3d
<registry>/<namespace>/hex-core-service:latest
```

**For production deployments:** Always use explicit version tags (`v1.2.0`) or commit SHAs, never `latest`.

---

## Environment Configuration

All runtime configuration is via environment variables. See the [Configuration Guide](configuration.md) for the complete reference.

### Minimal Configuration

Required variables for startup:

```bash
REGISTRY_MODE=catalog
REGISTRY_CATALOG_URL=https://example.org/catalog.json
IO_ADAPTER_ID=memory
AUTH_JWKS_URL=https://keycloak.example.com/realms/cerise/protocol/openid-connect/certs
AUTH_ISSUER=https://keycloak.example.com/realms/cerise
AUTH_AUDIENCE=hex-core-service
```

### Using ConfigMap and Secrets

Kubernetes example:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: hex-core-config
data:
  REGISTRY_MODE: "catalog"
  REGISTRY_CATALOG_URL: "https://config.example.org/hex-core/catalog.json"
  REGISTRY_ALLOWED_HOSTS: "codeberg.org,config.example.org"
  REGISTRY_REQUIRE_HTTPS: "true"
  IO_ADAPTER_ID: "circularise"
  IO_ADAPTER_VERSION: "v1"
  SERVER_PORT: "8080"
  LOG_LEVEL: "info"
  METRICS_ENABLED: "true"

---
apiVersion: v1
kind: Secret
metadata:
  name: hex-core-secrets
type: Opaque
stringData:
  AUTH_JWKS_URL: "https://keycloak.example.com/realms/cerise/protocol/openid-connect/certs"
  AUTH_ISSUER: "https://keycloak.example.com/realms/cerise"
  AUTH_AUDIENCE: "hex-core-service"
  IO_ADAPTER_BASE_URL: "https://io-adapter.internal.example.com"
```

### GitOps Catalog Deployment Pattern

Use a single catalog artifact as the registry source of truth:

```json
{
  "models": [
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "base_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/"
    }
  ]
}
```

Recommended flow:

1. Update `catalog.json` via GitOps pull request.
2. Publish catalog to stable URL (or mount as file in cluster).
3. Trigger `POST /admin/registry/refresh`.
4. Confirm `GET /models` reflects the new catalog.

No service restart is required for model list changes.

---

## Container Registry

### Target Registry

**Primary:** Scaleway Container Registry

- **Registry:** `rg.fr-par.scw.cloud`
- **Namespace:** `ce-rise`
- **Repository:** `hex-core-service`

**Full Image Path:**
```
rg.fr-par.scw.cloud/ce-rise/hex-core-service:<tag>
```

### Authentication

```bash
# Login to Scaleway Container Registry
docker login rg.fr-par.scw.cloud -u <username> -p <secret-key>

# Pull an image
docker pull rg.fr-par.scw.cloud/ce-rise/hex-core-service:v1.2.0
```

For Kubernetes, create an image pull secret:

```bash
kubectl create secret docker-registry scaleway-registry \
  --docker-server=rg.fr-par.scw.cloud \
  --docker-username=<username> \
  --docker-password=<secret-key>
```

### Image Retention

- **Version tags** (`v1.2.0`): Retained indefinitely
- **Commit SHA tags**: Retained for 90 days
- **`latest` tag**: Always points to the most recent release

---

## Kubernetes Deployment

### Deployment Manifest

Example deployment with all recommended practices:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: hex-core-service
  namespace: cerise
spec:
  replicas: 3
  selector:
    matchLabels:
      app: hex-core-service
  template:
    metadata:
      labels:
        app: hex-core-service
        version: v1.2.0
    spec:
      imagePullSecrets:
        - name: scaleway-registry
      containers:
        - name: hex-core-service
          image: rg.fr-par.scw.cloud/ce-rise/hex-core-service:v1.2.0
          ports:
            - containerPort: 8080
              name: http
          envFrom:
            - configMapRef:
                name: hex-core-config
            - secretRef:
                name: hex-core-secrets
          resources:
            requests:
              cpu: 100m
              memory: 128Mi
            limits:
              cpu: 500m
              memory: 512Mi
          livenessProbe:
            httpGet:
              path: /admin/health
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
            timeoutSeconds: 3
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /admin/ready
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 3
          securityContext:
            runAsNonRoot: true
            runAsUser: 1000
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop:
                - ALL
```

### Service Manifest

```yaml
apiVersion: v1
kind: Service
metadata:
  name: hex-core-service
  namespace: cerise
spec:
  selector:
    app: hex-core-service
  ports:
    - name: http
      port: 80
      targetPort: 8080
  type: ClusterIP
```

### Ingress (Optional)

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: hex-core-service
  namespace: cerise
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
    - hosts:
        - api.cerise.example.com
      secretName: hex-core-tls
  rules:
    - host: api.cerise.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: hex-core-service
                port:
                  number: 80
```

### Deployment Steps

```bash
# Apply all manifests
kubectl apply -f deploy/k8s/configmap.yaml
kubectl apply -f deploy/k8s/secret.yaml
kubectl apply -f deploy/k8s/deployment.yaml
kubectl apply -f deploy/k8s/service.yaml

# Check rollout status
kubectl rollout status deployment/hex-core-service -n cerise

# Verify pods are ready
kubectl get pods -n cerise -l app=hex-core-service

# Check logs
kubectl logs -f deployment/hex-core-service -n cerise

# Test health endpoint
kubectl port-forward -n cerise svc/hex-core-service 8080:80
curl http://localhost:8080/admin/health
```

---

## Release Process

### Automated Release Pipeline

Releases are fully automated via CI/CD. To create a new release:

1. **Ensure all tests pass** on `main` branch
2. **Tag the commit** with semantic version:
   ```bash
   git tag -a v1.2.0 -m "Release v1.2.0"
   git push origin v1.2.0
   ```
3. **CI/CD automatically:**
   - Runs full test suite (including integration tests)
   - Builds release binary (`cargo build --release`)
   - Builds `hex-cli` archives for Linux/macOS/Windows and uploads them as release-pipeline artifacts
   - Optionally generates SDKs (TypeScript, Python, Go) from repo OpenAPI specs
   - Builds Docker image
   - Pushes image with version, SHA, and `latest` tags
   - Mirrors tag to GitHub
   - Optionally publishes `hex-cli` to crates.io and generates Homebrew/Scoop manifests when enabled
   - Optionally publishes generated SDKs to npm, PyPI, and a dedicated Go module repository
   - Optionally generates typed Rust stubs from IO adapter OpenAPI for evaluation against hand-written `io-http` client

### OpenAPI Spec Release Model

OpenAPI specs are released via git history and tags, not as separate CI artifacts:

- Source of truth:
  - `crates/api/src/openapi.json`
  - `crates/io-http/src/io_adapter_openapi.json`
- Versioning:
  - semantic git tags (`vX.Y.Z`) identify the released spec version.
- Commit-time CI:
  - `.forgejo/workflows/openapi-ci.yml` creates/validates OpenAPI JSON on every push/PR.

### Release Checklist

Before tagging a release:

- [ ] All CI checks pass on `main`
- [ ] `CHANGELOG.md` updated with release notes
- [ ] `CITATION.cff` version and date updated
- [ ] Documentation reflects new features/changes
- [ ] Breaking changes are clearly documented
- [ ] Migration guide provided (if applicable)

### Optional CLI Distribution Toggles

The release workflow keeps CLI publishing disabled by default. Enable explicitly when needed:

- `CLI_DISTRIBUTION_ENABLED=true`
  - Activates optional CLI distribution job.
  - Downloads built CLI archives and generates:
    - `hex-cli.rb` (Homebrew formula)
    - `hex-cli.scoop.json` (Scoop manifest)
- `CLI_CRATES_IO_PUBLISH=true`
  - Runs `cargo publish -p hex-cli`.
  - Requires `CARGO_REGISTRY_TOKEN` secret.
- `CLI_RELEASE_BASE_URL` (optional)
  - Required when `CLI_DISTRIBUTION_ENABLED=true`.
  - Base URL used inside generated Homebrew/Scoop manifests.

### Optional SDK Generation and Publication Toggles

SDK generation and publishing are disabled by default. Enable explicitly in CI variables/secrets:

- `SDK_GENERATION_ENABLED=true`
  - Generates TypeScript (`typescript-fetch`), Python, and Go SDKs from API OpenAPI.
  - Uploads generated SDKs as workflow artifacts.
- `SDK_PUBLISH_NPM_ENABLED=true`
  - Publishes TypeScript SDK to npm.
  - Requires `NPM_TOKEN` secret.
- `SDK_PUBLISH_PYPI_ENABLED=true`
  - Builds and publishes Python SDK to PyPI.
  - Requires `PYPI_API_TOKEN` secret.
- `SDK_PUBLISH_GO_ENABLED=true`
  - Pushes generated Go SDK to dedicated repository and tags with release version.
  - Requires:
    - `GO_SDK_REPO` variable (`owner/repo`)
    - `GO_SDK_REPO_TOKEN` secret
    - Optional `GO_SDK_BRANCH` variable (default `main`)
- `IO_HTTP_TYPED_CLIENT_EVAL_ENABLED=true`
  - Generates Rust typed stubs from `crates/io-http/src/io_adapter_openapi.json`.
  - Uploads generated stub plus evaluation note as artifacts.

### Cross-Forge Mirroring

**Source of Truth:** Codeberg (`https://codeberg.org/CE-RISE-software/hex-core-service`)

**Mirror:** GitHub (`https://github.com/CE-RISE-software/hex-core-service`)

The GitHub mirror is **read-only** and used for:
- Release archival
- Zenodo DOI integration
- Broader discoverability

**Mirror Pipeline:**

| Event | Action |
|-------|--------|
| Tag `v*.*.*` pushed on Codeberg | Mirror sync propagates the tag to GitHub |
| Tag arrives on GitHub | GitHub Actions creates a Release automatically |
| GitHub Release published | Zenodo archives snapshot and mints DOI |
| Mirror failure | Alert logged; does **not** fail Codeberg pipeline |

### Rollback Procedure

If a release has critical issues:

```bash
# Kubernetes rollback to previous revision
kubectl rollout undo deployment/hex-core-service -n cerise

# Or rollback to a specific version
kubectl set image deployment/hex-core-service -n cerise \
  hex-core-service=rg.fr-par.scw.cloud/ce-rise/hex-core-service:v1.1.0

# Verify rollback
kubectl rollout status deployment/hex-core-service -n cerise
```

**Note:** Git tags are never deleted. If a release is critically flawed, tag a new patch version with fixes.

---

## Production Readiness Checklist

Before deploying to production:

- [ ] All required environment variables configured
- [ ] Secrets stored in Kubernetes Secrets (never in ConfigMaps)
- [ ] Resource requests and limits set appropriately
- [ ] Health and readiness probes configured
- [ ] Logging level set to `info` or `warn`
- [ ] Metrics endpoint enabled (`METRICS_ENABLED=true`)
- [ ] Prometheus scraping configured
- [ ] Alert rules defined and tested
- [ ] Network policies restrict egress to required services
- [ ] JWKS URL is reachable from the cluster
- [ ] IO adapter service is accessible
- [ ] Registry URLs are accessible (or allowlist configured)
- [ ] Backup and disaster recovery plan documented
- [ ] Runbook reviewed by operations team
