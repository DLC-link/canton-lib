use ledger::models::JsSubmitAndWaitForTransactionResponse;
use std::collections::HashMap;
use std::ops::Add;

pub struct Params {
    pub party: String,
    pub amounts: Vec<common::decimal::DamlDecimal>,
    pub instrument_id: common::transfer::InstrumentId,
    pub input_holding_cids: Vec<String>,
    pub ledger_host: String,
    pub access_token: String,
    pub registry_url: String,
    pub decentralized_party_id: String,
}

pub struct SplitResult {
    pub output_holding_cids: Vec<String>,
    pub change_holding_cids: Vec<String>,
}

/// Split a single amount using MergeSplit
#[allow(clippy::too_many_arguments)]
async fn split_once(
    party: String,
    amount: common::decimal::DamlDecimal,
    instrument_id: common::transfer::InstrumentId,
    input_holding_cids: Vec<String>,
    ledger_host: String,
    access_token: String,
    registry_url: String,
    decentralized_party_id: String,
) -> Result<(String, Vec<String>), String> {
    // Create metadata with the MergeSplit transaction kind
    let mut transfer_meta: HashMap<String, String> = HashMap::new();
    transfer_meta.insert(
        "splice.lfdecentralizedtrust.org/reason".to_string(),
        "merge-split".to_string(),
    );
    transfer_meta.insert(
        "splice.lfdecentralizedtrust.org/tx-kind".to_string(),
        "merge-split".to_string(),
    );

    // Create a self-transfer (sender == receiver triggers MergeSplit)
    let transfer = common::transfer::Transfer {
        sender: party.clone(),
        receiver: party.clone(), // Self-transfer
        amount,
        instrument_id,
        requested_at: chrono::Utc::now().to_rfc3339(),
        execute_before: chrono::Utc::now()
            .add(chrono::Duration::hours(5))
            .to_rfc3339(),
        input_holding_cids: Some(input_holding_cids),
        meta: Some(common::transfer::Meta {
            values: Some(transfer_meta),
        }),
    };

    let additional_information =
        registry::transfer_factory::get(registry::transfer_factory::Params {
            registry_url,
            decentralized_party_id: decentralized_party_id.clone(),
            request: registry::transfer_factory::Request {
                choice_arguments: common::transfer_factory::ChoiceArguments {
                    expected_admin: decentralized_party_id.clone(),
                    transfer: transfer.clone(),
                    extra_args: common::transfer_factory::ExtraArgs {
                        context: common::transfer_factory::Context {
                            values: HashMap::new(),
                        },
                        meta: common::transfer_factory::Meta {
                            values: common::transfer_factory::MetaValue {},
                        },
                    },
                },
                exclude_debug_fields: true,
            },
        })
        .await?;

    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_FACTORY.to_string(),
            contract_id: additional_information.factory_id,
            choice: "TransferFactory_Transfer".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::TransferFactory(
                common::transfer_factory::ChoiceArguments {
                    expected_admin: decentralized_party_id,
                    transfer: transfer.clone(),
                    extra_args: common::transfer_factory::ExtraArgs {
                        context: additional_information.choice_context.choice_context_data,
                        meta: common::transfer_factory::Meta {
                            values: common::transfer_factory::MetaValue {},
                        },
                    },
                },
            ),
        },
    };

    let submission_request = common::submission::Submission {
        act_as: vec![transfer.sender],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: additional_information.choice_context.disclosed_contracts,
        commands: vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
        ..Default::default()
    };

    let response_raw = ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host,
        access_token,
        request: submission_request,
    })
    .await?;

    // Parse the response to extract the output and change holding CIDs
    let response: JsSubmitAndWaitForTransactionResponse = serde_json::from_str(&response_raw)
        .map_err(|e| format!("Failed to parse submit response: {e}"))?;

    parse_split_response(&response)
}

/// Extract `(output_cid, change_cids)` from a flat-shaped submit response for
/// a split (MergeSplit self-transfer).
///
/// Walks `transaction.events`, finds the first `ExercisedEvent` whose
/// `exercise_result` is an object, then pulls the first
/// `output.value.receiverHoldingCids[0]` as the output and
/// `senderChangeCids` as the change list. The `exercise_result` payload is a
/// raw `serde_json::Value` because the Daml-encoded variant shape isn't part
/// of the Ledger API schema.
fn parse_split_response(
    response: &JsSubmitAndWaitForTransactionResponse,
) -> Result<(String, Vec<String>), String> {
    let events = &response.transaction.events;

    let mut exercise_result = None;
    for event in events {
        if let Some(exercised) = crate::event_helpers::as_exercised_event(event) {
            if let Some(Some(result)) = exercised.exercise_result.as_ref() {
                if result.is_object() {
                    exercise_result = Some(result);
                    break;
                }
            }
        }
    }

    let exercise_result = exercise_result.ok_or("Failed to find ExercisedEvent")?;

    // Extract receiverHoldingCids from output.value.receiverHoldingCids
    let output_cid = exercise_result["output"]["value"]["receiverHoldingCids"][0]
        .as_str()
        .ok_or("Failed to extract output holding CID")?
        .to_string();

    // Extract senderChangeCids (remaining holdings after split)
    let change_cids: Vec<String> = exercise_result["senderChangeCids"]
        .as_array()
        .ok_or("Failed to extract change holding CIDs")?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    Ok((output_cid, change_cids))
}

