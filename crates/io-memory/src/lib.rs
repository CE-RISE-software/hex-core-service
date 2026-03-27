use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;

use hex_core::domain::{
    auth::SecurityContext,
    error::StoreError,
    record::{Record, RecordId},
};
use hex_core::ports::outbound::record_store::RecordStorePort;

#[derive(Debug)]
struct Entry {
    record: Record,
}

/// In-memory implementation of `RecordStorePort`.
/// Intended for tests and local development only — data is lost on restart.
#[derive(Debug, Default, Clone)]
pub struct MemoryRecordStore {
    store: Arc<RwLock<HashMap<String, Entry>>>,
    /// Maps idempotency key → record id to enforce deduplication.
    idempotency_index: Arc<RwLock<HashMap<String, String>>>,
}

impl MemoryRecordStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RecordStorePort for MemoryRecordStore {
    async fn write(
        &self,
        _ctx: &SecurityContext,
        idempotency_key: &str,
        record: Record,
    ) -> Result<RecordId, StoreError> {
        let mut idx = self.idempotency_index.write().await;

        if let Some(existing_id) = idx.get(idempotency_key) {
            let store = self.store.read().await;
            if let Some(entry) = store.get(existing_id) {
                // Same key, same payload — idempotent success.
                if entry.record.payload == record.payload {
                    return Ok(entry.record.id.clone());
                }
                // Same key, different payload — conflict.
                return Err(StoreError::IdempotencyConflict {
                    key: idempotency_key.to_string(),
                });
            }
        }

        let id = record.id.clone();
        idx.insert(idempotency_key.to_string(), id.0.clone());

        let mut store = self.store.write().await;
        store.insert(id.0.clone(), Entry { record });

        Ok(id)
    }

    async fn read(&self, _ctx: &SecurityContext, id: &RecordId) -> Result<Record, StoreError> {
        let store = self.store.read().await;
        store
            .get(&id.0)
            .map(|e| e.record.clone())
            .ok_or_else(|| StoreError::NotFound { id: id.0.clone() })
    }

    async fn query(
        &self,
        _ctx: &SecurityContext,
        filter: serde_json::Value,
    ) -> Result<Vec<Record>, StoreError> {
        let store = self.store.read().await;
        let mut records: Vec<Record> = store.values().map(|e| e.record.clone()).collect();
        apply_filter(&mut records, &filter)?;
        Ok(records)
    }
}

fn apply_filter(records: &mut Vec<Record>, filter: &Value) -> Result<(), StoreError> {
    let Some(obj) = filter.as_object() else {
        if filter.is_null() {
            return Ok(());
        }
        return Err(StoreError::Internal(
            "query filter must be a JSON object".into(),
        ));
    };

    if let Some(where_clause) = obj.get("where") {
        let conditions = where_clause
            .as_array()
            .ok_or_else(|| StoreError::Internal("query filter 'where' must be an array".into()))?;
        records.retain(|record| {
            conditions
                .iter()
                .all(|cond| condition_matches(record, cond).unwrap_or(false))
        });
    }

    if let Some(sort_clause) = obj.get("sort") {
        let sort_items = sort_clause
            .as_array()
            .ok_or_else(|| StoreError::Internal("query filter 'sort' must be an array".into()))?;
        if let Some(first) = sort_items.first() {
            let field = first
                .get("field")
                .and_then(Value::as_str)
                .ok_or_else(|| StoreError::Internal("sort item missing string 'field'".into()))?;
            let direction = first
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("asc");
            let desc = matches!(direction, "desc" | "DESC");
            records.sort_by(|a, b| {
                compare_optional_json(field_value(a, field), field_value(b, field))
            });
            if desc {
                records.reverse();
            }
        }
    }

    let offset = obj.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let limit = obj.get("limit").and_then(Value::as_u64).map(|n| n as usize);
    let end = limit
        .map(|n| offset.saturating_add(n))
        .unwrap_or(records.len());
    let sliced = records
        .iter()
        .skip(offset)
        .take(end.saturating_sub(offset))
        .cloned()
        .collect();
    *records = sliced;

    Ok(())
}

fn condition_matches(record: &Record, condition: &Value) -> Result<bool, StoreError> {
    let obj = condition
        .as_object()
        .ok_or_else(|| StoreError::Internal("query condition must be an object".into()))?;
    let field = obj
        .get("field")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Internal("query condition missing string 'field'".into()))?;
    let op = obj
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::Internal("query condition missing string 'op'".into()))?;
    let expected = obj.get("value").cloned().unwrap_or(Value::Null);
    let actual = field_value(record, field);

    match op {
        "eq" => Ok(actual == Some(&expected)),
        "ne" => Ok(actual != Some(&expected)),
        "gt" => Ok(compare_json(actual, Some(&expected)).is_gt()),
        "gte" => Ok(compare_json(actual, Some(&expected)).is_ge()),
        "lt" => Ok(compare_json(actual, Some(&expected)).is_lt()),
        "lte" => Ok(compare_json(actual, Some(&expected)).is_le()),
        "in" => {
            let items = expected.as_array().ok_or_else(|| {
                StoreError::Internal("query condition 'in' expects array value".into())
            })?;
            Ok(actual.is_some_and(|a| items.iter().any(|item| item == a)))
        }
        "contains" => Ok(match (actual, &expected) {
            (Some(Value::String(actual)), Value::String(needle)) => actual.contains(needle),
            (Some(Value::Array(values)), needle) => values.iter().any(|v| v == needle),
            _ => false,
        }),
        "exists" => {
            let expected_bool = expected.as_bool().ok_or_else(|| {
                StoreError::Internal("query condition 'exists' expects boolean value".into())
            })?;
            Ok(actual.is_some() == expected_bool)
        }
        other => Err(StoreError::Internal(format!(
            "unsupported query operator '{other}'"
        ))),
    }
}

