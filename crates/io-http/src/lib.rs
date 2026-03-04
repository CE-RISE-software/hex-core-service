use async_trait::async_trait;
use hex_core::domain::{
    auth::SecurityContext,
    error::StoreError,
    record::{Record, RecordId},
};
use hex_core::ports::outbound::record_store::RecordStorePort;
use serde::Deserialize;
use std::time::Duration;

const IO_ADAPTER_PATH_WRITE: &str = "/records";
const IO_ADAPTER_PATH_QUERY: &str = "/records/query";

pub struct HttpRecordStore {
    base_url: String,
    client: reqwest::Client,
}

impl HttpRecordStore {
    pub fn new(base_url: impl Into<String>, timeout_ms: u64) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .expect("failed to build HTTP client");
        Self { base_url, client }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn with_auth(
        &self,
        req: reqwest::RequestBuilder,
        ctx: &SecurityContext,
    ) -> reqwest::RequestBuilder {
        if let Some(token) = ctx.raw_token.as_deref() {
            req.bearer_auth(token)
        } else {
            req
        }
    }

    async fn response_text(resp: reqwest::Response) -> String {
        resp.text().await.unwrap_or_else(|_| "".into())
    }

    fn map_transport_error(err: reqwest::Error) -> StoreError {
        StoreError::Unavailable(format!("io-http transport error: {err}"))
    }

    fn map_status(status: reqwest::StatusCode, body: &str, op: &str) -> StoreError {
        match status {
            reqwest::StatusCode::NOT_FOUND => StoreError::NotFound {
                id: format!("{op}: not found"),
            },
            reqwest::StatusCode::CONFLICT => StoreError::IdempotencyConflict {
                key: format!("{op}: conflict"),
            },
            s if s.is_server_error() || s == reqwest::StatusCode::TOO_MANY_REQUESTS => {
                StoreError::Unavailable(format!("{op} failed with status {s}: {body}"))
            }
            s => StoreError::Internal(format!("{op} failed with status {s}: {body}")),
        }
    }
}

#[async_trait]
impl RecordStorePort for HttpRecordStore {
    async fn write(
        &self,
        ctx: &SecurityContext,
        idempotency_key: &str,
        record: Record,
    ) -> Result<RecordId, StoreError> {
        let url = self.endpoint(IO_ADAPTER_PATH_WRITE);
        let req = self
            .client
            .post(url)
            .header("Idempotency-Key", idempotency_key)
            .json(&record);
        let req = self.with_auth(req, ctx);
        let resp = req.send().await.map_err(Self::map_transport_error)?;
        let status = resp.status();

        if !status.is_success() {
            let body = Self::response_text(resp).await;
            return Err(Self::map_status(status, &body, "write"));
        }

        let payload: WriteResponse = resp
            .json()
            .await
            .map_err(|e| StoreError::Internal(format!("write response decode failed: {e}")))?;
        Ok(RecordId(payload.id))
    }

    async fn read(&self, ctx: &SecurityContext, id: &RecordId) -> Result<Record, StoreError> {
        let url = self.endpoint(&format!("{}/{}", IO_ADAPTER_PATH_WRITE, id.0));
        let req = self.with_auth(self.client.get(url), ctx);
        let resp = req.send().await.map_err(Self::map_transport_error)?;
        let status = resp.status();
        if !status.is_success() {
            let body = Self::response_text(resp).await;
            return Err(Self::map_status(status, &body, "read"));
        }

        resp.json()
            .await
            .map_err(|e| StoreError::Internal(format!("read response decode failed: {e}")))
    }

