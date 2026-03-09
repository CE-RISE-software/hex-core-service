# Authentication

This page explains authentication options for service operators and API consumers.

## Overview

`hex-core-service` supports three runtime auth modes selected with `AUTH_MODE`:

- `jwt_jwks`
- `forward_auth`
- `none` (isolated non-production only)

All endpoints except `GET /admin/health` require authentication.

## Auth Mode by Scenario

| Scenario | Recommended mode | Why |
|---|---|---|
| Public/production API with OIDC provider | `jwt_jwks` | Service validates JWT directly against JWKS |
| Enterprise gateway already handles auth (OIDC/SAML/LDAP/introspection) | `forward_auth` | Gateway is source of truth; service trusts injected identity headers |
| Local dev or isolated integration testing | `none` | Lets you run flows without IdP/gateway setup |

## Mode 1: `jwt_jwks` (default)

Direct bearer JWT validation in this service.

Required variables:

- `AUTH_MODE=jwt_jwks`
- `AUTH_JWKS_URL`
- `AUTH_ISSUER`
- `AUTH_AUDIENCE`
- optional `AUTH_JWKS_REFRESH_SECS` (default `3600`)

Expected request header:

```http
Authorization: Bearer <token>
```

Current claim mapping:

- subject: `sub`
- roles: `realm_access.roles`
- scopes: `scope` (space-separated)

## Mode 2: `forward_auth`

Use this when an upstream gateway/proxy/mesh authenticates users and forwards identity headers.

Required variable:

- `AUTH_MODE=forward_auth`

Header mapping variables (defaults):

- `AUTH_FORWARD_SUBJECT_HEADER=x-auth-subject`
- `AUTH_FORWARD_ROLES_HEADER=x-auth-roles`
- `AUTH_FORWARD_SCOPES_HEADER=x-auth-scopes`
- `AUTH_FORWARD_TENANT_HEADER` (optional)
- `AUTH_FORWARD_TOKEN_HEADER` (optional)

Example forwarded headers:

```http
x-auth-subject: user-123
x-auth-roles: admin,qa
x-auth-scopes: records:read records:write
x-auth-tenant: tenant-a
```

Security requirement:

- accept these headers only from trusted upstream infrastructure.

## Mode 3: `none` (non-production only)

Disables authentication checks and injects a fixed local identity.

Required variables:

- `AUTH_MODE=none`
- `AUTH_ALLOW_INSECURE_NONE=true`

Optional identity variables:

- `AUTH_NONE_SUBJECT` (default `dev-anonymous`)
- `AUTH_NONE_ROLES`
- `AUTH_NONE_SCOPES`
- `AUTH_NONE_TENANT`

Safety behavior:

- startup fails if `AUTH_ALLOW_INSECURE_NONE` is not explicitly `true`.

## Integration Notes

### OIDC providers

Works with Keycloak and other OIDC providers that expose JWKS and compatible claims.

### SAML environments

Recommended pattern:

1. Handle SAML in gateway/identity broker.
2. Use `forward_auth` into this service.

### OAuth2 opaque tokens

Recommended pattern:

1. Introspect token in gateway.
2. Forward resolved identity via `forward_auth`.

## Quick Troubleshooting

- `401 Unauthorized`: invalid/missing token, issuer/audience mismatch, expired token.
- `403 Forbidden`: identity authenticated but not allowed by upstream policy.
- `forward_auth` issues: header names do not match configured env vars.
