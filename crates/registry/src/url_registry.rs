use hex_core::domain::{
    error::RegistryError,
    model::{ArtifactSet, ModelId, ModelVersion},
};

#[derive(Debug, Clone, Default)]
pub struct ArtifactUrlSet {
    pub schema_url: Option<String>,
    pub shacl_url: Option<String>,
    pub owl_url: Option<String>,
    pub openapi_url: Option<String>,
}

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
    pub schema: String,
    pub shacl: String,
    pub owl: String,
    pub openapi: String,
}

impl Default for ArtifactFileMap {
    fn default() -> Self {
        Self {
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

    /// Fetch a single artifact file from an explicit URL.
    /// Returns `None` if the server responds with 404.
    /// Returns `Err` for all other non-success responses.
    async fn fetch_optional_url(&self, url: &str) -> Result<Option<String>, RegistryError> {
        self.validate_url(url)?;

        let response =
            self.client
                .get(url)
                .send()
                .await
                .map_err(|e| RegistryError::FetchFailed {
                    url: url.to_string(),
                    reason: e.to_string(),
                })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(RegistryError::FetchFailed {
                url: url.to_string(),
                reason: format!("HTTP {}", response.status()),
            });
        }

        let text = response
            .text()
            .await
            .map_err(|e| RegistryError::FetchFailed {
                url: url.to_string(),
                reason: e.to_string(),
            })?;

        Ok(Some(text))
    }

    /// Resolve all artifacts for a (model, version) pair using inferred filenames.
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
        _model: &ModelId,
        _version: &ModelVersion,
        base_url: &str,
    ) -> Result<ArtifactSet, RegistryError> {
        let base_url = self.normalize_and_validate_base_url(base_url.to_string())?;
        self.resolve_artifacts_from_urls(&ArtifactUrlSet {
            schema_url: Some(format!("{}{}", base_url, self.artifact_map.schema)),
            shacl_url: Some(format!("{}{}", base_url, self.artifact_map.shacl)),
            owl_url: Some(format!("{}{}", base_url, self.artifact_map.owl)),
            openapi_url: Some(format!("{}{}", base_url, self.artifact_map.openapi)),
        })
        .await
    }

    /// Resolve artifacts from explicit per-artifact URLs. Missing URLs and `404`
    /// responses both resolve to absent artifacts instead of hard failure.
    pub async fn resolve_artifacts_from_urls(
        &self,
        urls: &ArtifactUrlSet,
    ) -> Result<ArtifactSet, RegistryError> {
        let schema = match urls.schema_url.as_deref() {
            Some(url) => self.fetch_optional_url(url).await?,
            None => None,
        };
        let shacl = match urls.shacl_url.as_deref() {
            Some(url) => self.fetch_optional_url(url).await?,
            None => None,
        };
        let owl = match urls.owl_url.as_deref() {
            Some(url) => self.fetch_optional_url(url).await?,
            None => None,
        };
        let openapi = match urls.openapi_url.as_deref() {
            Some(url) => self.fetch_optional_url(url).await?,
            None => None,
        };

        Ok(ArtifactSet {
            schema,
            shacl,
            owl,
            openapi,
        })
    }
}
