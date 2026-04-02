use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transfer {
    pub sender: String,
    pub receiver: String,
    pub amount: crate::decimal::DamlDecimal,
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
    #[serde(rename = "templateId", skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(rename = "contractId")]
    pub contract_id: String,
    #[serde(rename = "createdEventBlob")]
    pub created_event_blob: String,
    #[serde(rename = "synchronizerId")]
    pub synchronizer_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decimal::DamlDecimal;

    #[test]
    fn transfer_amount_serializes_as_string() {
        let transfer = Transfer {
            sender: "sender1".to_string(),
            receiver: "receiver1".to_string(),
            amount: DamlDecimal::parse("0.02").unwrap(),
            instrument_id: InstrumentId {
                admin: "admin1".to_string(),
                id: "CBTC".to_string(),
            },
            requested_at: "2024-01-01T00:00:00Z".to_string(),
            execute_before: "2024-12-31T23:59:59Z".to_string(),
            input_holding_cids: None,
            meta: None,
        };

        let json = serde_json::to_value(&transfer).unwrap();
        assert_eq!(json["amount"], serde_json::Value::String("0.02".to_string()));

        let json_str = serde_json::to_string(&transfer).unwrap();
        let deserialized: Transfer = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.amount, DamlDecimal::parse("0.02").unwrap());
    }
}
