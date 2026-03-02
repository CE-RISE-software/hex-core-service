//! JWT validation middleware for the REST inbound adapter.
//!
//! Responsibilities (see AGENTS.md §10):
//! - Fetch and cache Keycloak JWKS from AUTH_JWKS_URL.
//! - Verify JWT signature, `iss`, `aud`, `exp`, `nbf`.
//! - Map validated claims to a `SecurityContext`.
//!
//! The core crate has zero knowledge of this module.

use hex_core::domain::auth::SecurityContext;

/// Claims extracted from a validated Keycloak JWT.
#[derive(Debug, serde::Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: serde_json::Value,
    pub exp: u64,
    pub nbf: Option<u64>,
    #[serde(default)]
    pub realm_access: Option<RealmAccess>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct RealmAccess {
    #[serde(default)]
    pub roles: Vec<String>,
}

impl Claims {
    /// Convert validated claims into the minimal `SecurityContext` the core expects.
    pub fn into_security_context(self, raw_token: Option<String>) -> SecurityContext {
        let roles = self.realm_access.map(|r| r.roles).unwrap_or_default();

        let scopes = self
            .scope
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_owned)
            .collect();

        SecurityContext {
            subject: self.sub,
            roles,
            scopes,
            tenant: None,
            raw_token,
        }
    }
}

// TODO: implement axum middleware/extractor that:
//   1. Extracts the Bearer token from the Authorization header.
//   2. Fetches/refreshes the JWKS from AUTH_JWKS_URL (cached by AUTH_JWKS_REFRESH_SECS).
//   3. Decodes and verifies the JWT using jsonwebtoken.
//   4. Rejects the request with 401 on any validation failure.
//   5. Calls Claims::into_security_context and inserts it into request extensions.
