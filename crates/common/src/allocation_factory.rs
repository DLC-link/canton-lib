use serde::{Deserialize, Serialize};

use crate::allocation::AllocationSpecification;
use crate::transfer::DisclosedContract;
use crate::transfer_factory::{Context, ExtraArgs};

/// Arguments for the token-standard `AllocationFactory_Allocate` choice, sent
/// as the `choiceArguments` of a `getAllocationFactory` request so the registry
/// can return instrument-specific reference data.
///
/// The `extra_args` context and meta are expected to be empty on the request;
/// the populated choice context is returned by the registry in [`Response`].
#[derive(Serialize, Deserialize, Debug)]
pub struct ChoiceArguments {
    #[serde(rename = "expectedAdmin")]
    pub expected_admin: String,
    pub allocation: AllocationSpecification,
    #[serde(rename = "requestedAt")]
    pub requested_at: String,
    #[serde(rename = "inputHoldingCids")]
    pub input_holding_cids: Vec<String>,
    #[serde(rename = "extraArgs")]
    pub extra_args: ExtraArgs,
}

/// Response of the `getAllocationFactory` registry endpoint: the factory
/// contract id together with the choice context needed to exercise
/// `AllocationFactory_Allocate`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    #[serde(rename = "factoryId")]
    pub factory_id: String,
    #[serde(rename = "choiceContext")]
    pub choice_context: ChoiceContext,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChoiceContext {
    #[serde(rename = "choiceContextData")]
    pub choice_context_data: Context,
    #[serde(rename = "disclosedContracts")]
    pub disclosed_contracts: Vec<DisclosedContract>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::{Metadata, Reference, SettlementInfo, TransferLeg};
    use crate::decimal::DamlDecimal;
    use crate::transfer::InstrumentId;
    use crate::transfer_factory::{Context, Meta, MetaValue};
    use std::collections::HashMap;

    fn empty_extra_args() -> ExtraArgs {
        ExtraArgs {
            context: Context {
                values: HashMap::new(),
            },
            meta: Meta {
                values: MetaValue {},
            },
        }
    }

    #[test]
    fn choice_arguments_serialize_with_camel_case_keys() {
        let args = ChoiceArguments {
            expected_admin: "admin1".to_string(),
            allocation: AllocationSpecification {
                settlement: SettlementInfo {
                    executor: "venue1".to_string(),
                    settlement_ref: Reference {
                        id: "ref".to_string(),
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
                    amount: DamlDecimal::parse("100.0").unwrap(),
                    instrument_id: InstrumentId {
                        admin: "admin1".to_string(),
                        id: "CBTC".to_string(),
                    },
                    meta: Metadata::default(),
                },
            },
            requested_at: "2024-01-01T00:00:00Z".to_string(),
            input_holding_cids: vec!["cid1".to_string(), "cid2".to_string()],
            extra_args: empty_extra_args(),
        };

        let json = serde_json::to_value(&args).unwrap();
        assert_eq!(json["expectedAdmin"], "admin1");
        assert_eq!(json["inputHoldingCids"][0], "cid1");
        assert_eq!(
            json["extraArgs"]["context"]["values"],
            serde_json::json!({})
        );
        assert_eq!(
            json["allocation"]["transferLeg"]["amount"],
            serde_json::Value::String("100.0".to_string())
        );
    }

    #[test]
    fn response_deserializes_factory_and_choice_context() {
        let json = r#"{
            "factoryId": "00factory",
            "choiceContext": {
                "choiceContextData": {
                    "values": {
                        "instrument-configuration": {"tag": "AV_ContractId", "value": "00cfg"}
                    }
                },
                "disclosedContracts": [
                    {
                        "templateId": "pkg:Mod:T",
                        "contractId": "00disclosed",
                        "createdEventBlob": "blob",
                        "synchronizerId": "sync::1220"
                    }
                ]
            }
        }"#;
        let response: Response = serde_json::from_str(json).unwrap();
        assert_eq!(response.factory_id, "00factory");
        assert_eq!(response.choice_context.disclosed_contracts.len(), 1);
        assert_eq!(
            response.choice_context.disclosed_contracts[0].contract_id,
            "00disclosed"
        );
        assert!(
            response
                .choice_context
                .choice_context_data
                .values
                .contains_key("instrument-configuration")
        );
    }
}
