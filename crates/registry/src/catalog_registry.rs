use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use hex_core::{
    domain::{
        error::RegistryError,
        model::{ArtifactSet, ModelDescriptor, ModelId, ModelVersion, RefreshSummary},
    },
    ports::outbound::registry::ArtifactRegistryPort,
};
use tokio::sync::RwLock;
use tracing::warn;

use crate::{
    index::RegistryIndex,
    url_registry::{ArtifactUrlSet, UrlArtifactRegistry},
};

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub model: ModelId,
    pub version: ModelVersion,
    pub artifact_urls: ArtifactUrlSet,
}

#[derive(Debug, serde::Deserialize)]
struct CatalogEntryRaw {
    model: Option<String>,
    version: Option<String>,
    route_url: Option<String>,
    schema_url: Option<String>,
    shacl_url: Option<String>,
    owl_url: Option<String>,
    openapi_url: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum CatalogDocument {
    Entries(Vec<CatalogEntryRaw>),
    Wrapped { models: Vec<CatalogEntryRaw> },
}

impl CatalogEntryRaw {
    fn into_entry(self) -> Result<CatalogEntry, RegistryError> {
        let artifact_urls = ArtifactUrlSet {
            route_url: self.route_url,
            schema_url: self.schema_url,
            shacl_url: self.shacl_url,
            owl_url: self.owl_url,
            openapi_url: self.openapi_url,
        };

        if artifact_urls.route_url.is_none()
            && artifact_urls.schema_url.is_none()
            && artifact_urls.shacl_url.is_none()
            && artifact_urls.owl_url.is_none()
            && artifact_urls.openapi_url.is_none()
        {
            return Err(RegistryError::Internal(
                "catalog entry must declare at least one explicit artifact URL".into(),
            ));
        }

        let parsed = inferred_model_version(&artifact_urls);
        let model = match (self.model, parsed.as_ref().map(|(m, _)| m.clone())) {
            (Some(m), _) => m,
            (None, Some(m)) => m,
            (None, None) => {
                return Err(RegistryError::Internal(format!(
                    "catalog entry missing model and cannot infer it from declared artifact URLs"
                )));
            }
        };
        let version = match (self.version, parsed.map(|(_, v)| v)) {
            (Some(v), _) => v,
            (None, Some(v)) => v,
            (None, None) => {
                return Err(RegistryError::Internal(format!(
                    "catalog entry missing version and cannot infer it from declared artifact URLs"
                )));
            }
        };

        Ok(CatalogEntry {
            model: ModelId(model),
            version: ModelVersion(version),
            artifact_urls,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CatalogArtifactRegistry {
    resolver: UrlArtifactRegistry,
    index: RegistryIndex,
    source: CatalogSource,
}

#[derive(Debug, Clone)]
enum CatalogSource {
    Inline(Arc<RwLock<Vec<CatalogEntry>>>),
    Url(String),
    File(PathBuf),
}

impl CatalogArtifactRegistry {
    pub async fn from_json_catalog(
        catalog_json: &str,
        allowed_hosts: Vec<String>,
        require_https: bool,
    ) -> Result<Self, RegistryError> {
        let entries = parse_entries_from_json(catalog_json)?;
        Self::new(entries, allowed_hosts, require_https).await
    }

    pub async fn new(
        catalog_entries: Vec<CatalogEntry>,
        allowed_hosts: Vec<String>,
        require_https: bool,
    ) -> Result<Self, RegistryError> {
        let source = CatalogSource::Inline(Arc::new(RwLock::new(catalog_entries)));
        Self::with_source(source, allowed_hosts, require_https).await
    }

    pub async fn from_catalog_url(
        catalog_url: impl Into<String>,
        allowed_hosts: Vec<String>,
        require_https: bool,
    ) -> Result<Self, RegistryError> {
        let source = CatalogSource::Url(catalog_url.into());
        Self::with_source(source, allowed_hosts, require_https).await
    }

    pub async fn from_catalog_file(
        catalog_path: impl AsRef<Path>,
        allowed_hosts: Vec<String>,
        require_https: bool,
    ) -> Result<Self, RegistryError> {
        let source = CatalogSource::File(catalog_path.as_ref().to_path_buf());
        Self::with_source(source, allowed_hosts, require_https).await
    }

    pub async fn replace_catalog_from_json(&self, catalog_json: &str) -> Result<(), RegistryError> {
        let entries = parse_entries_from_json(catalog_json)?;
        self.replace_catalog_entries(entries).await
    }

    pub async fn replace_catalog_entries(
        &self,
        entries: Vec<CatalogEntry>,
    ) -> Result<(), RegistryError> {
        validate_unique_model_versions(&entries)?;
        match &self.source {
            CatalogSource::Inline(current) => {
                let mut guard = current.write().await;
                *guard = entries;
                Ok(())
            }
            _ => Err(RegistryError::Internal(
                "replace_catalog_entries is only supported for inline catalog source".into(),
            )),
        }
    }

    async fn with_source(
        source: CatalogSource,
        allowed_hosts: Vec<String>,
        require_https: bool,
    ) -> Result<Self, RegistryError> {
        let resolver = UrlArtifactRegistry::new(
            // Unused in catalog mode; kept to reuse URL policy + fetch logic.
            "https://invalid.local/{model}/{version}/".to_string(),
            allowed_hosts,
            require_https,
            Default::default(),
        );

        let registry = Self {
            resolver,
            index: RegistryIndex::new(),
            source,
        };

        let _ = registry.refresh().await?;
        Ok(registry)
    }

    async fn load_entries(&self) -> Result<Vec<CatalogEntry>, RegistryError> {
        match &self.source {
            CatalogSource::Inline(entries) => Ok(entries.read().await.clone()),
            CatalogSource::Url(url) => {
                let body = reqwest::get(url)
                    .await
                    .map_err(|e| RegistryError::FetchFailed {
                        url: url.clone(),
                        reason: e.to_string(),
                    })?
                    .text()
                    .await
                    .map_err(|e| RegistryError::FetchFailed {
                        url: url.clone(),
                        reason: e.to_string(),
                    })?;
                parse_entries_from_json(&body)
            }
            CatalogSource::File(path) => {
                let body = tokio::fs::read_to_string(path).await.map_err(|e| {
                    RegistryError::Internal(format!(
                        "failed reading catalog file {}: {e}",
                        path.display()
                    ))
                })?;
                parse_entries_from_json(&body)
            }
        }
    }
}

#[async_trait]
impl ArtifactRegistryPort for CatalogArtifactRegistry {
    async fn resolve(
        &self,
        model: &ModelId,
        version: &ModelVersion,
    ) -> Result<ArtifactSet, RegistryError> {
        self.index
            .get(model, version)
            .await
            .ok_or_else(|| RegistryError::ModelNotFound {
                model: model.0.clone(),
                version: version.0.clone(),
            })
    }

    async fn list_models(&self) -> Result<Vec<ModelDescriptor>, RegistryError> {
        Ok(self.index.list().await)
    }

    async fn refresh(&self) -> Result<RefreshSummary, RegistryError> {
        let catalog_entries = self.load_entries().await?;
        validate_unique_model_versions(&catalog_entries)?;

        let mut entries = HashMap::new();
        let mut errors = Vec::new();

        for item in &catalog_entries {
            let result = self
                .resolver
                .resolve_artifacts_from_urls(&item.artifact_urls)
                .await;

            match result {
                Ok(artifacts) => {
                    entries.insert((item.model.clone(), item.version.clone()), artifacts);
                }
                Err(e) => {
                    warn!(
                        model = %item.model,
                        version = %item.version,
                        route_url = ?item.artifact_urls.route_url,
                        error = %e,
                        "failed to resolve catalog entry"
                    );
                    errors.push(format!("{}@{}: {}", item.model.0, item.version.0, e));
                }
            }
        }

        Ok(self.index.swap_with_errors(entries, errors).await)
    }
}

fn parse_entries_from_json(catalog_json: &str) -> Result<Vec<CatalogEntry>, RegistryError> {
    let doc: CatalogDocument = serde_json::from_str(catalog_json)
        .map_err(|e| RegistryError::Internal(format!("invalid catalog JSON: {e}")))?;

    let raw_entries = match doc {
        CatalogDocument::Entries(v) => v,
        CatalogDocument::Wrapped { models } => models,
    };

    let mut entries = Vec::with_capacity(raw_entries.len());
    for raw in raw_entries {
        entries.push(raw.into_entry()?);
    }
    Ok(entries)
}

fn validate_unique_model_versions(entries: &[CatalogEntry]) -> Result<(), RegistryError> {
    let mut seen = HashSet::new();

    for entry in entries {
        let key = (entry.model.0.clone(), entry.version.0.clone());
        if !seen.insert(key.clone()) {
            return Err(RegistryError::Internal(format!(
                "duplicate catalog entry for model={} version={}",
                key.0, key.1
            )));
        }
    }

    Ok(())
}

fn parse_model_version_from_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim_end_matches('/');
    let marker = "/CE-RISE-models/";
    let start = trimmed.find(marker)? + marker.len();
    let rest = &trimmed[start..];
    let mut parts = rest.split('/');

    let model = parts.next()?.to_string();
    let _transport = parts.next()?; // e.g. "src" or "raw"
    let tag = parts.next()?; // "tag"
    if tag != "tag" {
        return None;
    }
    let version_tag = parts.next()?; // "pages-vX.Y.Z"
    let generated = parts.next()?; // "generated"
    if generated != "generated" {
        return None;
    }
    let version = version_tag.strip_prefix("pages-v")?.to_string();
    Some((model, version))
}

fn inferred_model_version(artifact_urls: &ArtifactUrlSet) -> Option<(String, String)> {
    artifact_urls
        .route_url
        .as_deref()
        .and_then(parse_model_version_from_url)
        .or_else(|| {
            artifact_urls
                .schema_url
                .as_deref()
                .and_then(parse_model_version_from_url)
        })
        .or_else(|| {
            artifact_urls
                .shacl_url
                .as_deref()
                .and_then(parse_model_version_from_url)
        })
        .or_else(|| {
            artifact_urls
                .owl_url
                .as_deref()
                .and_then(parse_model_version_from_url)
        })
        .or_else(|| {
            artifact_urls
                .openapi_url
                .as_deref()
                .and_then(parse_model_version_from_url)
        })
}

#[cfg(test)]
mod tests {
    use super::parse_model_version_from_url;
    use crate::catalog_registry::CatalogArtifactRegistry;
    use hex_core::ports::outbound::registry::ArtifactRegistryPort;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn parse_src_tag_pages_url() {
        let (model, version) = parse_model_version_from_url(
            "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated",
        )
        .expect("must parse");
        assert_eq!(model, "re-indicators-specification");
        assert_eq!(version, "0.0.3");
    }

    #[test]
    fn parse_raw_tag_pages_url() {
        let (model, version) = parse_model_version_from_url(
            "https://codeberg.org/CE-RISE-models/dp-record-metadata/raw/tag/pages-v0.0.2/generated/",
        )
        .expect("must parse");
        assert_eq!(model, "dp-record-metadata");
        assert_eq!(version, "0.0.2");
    }

    #[tokio::test]
    async fn catalog_registry_lists_models_from_explicit_urls() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"create"}"#);
        let not_found = ResponseTemplate::new(404);

        // Model A v0.0.3
        Mock::given(method("GET"))
            .and(path(
                "/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/route.json",
            ))
            .respond_with(route.clone())
            .mount(&server)
            .await;
        for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/{filename}"
                )))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        // Model B v1.1.0
        Mock::given(method("GET"))
            .and(path(
                "/CE-RISE-models/dp-record-metadata/src/tag/pages-v1.1.0/generated/route.json",
            ))
            .respond_with(route)
            .mount(&server)
            .await;
        for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/dp-record-metadata/src/tag/pages-v1.1.0/generated/{filename}"
                )))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        let catalog = format!(
            r#"{{
  "models": [
    {{
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "route_url": "{}/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/route.json"
    }},
    {{
      "model": "dp-record-metadata",
      "version": "1.1.0",
      "route_url": "{}/CE-RISE-models/dp-record-metadata/src/tag/pages-v1.1.0/generated/route.json"
    }}
  ]
}}"#,
            server.uri(),
            server.uri()
        );

        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog, vec![], false)
            .await
            .expect("catalog registry should initialize");

        let mut models = registry
            .list_models()
            .await
            .expect("list_models should succeed");
        models.sort_by(|a, b| {
            (a.id.0.as_str(), a.version.0.as_str()).cmp(&(b.id.0.as_str(), b.version.0.as_str()))
        });

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id.0, "dp-record-metadata");
        assert_eq!(models[0].version.0, "1.1.0");
        assert_eq!(models[1].id.0, "re-indicators-specification");
        assert_eq!(models[1].version.0, "0.0.3");
    }

    #[tokio::test]
    async fn duplicate_model_and_version_is_rejected() {
        let catalog = r#"{
  "models": [
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "route_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/route.json"
    },
    {
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "route_url": "https://codeberg.org/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/route.json"
    }
  ]
}"#;

        let err = CatalogArtifactRegistry::from_json_catalog(catalog, vec![], true)
            .await
            .expect_err("duplicate model/version must be rejected");

        let msg = err.to_string();
        assert!(
            msg.contains(
                "duplicate catalog entry for model=re-indicators-specification version=0.0.3"
            ),
            "unexpected error message: {msg}"
        );
    }

    #[tokio::test]
    async fn same_model_with_different_versions_is_accepted() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"create"}"#);
        let not_found = ResponseTemplate::new(404);

        for version in ["0.0.2", "0.0.3"] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/re-indicators-specification/src/tag/pages-v{version}/generated/route.json"
                )))
                .respond_with(route.clone())
                .mount(&server)
                .await;
            for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
                Mock::given(method("GET"))
                    .and(path(format!(
                        "/CE-RISE-models/re-indicators-specification/src/tag/pages-v{version}/generated/{filename}"
                    )))
                    .respond_with(not_found.clone())
                    .mount(&server)
                    .await;
            }
        }

        let catalog = format!(
            r#"{{
  "models": [
    {{
      "model": "re-indicators-specification",
      "version": "0.0.2",
      "route_url": "{}/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.2/generated/route.json"
    }},
    {{
      "model": "re-indicators-specification",
      "version": "0.0.3",
      "route_url": "{}/CE-RISE-models/re-indicators-specification/src/tag/pages-v0.0.3/generated/route.json"
    }}
  ]
}}"#,
            server.uri(),
            server.uri()
        );

        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog, vec![], false)
            .await
            .expect("same model with different versions should be accepted");

        let mut models = registry
            .list_models()
            .await
            .expect("list_models should succeed");
        models.sort_by(|a, b| a.version.0.cmp(&b.version.0));

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id.0, "re-indicators-specification");
        assert_eq!(models[0].version.0, "0.0.2");
        assert_eq!(models[1].id.0, "re-indicators-specification");
        assert_eq!(models[1].version.0, "0.0.3");
    }

    #[tokio::test]
    async fn explicit_schema_only_entry_is_indexed_without_route() {
        let server = MockServer::start().await;
        let schema = ResponseTemplate::new(200).set_body_string(r#"{"type":"object"}"#);

        Mock::given(method("GET"))
            .and(path(
                "/external-models/schema-only/src/tag/pages-v1.2.3/generated/schema.json",
            ))
            .respond_with(schema)
            .mount(&server)
            .await;

        let catalog = format!(
            r#"{{
  "models": [
    {{
      "model": "schema-only",
      "version": "1.2.3",
      "schema_url": "{}/external-models/schema-only/src/tag/pages-v1.2.3/generated/schema.json"
    }}
  ]
}}"#,
            server.uri()
        );

        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog, vec![], false)
            .await
            .expect("schema-only entry should initialize");

        let models = registry.list_models().await.expect("list should work");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id.0, "schema-only");
        assert_eq!(models[0].version.0, "1.2.3");

        let artifacts = registry
            .resolve(
                &hex_core::domain::model::ModelId("schema-only".into()),
                &hex_core::domain::model::ModelVersion("1.2.3".into()),
            )
            .await
            .expect("resolve should work");
        assert!(artifacts.route.is_none());
        assert_eq!(artifacts.schema.as_deref(), Some(r#"{"type":"object"}"#));
    }

    #[tokio::test]
    async fn refresh_reloads_inline_catalog_after_replacement() {
        let server = MockServer::start().await;
        let route = ResponseTemplate::new(200).set_body_string(r#"{"op":"create"}"#);
        let not_found = ResponseTemplate::new(404);

        // First catalog entry model-a@1.0.0
        Mock::given(method("GET"))
            .and(path(
                "/CE-RISE-models/model-a/src/tag/pages-v1.0.0/generated/route.json",
            ))
            .respond_with(route.clone())
            .mount(&server)
            .await;
        for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/model-a/src/tag/pages-v1.0.0/generated/{filename}"
                )))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        // Second catalog entry model-b@2.0.0
        Mock::given(method("GET"))
            .and(path(
                "/CE-RISE-models/model-b/src/tag/pages-v2.0.0/generated/route.json",
            ))
            .respond_with(route)
            .mount(&server)
            .await;
        for filename in ["schema.json", "shacl.ttl", "owl.ttl", "openapi.json"] {
            Mock::given(method("GET"))
                .and(path(format!(
                    "/CE-RISE-models/model-b/src/tag/pages-v2.0.0/generated/{filename}"
                )))
                .respond_with(not_found.clone())
                .mount(&server)
                .await;
        }

        let catalog_a = format!(
            r#"[{{"model":"model-a","version":"1.0.0","route_url":"{}/CE-RISE-models/model-a/src/tag/pages-v1.0.0/generated/route.json"}}]"#,
            server.uri()
        );
        let registry = CatalogArtifactRegistry::from_json_catalog(&catalog_a, vec![], false)
            .await
            .expect("registry should initialize");

        let models = registry.list_models().await.expect("list should work");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id.0, "model-a");
        assert_eq!(models[0].version.0, "1.0.0");

        let catalog_b = format!(
            r#"[{{"model":"model-b","version":"2.0.0","route_url":"{}/CE-RISE-models/model-b/src/tag/pages-v2.0.0/generated/route.json"}}]"#,
            server.uri()
        );
        registry
            .replace_catalog_from_json(&catalog_b)
            .await
            .expect("replace should work");
        registry.refresh().await.expect("refresh should work");

        let models = registry.list_models().await.expect("list should work");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id.0, "model-b");
        assert_eq!(models[0].version.0, "2.0.0");
    }
}
