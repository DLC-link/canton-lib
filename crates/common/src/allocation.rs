use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::decimal::DamlDecimal;
use crate::transfer::InstrumentId;

/// DAML `Splice.Api.Token.MetadataV1.Metadata` — a string-keyed map of
/// app-specific annotations. Encoded as `{ "values": { .. } }`; an absent or
/// empty map serializes as `{ "values": {} }`, which is the DAML JSON encoding
/// of an empty `TextMap`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Metadata {
    pub values: HashMap<String, String>,
}

/// DAML `Splice.Api.Token.AllocationV1.Reference` — an app-specific identifier
/// for the settlement that an allocation belongs to.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Reference {
    /// The key identifying the data. May be the empty string when `cid` alone
    /// is sufficient.
    pub id: String,
    /// Optional contract id used to refer to a contract. DAML `AnyContractId`
    /// is `ContractId AnyContract`, so it is encoded as a plain contract-id
    /// string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
}

/// DAML `Splice.Api.Token.AllocationV1.SettlementInfo` — the timing and
/// authority shared by every leg of a single settlement.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettlementInfo {
    /// The party responsible for executing the settlement (the venue).
    pub executor: String,
    #[serde(rename = "settlementRef")]
    pub settlement_ref: Reference,
    #[serde(rename = "requestedAt")]
    pub requested_at: String,
    /// Exclusive deadline by which senders must allocate.
    #[serde(rename = "allocateBefore")]
    pub allocate_before: String,
    /// Exclusive deadline by which the executor must settle.
    #[serde(rename = "settleBefore")]
    pub settle_before: String,
    pub meta: Metadata,
}

/// DAML `Splice.Api.Token.AllocationV1.TransferLeg` — a single directed
/// transfer of one instrument from a sender to a receiver.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferLeg {
    pub sender: String,
    pub receiver: String,
    pub amount: DamlDecimal,
    #[serde(rename = "instrumentId")]
    pub instrument_id: InstrumentId,
    pub meta: Metadata,
}

/// DAML `Splice.Api.Token.AllocationV1.AllocationSpecification` — what should
/// be allocated: the shared settlement plus this leg's id and details.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AllocationSpecification {
    pub settlement: SettlementInfo,
    #[serde(rename = "transferLegId")]
    pub transfer_leg_id: String,
    #[serde(rename = "transferLeg")]
    pub transfer_leg: TransferLeg,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> AllocationSpecification {
        AllocationSpecification {
            settlement: SettlementInfo {
                executor: "venue1".to_string(),
                settlement_ref: Reference {
                    id: "OTCTradeProposal".to_string(),
                    cid: None,
                },
                requested_at: "2024-01-01T00:00:00Z".to_string(),
                allocate_before: "2024-01-02T00:00:00Z".to_string(),
                settle_before: "2024-01-03T00:00:00Z".to_string(),
                meta: Metadata::default(),
            },
            transfer_leg_id: "leg0".to_string(),
            transfer_leg: TransferLeg {
                sender: "sender1".to_string(),
                receiver: "receiver1".to_string(),
                amount: DamlDecimal::parse("0.02").unwrap(),
                instrument_id: InstrumentId {
                    admin: "admin1".to_string(),
                    id: "CBTC".to_string(),
                },
                meta: Metadata::default(),
            },
        }
    }

    #[test]
    fn amount_serializes_as_string_and_fields_are_camel_case() {
        let json = serde_json::to_value(sample_spec()).unwrap();
        assert_eq!(
            json["transferLeg"]["amount"],
            serde_json::Value::String("0.02".to_string())
        );
        assert_eq!(json["transferLegId"], "leg0");
        assert_eq!(
            json["settlement"]["settlementRef"]["id"],
            "OTCTradeProposal"
        );
        assert_eq!(json["settlement"]["allocateBefore"], "2024-01-02T00:00:00Z");
    }

    #[test]
    fn empty_metadata_serializes_as_empty_values_map() {
        let json = serde_json::to_value(Metadata::default()).unwrap();
        assert_eq!(json, serde_json::json!({ "values": {} }));
    }

    #[test]
    fn absent_reference_cid_is_omitted() {
        let json = serde_json::to_value(Reference {
            id: "ref".to_string(),
            cid: None,
        })
        .unwrap();
        assert!(json.get("cid").is_none());
    }

    #[test]
    fn allocation_specification_round_trips() {
        let spec = sample_spec();
        let serialized = serde_json::to_string(&spec).unwrap();
        let deserialized: AllocationSpecification = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.transfer_leg.amount,
            DamlDecimal::parse("0.02").unwrap()
        );
        assert_eq!(deserialized.transfer_leg_id, "leg0");
        assert_eq!(deserialized.settlement.executor, "venue1");
    }
}
