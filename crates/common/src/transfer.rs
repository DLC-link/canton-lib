use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transfer {
    pub sender: String,
    pub receiver: String,
    pub amount: String,
    #[serde(rename = "instrumentId")]
    pub instrument_id: InstrumentId,
    #[serde(rename = "requestedAt")]
    pub requested_at: String,
    #[serde(rename = "executeBefore")]
    pub execute_before: String,
    #[serde(rename = "inputHoldingCids")]
    pub input_holding_cids: Option<Vec<String>>,
    pub meta: Option<Meta>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Meta {
    pub values: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstrumentId {
    pub admin: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DisclosedContract {
    #[serde(rename = "templateId")]
    pub template_id: String,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    #[serde(rename = "createdEventBlob")]
    pub created_event_blob: String,
    #[serde(rename = "synchronizerId")]
    pub synchronizer_id: String,
}