/// Split holdings into multiple chunks plus change.
/// Takes input holdings and splits them sequentially into the specified amounts.
/// Returns all output holdings plus any remaining change.
pub async fn submit(params: Params) -> Result<SplitResult, String> {
    let mut output_holding_cids = Vec::new();
    let mut current_holdings = params.input_holding_cids;

    // Split off each amount sequentially
    for amount in params.amounts {
        let (output_cid, change_cids) = split_once(
            params.party.clone(),
            amount,
            params.instrument_id.clone(),
            current_holdings,
            params.ledger_host.clone(),
            params.access_token.clone(),
            params.registry_url.clone(),
            params.decentralized_party_id.clone(),
        )
        .await?;

        output_holding_cids.push(output_cid);
        current_holdings = change_cids;

        if current_holdings.is_empty() {
            return Err("Insufficient funds for split".to_string());
        }
    }

    Ok(SplitResult {
        output_holding_cids,
        change_holding_cids: current_holdings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::active_contracts;
    use keycloak::login::{PasswordParams, password, password_url};
    use std::env;

    #[tokio::test]
    async fn test_split() {
        // Load environment variables from .env file
        dotenvy::dotenv().ok();

        let params = PasswordParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
            password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
            url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = password(params).await.unwrap();

        let party = env::var("PARTY_ID").expect("PARTY_ID must be set");
        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let decentralized_party =
            env::var("DECENTRALIZED_PARTY_ID").expect("DECENTRALIZED_PARTY_ID must be set");

        // Get active contracts to use as input
        let contracts = active_contracts::get(active_contracts::Params {
            ledger_host: ledger_host.clone(),
            party: party.clone(),
            access_token: login_response.access_token.clone(),
        })
        .await
        .unwrap();

        assert!(!contracts.is_empty(), "Need at least one contract to split");

        let input_holding_cids: Vec<String> = contracts
            .iter()
            .map(|c| c.created_event.contract_id.clone())
            .collect();

        let split_params = Params {
            party,
            amounts: vec![
                common::decimal::DamlDecimal::parse("1.0").unwrap(),
                common::decimal::DamlDecimal::parse("2.0").unwrap(),
                common::decimal::DamlDecimal::parse("0.5").unwrap(),
            ], // Split into 1.0, 2.0, 0.5, and change
            instrument_id: common::transfer::InstrumentId {
                admin: decentralized_party.clone(),
                id: "CBTC".to_string(),
            },
            input_holding_cids,
            ledger_host,
            access_token: login_response.access_token,
            registry_url: env::var("REGISTRY_URL").expect("REGISTRY_URL must be set"),
            decentralized_party_id: decentralized_party,
        };

        let result = submit(split_params).await.unwrap();

        assert!(!result.output_holding_cids.is_empty());
        assert!(!result.change_holding_cids.is_empty());
    }
}

#[cfg(test)]
mod parser_tests {
    //! Pure-data fixture tests for the flat-event parser used by
    //! `split_once` (`parse_split_response`).

    use super::*;
    use crate::utils::test_fixtures::{
        created_event_value, exercised_event_value, transaction_response,
    };
    use serde_json::json;

    #[test]
    fn happy_path_extracts_output_and_change_cids() {
        let response = transaction_response(
            "tx-1",
            json!([exercised_event_value(
                "pkg:Splice.Api.Token.TransferInstructionV1:TransferFactory",
                "TransferFactory_Transfer",
                json!({
                    "senderChangeCids": [
                        "00change-1",
                        "00change-2"
                    ],
                    "output": {
                        "tag": "TransferInstructionResult_Completed",
                        "value": {
                            "receiverHoldingCids": [
                                "00output-cid",
                                "00ignored-cid"
                            ]
                        }
                    }
                }),
            )]),
        );

        let (output_cid, change_cids) = parse_split_response(&response).unwrap();
        assert_eq!(output_cid, "00output-cid");
        assert_eq!(change_cids, vec!["00change-1", "00change-2"]);
    }

    #[test]
    fn missing_exercised_event_returns_err() {
        // Only a CreatedEvent — parser cannot find an ExercisedEvent.
        let response = transaction_response(
            "tx-x",
            json!([created_event_value(
                "pkg:Some:Template",
                "00x",
                json!(null),
            )]),
        );

        let err = parse_split_response(&response).unwrap_err();
        assert!(
            err.contains("Failed to find ExercisedEvent"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_events_returns_err() {
        // `events` is required on the wire now; pass an empty list and verify
        // the parser falls through to its post-loop check.
        let response = transaction_response("tx-x", json!(null));
        let err = parse_split_response(&response).unwrap_err();
        assert!(
            err.contains("Failed to find ExercisedEvent"),
            "unexpected error: {err}"
        );
    }
}