    async fn query(
        &self,
        ctx: &SecurityContext,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError> {
        let url = self.endpoint(IO_ADAPTER_PATH_QUERY);
        let req = self.with_auth(
            self.client
                .post(url)
                .json(&serde_json::json!({ "filter": filter })),
            ctx,
        );
        let resp = req.send().await.map_err(Self::map_transport_error)?;
        let status = resp.status();
        if !status.is_success() {
            let body = Self::response_text(resp).await;
            return Err(Self::map_status(status, &body, "query"));
        }

        let payload: QueryResponse = resp
            .json()
            .await
            .map_err(|e| StoreError::Internal(format!("query response decode failed: {e}")))?;
        Ok(payload.records)
    }
}

#[derive(Debug, Deserialize)]
struct WriteResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    records: Vec<Record>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_core::domain::{
        auth::SecurityContext,
        model::{ModelId, ModelVersion},
        record::{Record, RecordId},
    };
    use wiremock::{
        matchers::{body_json, header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn ctx_with_token() -> SecurityContext {
        SecurityContext {
            subject: "user-1".into(),
            roles: vec!["admin".into()],
            scopes: vec!["records:write".into()],
            tenant: None,
            raw_token: Some("token-123".into()),
        }
    }

    fn sample_record() -> Record {
        Record {
            id: RecordId("rec-1".into()),
            model: ModelId("model-a".into()),
            version: ModelVersion("1.0.0".into()),
            payload: serde_json::json!({"x": 1}),
        }
    }

    #[tokio::test]
    async fn write_success_forwards_token_and_idempotency_key() {
        let server = MockServer::start().await;
        let store = HttpRecordStore::new(server.uri(), 5_000);
        let record = sample_record();

        Mock::given(method("POST"))
            .and(path(IO_ADAPTER_PATH_WRITE))
            .and(header("authorization", "Bearer token-123"))
            .and(header("idempotency-key", "idem-1"))
            .and(body_json(
                serde_json::to_value(&record).expect("record json"),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "rec-1"
            })))
            .mount(&server)
            .await;

        let id = store
            .write(&ctx_with_token(), "idem-1", record)
            .await
            .expect("write succeeds");
        assert_eq!(id.0, "rec-1");
    }

    #[tokio::test]
    async fn write_conflict_maps_to_idempotency_conflict() {
        let server = MockServer::start().await;
        let store = HttpRecordStore::new(server.uri(), 5_000);

        Mock::given(method("POST"))
            .and(path(IO_ADAPTER_PATH_WRITE))
            .respond_with(ResponseTemplate::new(409))
            .mount(&server)
            .await;

        let err = store
            .write(&ctx_with_token(), "idem-1", sample_record())
            .await
            .expect_err("write should conflict");
        assert!(matches!(err, StoreError::IdempotencyConflict { .. }));
    }

    #[tokio::test]
    async fn read_success_and_not_found() {
        let server = MockServer::start().await;
        let store = HttpRecordStore::new(server.uri(), 5_000);
        let record = sample_record();

        Mock::given(method("GET"))
            .and(path("/records/rec-1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::to_value(&record).expect("record json")),
            )
            .mount(&server)
            .await;

        let got = store
            .read(&ctx_with_token(), &RecordId("rec-1".into()))
            .await
            .expect("read succeeds");
        assert_eq!(got.id.0, "rec-1");

        Mock::given(method("GET"))
            .and(path("/records/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let err = store
            .read(&ctx_with_token(), &RecordId("missing".into()))
            .await
            .expect_err("missing read should fail");
        assert!(matches!(err, StoreError::NotFound { .. }));
    }

    #[tokio::test]
    async fn query_success_parses_records() {
        let server = MockServer::start().await;
        let store = HttpRecordStore::new(server.uri(), 5_000);
        let record = sample_record();

        Mock::given(method("POST"))
            .and(path(IO_ADAPTER_PATH_QUERY))
            .and(header("authorization", "Bearer token-123"))
            .and(body_json(serde_json::json!({
                "filter": {"model": "model-a"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "records": [serde_json::to_value(&record).expect("record json")]
            })))
            .mount(&server)
            .await;

        let records = store
            .query(&ctx_with_token(), serde_json::json!({"model": "model-a"}))
            .await
            .expect("query succeeds");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id.0, "rec-1");
    }

    #[test]
    fn openapi_contract_matches_io_http_paths_and_methods() {
        const IO_ADAPTER_PATH_READ_TEMPLATE: &str = "/records/{id}";
        let spec: serde_json::Value = serde_json::from_str(include_str!("io_adapter_openapi.json"))
            .expect("openapi json must be valid");
        let paths = spec["paths"].as_object().expect("paths object");

        let write = paths
            .get(IO_ADAPTER_PATH_WRITE)
            .expect("openapi must define /records");
        assert!(
            write.get("post").is_some(),
            "openapi /records must expose POST"
        );

        let read = paths
            .get(IO_ADAPTER_PATH_READ_TEMPLATE)
            .expect("openapi must define /records/{id}");
        assert!(
            read.get("get").is_some(),
            "openapi /records/{{id}} must expose GET"
        );

        let query = paths
            .get(IO_ADAPTER_PATH_QUERY)
            .expect("openapi must define /records/query");
        assert!(
            query.get("post").is_some(),
            "openapi /records/query must expose POST"
        );
    }
}
