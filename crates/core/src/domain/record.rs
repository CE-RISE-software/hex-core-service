use crate::domain::model::{ModelId, ModelVersion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: RecordId,
    pub model: ModelId,
    pub version: ModelVersion,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::{Record, RecordId};
    use crate::domain::model::{ModelId, ModelVersion};

    #[test]
    fn record_id_serde_round_trip() {
        let id = RecordId("rec-123".into());
        let json = serde_json::to_string(&id).expect("serialize RecordId");
        let decoded: RecordId = serde_json::from_str(&json).expect("deserialize RecordId");
        assert_eq!(decoded.0, "rec-123");
    }

    #[test]
    fn record_serde_round_trip() {
        let record = Record {
            id: RecordId("rec-1".into()),
            model: ModelId("product-passport".into()),
            version: ModelVersion("1.0.0".into()),
            payload: serde_json::json!({
                "record_scope": "product",
                "sequence_order": 1
            }),
        };

        let json = serde_json::to_string(&record).expect("serialize Record");
        let decoded: Record = serde_json::from_str(&json).expect("deserialize Record");

        assert_eq!(decoded.id.0, "rec-1");
        assert_eq!(decoded.model.0, "product-passport");
        assert_eq!(decoded.version.0, "1.0.0");
        assert_eq!(decoded.payload["sequence_order"], 1);
    }
}
