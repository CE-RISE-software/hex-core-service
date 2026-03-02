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
