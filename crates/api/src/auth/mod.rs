use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use axum::{
    body::Body,
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use hex_core::domain::auth::SecurityContext;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct RequestContext {
    pub headers: HeaderMap,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, request: &RequestContext) -> Result<SecurityContext, ApiError>;
}

pub type AuthProviderHandle = Arc<dyn AuthProvider>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    JwtJwks,
    ForwardAuth,
    None,
}

impl AuthMode {
    pub fn from_env() -> Result<Self, ApiError> {
        let mode = std::env::var("AUTH_MODE").unwrap_or_else(|_| "jwt_jwks".into());
        match mode.as_str() {
            "jwt_jwks" => Ok(Self::JwtJwks),
            "forward_auth" => Ok(Self::ForwardAuth),
            "none" => Ok(Self::None),
            other => Err(ApiError::Internal(format!(
                "unsupported AUTH_MODE='{other}', expected one of: jwt_jwks, forward_auth, none"
            ))),
        }
    }
}

pub fn build_provider_from_env() -> Result<AuthProviderHandle, ApiError> {
    match AuthMode::from_env()? {
        AuthMode::JwtJwks => {
            let config = JwtJwksConfig::from_env()?;
            Ok(Arc::new(JwtJwksProvider::new(config)))
        }
        AuthMode::ForwardAuth => {
            let config = ForwardAuthConfig::from_env();
            Ok(Arc::new(ForwardAuthProvider::new(config)))
        }
        AuthMode::None => {
            let config = NoAuthConfig::from_env()?;
            Ok(Arc::new(NoAuthProvider::new(config)))
        }
    }
}

pub async fn require_auth(
    State(provider): State<AuthProviderHandle>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let ctx = RequestContext {
        headers: req.headers().clone(),
    };
    let security_context = provider.authenticate(&ctx).await?;
    req.extensions_mut().insert(security_context);
    Ok(next.run(req).await)
}

#[derive(Debug, Clone)]
pub struct JwtJwksConfig {
    pub jwks_url: String,
    pub issuer: String,
    pub audience: String,
    pub jwks_refresh_secs: u64,
}