fn field_value<'a>(record: &'a Record, field: &str) -> Option<&'a Value> {
    match field {
        "id" => return None,
        "model" => return None,
        "version" => return None,
        _ => {}
    }

    let payload_path = field.strip_prefix("payload.")?;
    let root = &record.payload;
    get_json_path(root, payload_path)
}

fn get_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        let mut rest = segment;
        if let Some(bracket) = rest.find('[') {
            let key = &rest[..bracket];
            current = current.get(key)?;
            rest = &rest[bracket..];
        } else {
            current = current.get(rest)?;
            continue;
        }

        while let Some(stripped) = rest.strip_prefix('[') {
            let close = stripped.find(']')?;
            let index: usize = stripped[..close].parse().ok()?;
            current = current.get(index)?;
            rest = &stripped[close + 1..];
        }
    }
    Some(current)
}

fn compare_optional_json(left: Option<&Value>, right: Option<&Value>) -> std::cmp::Ordering {
    compare_json(left, right)
}

fn compare_json(left: Option<&Value>, right: Option<&Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
        (Some(Value::Number(a)), Some(Value::Number(b))) => a
            .as_f64()
            .partial_cmp(&b.as_f64())
            .unwrap_or(Ordering::Equal),
        (Some(Value::Bool(a)), Some(Value::Bool(b))) => a.cmp(b),
        (Some(a), Some(b)) => a.to_string().cmp(&b.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_core::domain::{
        auth::SecurityContext,
        model::{ModelId, ModelVersion},
        record::{Record, RecordId},
    };

    fn ctx() -> SecurityContext {
        SecurityContext {
            subject: "test-user".into(),
            roles: vec![],
            scopes: vec![],
            tenant: None,
            raw_token: None,
        }
    }

    fn record(id: &str) -> Record {
        Record {
            id: RecordId(id.into()),
            model: ModelId("test-model".into()),
            version: ModelVersion("1.0.0".into()),
            payload: serde_json::json!({ "key": "value" }),
        }
    }

    #[tokio::test]
    async fn write_and_read_round_trip() {
        let store = MemoryRecordStore::new();
        let rec = record("rec-1");
        let ctx = ctx();
        let id = store.write(&ctx, "idem-1", rec.clone()).await.unwrap();
        let fetched = store.read(&ctx, &id).await.unwrap();
        assert_eq!(fetched.id, rec.id);
    }

    #[tokio::test]
    async fn idempotent_write_returns_same_id() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let rec = record("rec-2");
        let id1 = store.write(&ctx, "idem-2", rec.clone()).await.unwrap();
        let id2 = store.write(&ctx, "idem-2", rec.clone()).await.unwrap();
        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn conflict_on_different_payload_same_key() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let rec1 = record("rec-3");
        let mut rec2 = record("rec-3");
        rec2.payload = serde_json::json!({ "key": "other" });
        store.write(&ctx, "idem-3", rec1).await.unwrap();
        let err = store.write(&ctx, "idem-3", rec2).await.unwrap_err();
        assert!(matches!(err, StoreError::IdempotencyConflict { .. }));
    }

    #[tokio::test]
    async fn read_not_found() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();
        let err = store
            .read(&ctx, &RecordId("missing".into()))
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::NotFound { .. }));
    }

    #[tokio::test]
    async fn query_applies_where_limit_and_offset() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();

        let mut a = record("rec-1");
        a.payload = serde_json::json!({"record_scope": "product", "score": 10});
        let mut b = record("rec-2");
        b.payload = serde_json::json!({"record_scope": "material", "score": 20});
        let mut c = record("rec-3");
        c.payload = serde_json::json!({"record_scope": "product", "score": 30});

        store.write(&ctx, "idem-a", a).await.unwrap();
        store.write(&ctx, "idem-b", b).await.unwrap();
        store.write(&ctx, "idem-c", c).await.unwrap();

        let records = store
            .query(
                &ctx,
                serde_json::json!({
                    "where": [
                        { "field": "payload.record_scope", "op": "eq", "value": "product" }
                    ],
                    "sort": [
                        { "field": "payload.score", "direction": "asc" }
                    ],
                    "limit": 1,
                    "offset": 1
                }),
            )
            .await
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id.0, "rec-3");
    }

    #[tokio::test]
    async fn query_rejects_invalid_filter_shape() {
        let store = MemoryRecordStore::new();
        let ctx = ctx();

        let err = store
            .query(&ctx, serde_json::json!({"where": {"field": "payload.a"}}))
            .await
            .unwrap_err();

        assert!(matches!(err, StoreError::Internal(_)));
    }
}
