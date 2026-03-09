use hex_core::domain::{
    error::RegistryError,
    model::{ArtifactSet, ModelId, ModelVersion},
};

/// Resolves model artifacts from a remote URL registry using a configurable
/// URL template of the form:
///   https://codeberg.org/CE-RISE-models/{model}/src/tag/pages-v{version}/generated/
#[derive(Debug, Clone)]
pub struct UrlArtifactRegistry {
    client: reqwest::Client,
    url_template: String,
    allowed_hosts: Vec<String>,
    require_https: bool,
    artifact_map: ArtifactFileMap,
}

/// Configurable filenames for each artifact type.
/// Defaults are chosen for conservative URL parsing and validation.
#[derive(Debug, Clone)]
pub struct ArtifactFileMap {
    pub route: String,
    pub schema: String,
    pub shacl: String,
    pub owl: String,
    pub openapi: String,
}

impl Default for ArtifactFileMap {
    fn default() -> Self {
        Self {
            route: "route.json".into(),
            schema: "schema.json".into(),
            shacl: "shacl.ttl".into(),
            owl: "owl.ttl".into(),
            openapi: "openapi.json".into(),
        }
    }
}

impl UrlArtifactRegistry {
    pub fn new(
        url_template: String,
        allowed_hosts: Vec<String>,
        require_https: bool,
        artifact_map: ArtifactFileMap,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            url_template,
            allowed_hosts,
            require_https,
            artifact_map,
        }
    }

    /// Interpolate the URL template for a given model and version.
    pub fn build_base_url(
        &self,
        model: &ModelId,
        version: &ModelVersion,
    ) -> Result<String, RegistryError> {
        let url = self
            .url_template
            .replace("{model}", &model.0)
            .replace("{version}", &version.0);

        self.normalize_and_validate_base_url(url)
    }

    fn normalize_and_validate_base_url(&self, mut url: String) -> Result<String, RegistryError> {
        if !url.ends_with('/') {
            url.push('/');
        }
        self.validate_url(&url)?;
        Ok(url)
    }

    /// Enforce HTTPS and allowed-host policy.
    fn validate_url(&self, url: &str) -> Result<(), RegistryError> {
        if self.require_https && !url.starts_with("https://") {
            return Err(RegistryError::InsecureUrl {
                url: url.to_string(),
            });
        }

        if !self.allowed_hosts.is_empty() {
            let host = url
                .strip_prefix("https://")
                .or_else(|| url.strip_prefix("http://"))
                .and_then(|s| s.split('/').next())
                .unwrap_or("");

            if !self.allowed_hosts.iter().any(|h| h == host) {
                return Err(RegistryError::DisallowedHost {
                    host: host.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Fetch a single artifact file from the resolved base URL.
    /// Returns `None` if the server responds with 404.
    /// Returns `Err` for all other non-success responses.
    async fn fetch_optional(
        &self,
        base_url: &str,
        filename: &str,
    ) -> Result<Option<String>, RegistryError> {
        let url = format!("{}{}", base_url, filename);

        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| RegistryError::FetchFailed {
                    url: url.clone(),
                    reason: e.to_string(),
                })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(RegistryError::FetchFailed {
                url,
                reason: format!("HTTP {}", response.status()),
            });
        }

        let text = response
            .text()
            .await
            .map_err(|e| RegistryError::FetchFailed {
                url: url.clone(),
                reason: e.to_string(),
            })?;

        Ok(Some(text))
    }

    /// Resolve all artifacts for a (model, version) pair.
    /// `route.json` is required; all others are optional.
    pub async fn resolve_artifacts(
        &self,
        model: &ModelId,
        version: &ModelVersion,
    ) -> Result<ArtifactSet, RegistryError> {
        let base_url = self.build_base_url(model, version)?;
        self.resolve_artifacts_from_base_url(model, version, &base_url)
            .await
    }

    /// Resolve artifacts from a fully qualified base URL.
    /// Used by catalog-backed registries that already provide explicit base URLs.
    pub async fn resolve_artifacts_from_base_url(
        &self,
        model: &ModelId,
        version: &ModelVersion,
        base_url: &str,
    ) -> Result<ArtifactSet, RegistryError> {
        let base_url = self.normalize_and_validate_base_url(base_url.to_string())?;

        // route.json is required
        let route_text = self
            .fetch_optional(&base_url, &self.artifact_map.route)
            .await?
            .ok_or_else(|| RegistryError::ModelNotFound {
                model: model.0.clone(),
                version: version.0.clone(),
            })?;

        let route = serde_json::from_str(&route_text).map_err(|e| RegistryError::FetchFailed {
            url: format!("{}{}", base_url, self.artifact_map.route),
            reason: format!("invalid JSON: {e}"),
        })?;

        // Optional artifacts
        let schema = self
            .fetch_optional(&base_url, &self.artifact_map.schema)
            .await?;
        let shacl = self
            .fetch_optional(&base_url, &self.artifact_map.shacl)
            .await?;
        let owl = self
            .fetch_optional(&base_url, &self.artifact_map.owl)
            .await?;
        let openapi = self
            .fetch_optional(&base_url, &self.artifact_map.openapi)
            .await?;

        Ok(ArtifactSet {
            route: Some(route),
            schema,
            shacl,
            owl,
            openapi,
        })
    }
}