impl JwtJwksConfig {
    pub fn from_env() -> Result<Self, ApiError> {
        let jwks_url = std::env::var("AUTH_JWKS_URL")
            .map_err(|_| ApiError::Internal("missing AUTH_JWKS_URL".into()))?;
        let issuer = std::env::var("AUTH_ISSUER")
            .map_err(|_| ApiError::Internal("missing AUTH_ISSUER".into()))?;
        let audience = std::env::var("AUTH_AUDIENCE")
            .map_err(|_| ApiError::Internal("missing AUTH_AUDIENCE".into()))?;
        let jwks_refresh_secs = std::env::var("AUTH_JWKS_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);

        Ok(Self {
            jwks_url,
            issuer,
            audience,
            jwks_refresh_secs,
        })
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: String,
    #[serde(default)]
    pub realm_access: Option<RealmAccess>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RealmAccess {
    #[serde(default)]
    pub roles: Vec<String>,
}

impl JwtClaims {
    pub fn into_security_context(self, raw_token: Option<String>) -> SecurityContext {
        let roles = self.realm_access.map(|r| r.roles).unwrap_or_default();
        let scopes = self
            .scope
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
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

#[derive(Clone)]
pub struct JwtJwksProvider {
    config: JwtJwksConfig,
    client: reqwest::Client,
    cache: Arc<RwLock<Option<CachedJwks>>>,
}

#[derive(Clone)]
struct CachedJwks {
    fetched_at: Instant,
    by_kid: HashMap<String, DecodingKey>,
    any_key: Option<DecodingKey>,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

impl JwtJwksProvider {
    pub fn new(config: JwtJwksConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn resolve_key(&self, kid: Option<&str>) -> Result<DecodingKey, ApiError> {
        let now = Instant::now();
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                let fresh = now.duration_since(cached.fetched_at)
                    < Duration::from_secs(self.config.jwks_refresh_secs);
                if fresh {
                    if let Some(found) = kid.and_then(|k| cached.by_kid.get(k)) {
                        return Ok(found.clone());
                    }
                    if kid.is_none() {
                        if let Some(key) = cached.any_key.as_ref() {
                            return Ok(key.clone());
                        }
                    }
                }
            }
        }

        let refreshed = self.fetch_jwks().await?;
        {
            let mut cache = self.cache.write().await;
            *cache = Some(refreshed.clone());
        }

        if let Some(k) = kid {
            refreshed
                .by_kid
                .get(k)
                .cloned()
                .ok_or_else(|| ApiError::Unauthorized(format!("no JWKS key found for kid '{k}'")))
        } else {
            refreshed
                .any_key
                .ok_or_else(|| ApiError::Unauthorized("JWKS contains no usable RSA keys".into()))
        }
    }

    async fn fetch_jwks(&self) -> Result<CachedJwks, ApiError> {
        let response = self
            .client
            .get(&self.config.jwks_url)
            .send()
            .await
            .map_err(|e| ApiError::Unauthorized(format!("failed to fetch JWKS: {e}")))?;

        if !response.status().is_success() {
            return Err(ApiError::Unauthorized(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| ApiError::Unauthorized(format!("invalid JWKS response: {e}")))?;

        let mut by_kid = HashMap::new();
        let mut any_key = None;
        for key in jwks.keys {
            if key.kty != "RSA" {
                continue;
            }
            let Some(n) = key.n else { continue };
            let Some(e) = key.e else { continue };
            let decoding_key = DecodingKey::from_rsa_components(&n, &e)
                .map_err(|err| ApiError::Unauthorized(format!("invalid JWKS RSA key: {err}")))?;
            if any_key.is_none() {
                any_key = Some(decoding_key.clone());
            }
            if let Some(kid) = key.kid {
                by_kid.insert(kid, decoding_key);
            }
        }

        Ok(CachedJwks {
            fetched_at: Instant::now(),
            by_kid,
            any_key,
        })
    }
}

#[async_trait]
impl AuthProvider for JwtJwksProvider {
    async fn authenticate(&self, request: &RequestContext) -> Result<SecurityContext, ApiError> {
        let token = extract_bearer(&request.headers)?;
        let header = decode_header(token)
            .map_err(|e| ApiError::Unauthorized(format!("invalid JWT header: {e}")))?;
        let key = self.resolve_key(header.kid.as_deref()).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.config.issuer.as_str()]);
        validation.set_audience(&[self.config.audience.as_str()]);

        let decoded = decode::<JwtClaims>(token, &key, &validation)
            .map_err(|e| ApiError::Unauthorized(format!("token validation failed: {e}")))?;

        Ok(decoded
            .claims
            .into_security_context(Some(token.to_string())))
    }
}

#[derive(Debug, Clone)]
pub struct ForwardAuthConfig {
    pub subject_header: String,
    pub roles_header: String,
    pub scopes_header: String,
    pub tenant_header: Option<String>,
    pub token_header: Option<String>,
}

impl ForwardAuthConfig {
    pub fn from_env() -> Self {
        Self {
            subject_header: std::env::var("AUTH_FORWARD_SUBJECT_HEADER")
                .unwrap_or_else(|_| "x-auth-subject".into()),
            roles_header: std::env::var("AUTH_FORWARD_ROLES_HEADER")
                .unwrap_or_else(|_| "x-auth-roles".into()),
            scopes_header: std::env::var("AUTH_FORWARD_SCOPES_HEADER")
                .unwrap_or_else(|_| "x-auth-scopes".into()),
            tenant_header: std::env::var("AUTH_FORWARD_TENANT_HEADER").ok(),
            token_header: std::env::var("AUTH_FORWARD_TOKEN_HEADER").ok(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForwardAuthProvider {
    config: ForwardAuthConfig,
}

impl ForwardAuthProvider {
    pub fn new(config: ForwardAuthConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AuthProvider for ForwardAuthProvider {
    async fn authenticate(&self, request: &RequestContext) -> Result<SecurityContext, ApiError> {
        let subject = header_value(&request.headers, &self.config.subject_header)
            .ok_or_else(|| ApiError::Unauthorized("missing forwarded subject header".into()))?;

        let roles_raw =
            header_value(&request.headers, &self.config.roles_header).unwrap_or_default();
        let roles = roles_raw
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        let scopes_raw =
            header_value(&request.headers, &self.config.scopes_header).unwrap_or_default();
        let scopes = scopes_raw
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();

        let tenant = self
            .config
            .tenant_header
            .as_ref()
            .and_then(|h| header_value(&request.headers, h))
            .filter(|v| !v.is_empty());

        let raw_token = if let Some(token_header) = &self.config.token_header {
            header_value(&request.headers, token_header)
        } else {
            request
                .headers
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(str::to_string)
        };

        Ok(SecurityContext {
            subject,
            roles,
            scopes,
            tenant,
            raw_token,
        })
    }
}

fn extract_bearer(headers: &HeaderMap) -> Result<&str, ApiError> {
    let raw = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".into()))?;
    raw.strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("expected Bearer token".into()))
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .map(str::to_string)
}

#[derive(Debug, Clone)]
pub struct NoAuthConfig {
    pub allow_insecure_none: bool,
    pub subject: String,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
    pub tenant: Option<String>,
}

impl NoAuthConfig {
    pub fn from_env() -> Result<Self, ApiError> {
        let allow_insecure_none = std::env::var("AUTH_ALLOW_INSECURE_NONE")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);
        if !allow_insecure_none {
            return Err(ApiError::Internal(
                "AUTH_MODE=none is disabled unless AUTH_ALLOW_INSECURE_NONE=true".into(),
            ));
        }

        let subject = std::env::var("AUTH_NONE_SUBJECT").unwrap_or_else(|_| "dev-anonymous".into());
        let roles = std::env::var("AUTH_NONE_ROLES")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let scopes = std::env::var("AUTH_NONE_SCOPES")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let tenant = std::env::var("AUTH_NONE_TENANT")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        Ok(Self {
            allow_insecure_none,
            subject,
            roles,
            scopes,
            tenant,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NoAuthProvider {
    config: NoAuthConfig,
}

impl NoAuthProvider {
    pub fn new(config: NoAuthConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AuthProvider for NoAuthProvider {
    async fn authenticate(&self, _request: &RequestContext) -> Result<SecurityContext, ApiError> {
        if !self.config.allow_insecure_none {
            return Err(ApiError::Unauthorized(
                "no-auth provider is not enabled".into(),
            ));
        }

        Ok(SecurityContext {
            subject: self.config.subject.clone(),
            roles: self.config.roles.clone(),
            scopes: self.config.scopes.clone(),
            tenant: self.config.tenant.clone(),
            raw_token: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn empty_request() -> RequestContext {
        RequestContext {
            headers: HeaderMap::new(),
        }
    }

    #[tokio::test]
    async fn no_auth_provider_injects_configured_identity() {
        let provider = NoAuthProvider::new(NoAuthConfig {
            allow_insecure_none: true,
            subject: "dryrun-user".into(),
            roles: vec!["admin".into(), "qa".into()],
            scopes: vec!["records:read".into(), "records:write".into()],
            tenant: Some("sandbox".into()),
        });

        let ctx = provider
            .authenticate(&empty_request())
            .await
            .expect("no-auth mode should authenticate");

        assert_eq!(ctx.subject, "dryrun-user");
        assert_eq!(ctx.roles, vec!["admin", "qa"]);
        assert_eq!(ctx.scopes, vec!["records:read", "records:write"]);
        assert_eq!(ctx.tenant.as_deref(), Some("sandbox"));
        assert!(ctx.raw_token.is_none());
    }

    #[tokio::test]
    async fn no_auth_provider_rejects_when_disabled() {
        let provider = NoAuthProvider::new(NoAuthConfig {
            allow_insecure_none: false,
            subject: "dryrun-user".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
        });

        let err = provider
            .authenticate(&empty_request())
            .await
            .expect_err("no-auth mode must reject when disabled");

        assert!(matches!(err, ApiError::Unauthorized(_)));
    }

    #[test]
    fn no_auth_config_from_env_requires_allow_flag() {
        let _guard = env_lock().lock().expect("env lock");

        std::env::remove_var("AUTH_ALLOW_INSECURE_NONE");
        std::env::remove_var("AUTH_NONE_SUBJECT");
        std::env::remove_var("AUTH_NONE_ROLES");
        std::env::remove_var("AUTH_NONE_SCOPES");
        std::env::remove_var("AUTH_NONE_TENANT");

        let err = NoAuthConfig::from_env().expect_err("must fail when allow flag is missing");
        assert!(matches!(err, ApiError::Internal(_)));
    }

    #[test]
    fn no_auth_config_from_env_parses_identity_fields() {
        let _guard = env_lock().lock().expect("env lock");

        std::env::set_var("AUTH_ALLOW_INSECURE_NONE", "true");
        std::env::set_var("AUTH_NONE_SUBJECT", "dryrun-user");
        std::env::set_var("AUTH_NONE_ROLES", "admin,qa");
        std::env::set_var("AUTH_NONE_SCOPES", "records:read records:write");
        std::env::set_var("AUTH_NONE_TENANT", "sandbox");

        let cfg = NoAuthConfig::from_env().expect("env should parse");
        assert!(cfg.allow_insecure_none);
        assert_eq!(cfg.subject, "dryrun-user");
        assert_eq!(cfg.roles, vec!["admin", "qa"]);
        assert_eq!(cfg.scopes, vec!["records:read", "records:write"]);
        assert_eq!(cfg.tenant.as_deref(), Some("sandbox"));

        std::env::remove_var("AUTH_ALLOW_INSECURE_NONE");
        std::env::remove_var("AUTH_NONE_SUBJECT");
        std::env::remove_var("AUTH_NONE_ROLES");
        std::env::remove_var("AUTH_NONE_SCOPES");
        std::env::remove_var("AUTH_NONE_TENANT");
    }
}
