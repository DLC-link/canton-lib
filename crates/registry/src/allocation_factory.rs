use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    #[serde(rename = "choiceArguments")]
    pub choice_arguments: common::allocation_factory::ChoiceArguments,
    #[serde(rename = "excludeDebugFields")]
    pub exclude_debug_fields: bool,
}

#[derive(Debug)]
pub struct Params {
    pub registry_url: String,
    pub decentralized_party_id: String,
    pub request: Request,
}

/// Get the allocation factory and the choice context required to exercise the
/// `AllocationFactory_Allocate` choice for a given allocation specification.
///
/// Mirrors [`crate::transfer_factory::get`] but targets the
/// `allocation-instruction/v1/allocation-factory` endpoint.
///
/// # Errors
///
/// Returns an error string if the request cannot be sent, the registry returns
/// a non-success status, or the response body cannot be parsed.
pub async fn get(params: Params) -> Result<common::allocation_factory::Response, String> {
    let url = format!(
        "{}/api/token-standard/v0/registrars/{}/registry/allocation-instruction/v1/allocation-factory",
        params.registry_url, params.decentralized_party_id
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&params.request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to registry: {e}"))?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "Allocation factory request failed [{status}]: {body_raw:?}"
        ));
    }

    let body = serde_json::from_str(&body_raw)
        .map_err(|e| format!("Failed to parse registry response: {e}"))?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::allocation::{
        AllocationSpecification, Metadata, Reference, SettlementInfo, TransferLeg,
    };
    use common::allocation_factory::ChoiceArguments;
    use common::decimal::DamlDecimal;
    use common::transfer::InstrumentId;
    use common::transfer_factory::{Context, ExtraArgs, Meta, MetaValue};
    use std::collections::HashMap;

    #[test]
    fn request_serializes_with_camel_case_keys() {
        let request = Request {
            choice_arguments: ChoiceArguments {
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
                        amount: DamlDecimal::parse("1.0").unwrap(),
                        instrument_id: InstrumentId {
                            admin: "admin1".to_string(),
                            id: "CBTC".to_string(),
                        },
                        meta: Metadata::default(),
                    },
                },
                requested_at: "2024-01-01T00:00:00Z".to_string(),
                input_holding_cids: vec!["cid1".to_string()],
                extra_args: ExtraArgs {
                    context: Context {
                        values: HashMap::new(),
                    },
                    meta: Meta {
                        values: MetaValue {},
                    },
                },
            },
            exclude_debug_fields: false,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["excludeDebugFields"], false);
        assert_eq!(json["choiceArguments"]["expectedAdmin"], "admin1");
        assert_eq!(json["choiceArguments"]["inputHoldingCids"][0], "cid1");
    }
}
