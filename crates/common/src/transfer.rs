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

/// Token Standard V2 wire types for the transfer path.
///
/// V1 stays at module level. `InstrumentId`, `Meta` and `DisclosedContract`
/// are version-neutral and serve both.
pub mod v2 {
    use serde::{Deserialize, Serialize};

    /// An on-chain managed account, per `Splice.Api.Token.HoldingV2.Account`.
    ///
    /// `owner` is `None` only for the special accounts an instrument admin
    /// manages, such as the source account for a mint. `id` defaults to the
    /// empty string.
    ///
    /// `owner` and `provider` serialize explicitly, `null` included: the Daml
    /// JSON encoding of `Optional` expects the field to be present.
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    pub struct Account {
        pub owner: Option<String>,
        pub provider: Option<String>,
        pub id: String,
    }

    impl Account {
        /// The basic account for an owner: no provider, empty id.
        pub fn basic(owner: impl Into<String>) -> Self {
            Self {
                owner: Some(owner.into()),
                provider: None,
                id: String::new(),
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Transfer {
        pub sender: Account,
        pub receiver: Account,
        pub amount: crate::decimal::DamlDecimal,
        #[serde(rename = "instrumentId")]
        pub instrument_id: super::InstrumentId,
        #[serde(rename = "requestedAt")]
        pub requested_at: String,
        #[serde(rename = "executeBefore")]
        pub execute_before: String,
        #[serde(rename = "inputHoldingCids")]
        pub input_holding_cids: Option<Vec<String>>,
        pub meta: Option<super::Meta>,
    }
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
        assert_eq!(
            json["amount"],
            serde_json::Value::String("0.02".to_string())
        );

        let json_str = serde_json::to_string(&transfer).unwrap();
        let deserialized: Transfer = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.amount, DamlDecimal::parse("0.02").unwrap());
    }

    #[test]
    fn v2_account_serializes_owner_and_provider_explicitly() {
        let account = v2::Account {
            owner: Some("alice::1220ab".to_string()),
            provider: None,
            id: String::new(),
        };

        let json = serde_json::to_value(&account).unwrap();
        assert_eq!(json["owner"], serde_json::json!("alice::1220ab"));
        assert_eq!(
            json["provider"],
            serde_json::Value::Null,
            "provider must serialize as an explicit null, not be omitted"
        );
        assert_eq!(json["id"], serde_json::json!(""));

        let special = v2::Account {
            owner: None,
            provider: None,
            id: String::new(),
        };
        let json = serde_json::to_value(&special).unwrap();
        assert_eq!(
            json["owner"],
            serde_json::Value::Null,
            "owner must serialize as an explicit null, not be omitted"
        );
    }

    #[test]
    fn v2_transfer_amount_serializes_as_string() {
        let transfer = v2::Transfer {
            sender: v2::Account {
                owner: Some("sender1".to_string()),
                provider: None,
                id: String::new(),
            },
            receiver: v2::Account {
                owner: Some("receiver1".to_string()),
                provider: None,
                id: String::new(),
            },
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
        assert_eq!(json["amount"], serde_json::json!("0.02"));
        assert_eq!(json["instrumentId"]["id"], serde_json::json!("CBTC"));
        assert_eq!(
            json["requestedAt"],
            serde_json::json!("2024-01-01T00:00:00Z")
        );
        assert_eq!(
            json["executeBefore"],
            serde_json::json!("2024-12-31T23:59:59Z")
        );

        let json_str = serde_json::to_string(&transfer).unwrap();
        let deserialized: v2::Transfer = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.amount, DamlDecimal::parse("0.02").unwrap());
    }
}
