# Authentication

This page describes all practical authentication integration patterns for `hex-core-service`.

## Overview

Authentication is implemented in the API adapter and translated into core `SecurityContext`.

- Core (`crates/core`) remains auth-agnostic.
- API (`crates/api`) enforces auth in middleware.
- All endpoints except `/admin/health` require authentication.

## Auth Architecture

`hex-core-service` uses a pluggable auth provider interface in the API layer:

- `AuthProvider` receives request context (headers)
- Provider authenticates caller
- Provider produces `SecurityContext` with:
  - `subject`
  - `roles`
  - `scopes`
  - optional `tenant`
  - optional `raw_token`

This design allows switching auth methods without changing core use-case logic.

## Supported Modes

Set `AUTH_MODE` to choose auth behavior.

### 1. `jwt_jwks` (default)

Direct bearer JWT validation in this service.

Required config:

- `AUTH_MODE=jwt_jwks`
- `AUTH_JWKS_URL`
- `AUTH_ISSUER`
- `AUTH_AUDIENCE`
- optional `AUTH_JWKS_REFRESH_SECS` (default `3600`)

Flow:

1. Read `Authorization: Bearer <token>`.
2. Decode JWT header and resolve `kid`.
3. Fetch/cache JWKS from `AUTH_JWKS_URL`.
4. Verify signature and claims (`iss`, `aud`, time constraints).
5. Map claims into `SecurityContext`.

Current claim mapping:

- subject: `sub`
- roles: `realm_access.roles` (Keycloak-compatible)
- scopes: `scope` (space-separated)

Compatibility:

- Works best with OIDC providers issuing RSA-signed JWTs with JWKS.
- Keycloak is a native fit.
- Other OIDC providers can work if claims are compatible with mapping.

### 2. `forward_auth`

Trust identity headers from an upstream gateway/proxy/mesh that already authenticated the caller.

Required config:

- `AUTH_MODE=forward_auth`

Header mapping config (optional; defaults shown):

- `AUTH_FORWARD_SUBJECT_HEADER=x-auth-subject`
- `AUTH_FORWARD_ROLES_HEADER=x-auth-roles` (comma-separated)
- `AUTH_FORWARD_SCOPES_HEADER=x-auth-scopes` (space-separated)
- `AUTH_FORWARD_TENANT_HEADER` (optional)
- `AUTH_FORWARD_TOKEN_HEADER` (optional)

Flow:

1. Upstream gateway validates identity (OIDC/SAML/LDAP/etc.).
2. Gateway injects trusted headers.
3. Service maps headers into `SecurityContext`.

Security note:

- Use this mode only behind trusted network boundaries.
- Strip external client-supplied auth headers at ingress.
- Allow only gateway-originated traffic to the service.

### 3. `none` (isolated dry-run only)

Disable authentication checks and inject a fixed local `SecurityContext`.

Required config:

- `AUTH_MODE=none`
- `AUTH_ALLOW_INSECURE_NONE=true`

Optional identity injection:

- `AUTH_NONE_SUBJECT` (default `dev-anonymous`)
- `AUTH_NONE_ROLES` (comma-separated)
- `AUTH_NONE_SCOPES` (space-separated)
- `AUTH_NONE_TENANT`

Safety behavior:

- Startup fails if `AUTH_MODE=none` and `AUTH_ALLOW_INSECURE_NONE` is not `true`.
- Intended only for isolated environments (local/dev/test/ephemeral sandboxes).

## Integration Possibilities

### OIDC / OAuth2 JWT (recommended direct mode)

Use `jwt_jwks`.

- Keycloak
- Auth0
- Azure AD / Entra ID
- Okta
- Any compatible OIDC provider

### SAML SSO

SAML is typically not used as direct API bearer auth.

Recommended pattern:

1. Handle SAML in IdP + gateway/identity broker.
2. Gateway either:
   - injects trusted identity headers (`forward_auth`), or
   - performs token exchange to OIDC/JWT and forwards bearer tokens (`jwt_jwks`).

This keeps service auth implementation simple and API-native.

### LDAP / Enterprise Directory

LDAP is usually handled upstream (gateway, broker, or IdP).

- If upstream emits JWTs: use `jwt_jwks`
- If upstream injects identity headers: use `forward_auth`

### Service-to-service / mTLS

mTLS can be integrated via gateway sidecar termination and forwarded identity headers (`forward_auth`).
Direct mTLS auth provider in service is not implemented yet.

