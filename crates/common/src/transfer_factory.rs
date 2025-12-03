use crate::transfer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct ChoiceArguments {
    #[serde(rename = "expectedAdmin")]
    pub expected_admin: String,
    pub transfer: transfer::Transfer,
    #[serde(rename = "extraArgs")]
    pub extra_args: ExtraArgs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtraArgs {
    pub context: Context,
    pub meta: Meta,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Context {
    pub values: HashMap<String, ContextValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ContextValue {
    Array(ContextValueArray),
    String(ContextValueString),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextValueArray {
    pub tag: String,
    pub value: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextValueString {
    pub tag: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Meta {
    pub values: MetaValue,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetaValue {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    #[serde(rename = "factoryId")]
    pub factory_id: String,
    #[serde(rename = "transferKind")]
    pub transfer_kind: String,
    #[serde(rename = "choiceContext")]
    pub choice_context: ChoiceContext,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChoiceContext {
    #[serde(rename = "choiceContextData")]
    pub choice_context_data: Context,
    #[serde(rename = "disclosedContracts")]
    pub disclosed_contracts: Vec<transfer::DisclosedContract>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choice_arguments_serialization() {
        let mut ctx_values: HashMap<String, ContextValue> = HashMap::new();

        let contract_id = "cid1".to_string();

        ctx_values.insert(
            "utility.digitalasset.com/instrument-configuration".to_string(),
            ContextValue::String(ContextValueString {
                tag: "AV_ContractId".to_string(),
                value: contract_id.clone(),
            }),
        );
        ctx_values.insert(
            "utility.digitalasset.com/sender-credentials".to_string(),
            ContextValue::Array(ContextValueArray {
                tag: "AV_List".to_string(),
                value: vec![],
            }),
        );
        ctx_values.insert(
            "instrument-configuration".to_string(),
            ContextValue::String(ContextValueString {
                tag: "AV_ContractId".to_string(),
                value: contract_id.clone(),
            }),
        );
        ctx_values.insert(
            "sender-credentials".to_string(),
            ContextValue::Array(ContextValueArray {
                tag: "AV_List".to_string(),
                value: vec![],
            }),
        );

        let choice_args = ChoiceArguments {
            expected_admin: "admin1".to_string(),
            transfer: transfer::Transfer {
                sender: "sender1".to_string(),
                receiver: "receiver1".to_string(),
                amount: "100.0".to_string(),
                instrument_id: transfer::InstrumentId {
                    admin: "admin1".to_string(),
                    id: "CBTC".to_string(),
                },
                requested_at: "2024-01-01T00:00:00Z".to_string(),
                execute_before: "2024-12-31T23:59:59Z".to_string(),
                input_holding_cids: Some(vec!["cid1".to_string(), "cid2".to_string()]),
                meta: Some(transfer::Meta { values: None }),
            },
            extra_args: ExtraArgs {
                context: Context { values: ctx_values },
                meta: Meta {
                    values: MetaValue {},
                },
            },
        };
        let serialized = serde_json::to_string(&choice_args).unwrap();
        assert!(!serialized.is_empty());
    }
}
