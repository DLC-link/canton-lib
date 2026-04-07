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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "tag", content = "value")]
pub enum ContextValue {
    #[serde(rename = "AV_Text")]
    Text(String),
    #[serde(rename = "AV_Int")]
    Int(i64),
    #[serde(rename = "AV_Decimal")]
    Decimal(crate::decimal::DamlDecimal),
    #[serde(rename = "AV_Bool")]
    Bool(bool),
    #[serde(rename = "AV_Date")]
    Date(String),
    #[serde(rename = "AV_Time")]
    Time(String),
    #[serde(rename = "AV_RelTime")]
    RelTime(String),
    #[serde(rename = "AV_Party")]
    Party(String),
    #[serde(rename = "AV_ContractId")]
    ContractId(String),
    #[serde(rename = "AV_List")]
    List(Vec<ContextValue>),
    #[serde(rename = "AV_Map")]
    Map(HashMap<String, ContextValue>),
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
    use crate::decimal::DamlDecimal;

    #[test]
    fn test_choice_arguments_serialization() {
        let mut ctx_values: HashMap<String, ContextValue> = HashMap::new();

        let contract_id = "cid1".to_string();

        ctx_values.insert(
            "utility.digitalasset.com/instrument-configuration".to_string(),
            ContextValue::ContractId(contract_id.clone()),
        );
        ctx_values.insert(
            "utility.digitalasset.com/sender-credentials".to_string(),
            ContextValue::List(vec![]),
        );
        ctx_values.insert(
            "utility.digitalasset.com/enable-result-contracts".to_string(),
            ContextValue::Bool(true),
        );
        ctx_values.insert(
            "instrument-configuration".to_string(),
            ContextValue::ContractId(contract_id.clone()),
        );
        ctx_values.insert(
            "sender-credentials".to_string(),
            ContextValue::List(vec![]),
        );

        let choice_args = ChoiceArguments {
            expected_admin: "admin1".to_string(),
            transfer: transfer::Transfer {
                sender: "sender1".to_string(),
                receiver: "receiver1".to_string(),
                amount: DamlDecimal::parse("100.0").unwrap(),
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

        // Verify round-trip: serialized JSON should deserialize back
        let deserialized: ChoiceArguments = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.extra_args.context.values.get("utility.digitalasset.com/enable-result-contracts"),
            Some(&ContextValue::Bool(true))
        );
    }

    #[test]
    fn test_context_deserialization_all_variants() {
        let json = r#"{"values":{
            "bool-field":{"tag":"AV_Bool","value":true},
            "list-field":{"tag":"AV_List","value":[]},
            "contract-id-field":{"tag":"AV_ContractId","value":"cid1"},
            "text-field":{"tag":"AV_Text","value":"hello"},
            "int-field":{"tag":"AV_Int","value":42},
            "decimal-field":{"tag":"AV_Decimal","value":"3.14"},
            "date-field":{"tag":"AV_Date","value":"2024-01-01"},
            "time-field":{"tag":"AV_Time","value":"2024-01-01T00:00:00Z"},
            "reltime-field":{"tag":"AV_RelTime","value":"PT1H"},
            "party-field":{"tag":"AV_Party","value":"party::1220abc"},
            "nested-list":{"tag":"AV_List","value":[{"tag":"AV_ContractId","value":"cid2"}]},
            "map-field":{"tag":"AV_Map","value":{"key1":{"tag":"AV_Text","value":"val1"}}}
        }}"#;
        let ctx: Context = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.values.len(), 12);
        assert_eq!(ctx.values.get("bool-field"), Some(&ContextValue::Bool(true)));
        assert_eq!(ctx.values.get("text-field"), Some(&ContextValue::Text("hello".to_string())));
        assert_eq!(ctx.values.get("int-field"), Some(&ContextValue::Int(42)));
        assert_eq!(ctx.values.get("party-field"), Some(&ContextValue::Party("party::1220abc".to_string())));
        assert_eq!(
            ctx.values.get("decimal-field"),
            Some(&ContextValue::Decimal(DamlDecimal::parse("3.14").unwrap()))
        );
        assert_eq!(
            ctx.values.get("nested-list"),
            Some(&ContextValue::List(vec![ContextValue::ContractId("cid2".to_string())]))
        );
    }
}