### Opaque OAuth2 tokens (introspection)

Not currently implemented in-service.
Recommended today: introspect upstream and forward identity headers (`forward_auth`).

## Deployment Patterns

### Pattern A: Direct JWT validation in service

Good for smaller deployments and simple trust boundaries.

```env
AUTH_MODE=jwt_jwks
AUTH_JWKS_URL=https://id.example.org/realms/cerise/protocol/openid-connect/certs
AUTH_ISSUER=https://id.example.org/realms/cerise
AUTH_AUDIENCE=hex-core-service
AUTH_JWKS_REFRESH_SECS=3600
```

### Pattern B: Gateway-managed auth

Good for enterprises using SAML, legacy providers, or central policy engines.

```env
AUTH_MODE=forward_auth
AUTH_FORWARD_SUBJECT_HEADER=x-auth-subject
AUTH_FORWARD_ROLES_HEADER=x-auth-roles
AUTH_FORWARD_SCOPES_HEADER=x-auth-scopes
AUTH_FORWARD_TENANT_HEADER=x-auth-tenant
```

Example gateway behavior:

- validate OIDC/SAML token
- enforce coarse policy
- inject trusted identity headers
- forward request to service

### Pattern C: Isolated dry-run (no auth)

Use only in isolated non-production environments.

```env
AUTH_MODE=none
AUTH_ALLOW_INSECURE_NONE=true
AUTH_NONE_SUBJECT=dryrun-user
AUTH_NONE_ROLES=admin,developer
AUTH_NONE_SCOPES=records:read records:write
AUTH_NONE_TENANT=sandbox
```

This mode allows testing service workflows without provisioning IdP/gateway auth components.

## Existing Auth Integration Examples

### Example 1: Existing Keycloak / OIDC realm

```env
AUTH_MODE=jwt_jwks
AUTH_JWKS_URL=https://keycloak.example.org/realms/cerise/protocol/openid-connect/certs
AUTH_ISSUER=https://keycloak.example.org/realms/cerise
AUTH_AUDIENCE=hex-core-service
AUTH_JWKS_REFRESH_SECS=3600
```

### Example 2: Existing SAML in gateway (forwarded identity)

Use gateway/broker to validate SAML and forward trusted identity headers.

Service config:

```env
AUTH_MODE=forward_auth
AUTH_FORWARD_SUBJECT_HEADER=x-auth-subject
AUTH_FORWARD_ROLES_HEADER=x-auth-roles
AUTH_FORWARD_SCOPES_HEADER=x-auth-scopes
AUTH_FORWARD_TENANT_HEADER=x-auth-tenant
```

Expected forwarded headers example:

```http
x-auth-subject: user-123
x-auth-roles: admin,qa
x-auth-scopes: records:read records:write
x-auth-tenant: tenant-a
```

### Example 3: Existing API gateway with OAuth2 introspection

If gateway introspects opaque tokens and forwards identity attributes:

- choose `AUTH_MODE=forward_auth`
- map forwarded headers as above
- keep token introspection outside this service

## Security Hardening Checklist

- Keep `/admin/*` behind network controls even with app-layer auth.
- Enforce TLS end-to-end or strict internal mTLS.
- In `forward_auth` mode:
  - drop spoofable auth headers from public ingress
  - only trust headers from known upstream gateway
- In `none` mode:
  - never expose service publicly
  - isolate by namespace/network policy
  - use only temporary or non-production environments
- Rotate keys in IdP/gateway and monitor auth failures.
- Avoid logging raw bearer tokens.

## Troubleshooting

### `401 Unauthorized` with `jwt_jwks`

Check:

1. Token present and formatted as `Bearer <token>`.
2. `AUTH_ISSUER` equals token `iss`.
3. `AUTH_AUDIENCE` matches token `aud`.
4. `AUTH_JWKS_URL` reachable from runtime environment.
5. Token algorithm/key type is compatible with configured validator.

### `401 Unauthorized` with `forward_auth`

Check:

1. Upstream gateway is injecting required subject header.
2. Header names in gateway and service config match exactly.
3. Ingress/proxy is not stripping required forwarded headers.
4. Public clients cannot directly set trusted auth headers.

## Roadmap-Compatible Extensions

This architecture can be extended with additional providers without touching core:

- OIDC discovery provider (`/.well-known/openid-configuration`)
- OAuth2 introspection provider
- mTLS certificate-based provider
- Multi-provider chains (fallback/priority)
