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

#[derive(Debug, Default)]
pub struct SplitResult {
    pub output_holding_cids: Vec<String>,
    pub change_holding_cids: Vec<String>,
}

/// Error from [`submit`], carrying what the failed run already created.
///
/// `partial` holds the holdings `submit` knows it created before the failure.
/// The failing step itself may also have committed (a response can fail to
/// parse after a successful submission), so callers that need an exact
/// picture should re-query the ACS.
#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub partial: SplitResult,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({} output and {} change holdings already created)",
            self.message,
            self.partial.output_holding_cids.len(),
            self.partial.change_holding_cids.len()
        )
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error {
            message,
            partial: SplitResult::default(),
        }
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
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
            choice: common::consts::CHOICE_TRANSFER_FACTORY_TRANSFER.to_string(),
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
/// Walks `transaction.events`, selects the `ExercisedEvent` of the
/// `TransferFactory_Transfer` choice whose `exercise_result` is an object,
/// then pulls the single `output.value.receiverHoldingCids` entry as the
/// output and `senderChangeCids` as the change list. The standard types
/// `receiverHoldingCids` as a list without a cardinality promise; a split
/// requests one output, so any other length is an error, and the error lists
/// the CIDs because the transaction has already committed. The
/// `exercise_result` payload is a raw `serde_json::Value` because the
/// Daml-encoded variant shape isn't part of the Ledger API schema.
fn parse_split_response(
    response: &JsSubmitAndWaitForTransactionResponse,
) -> Result<(String, Vec<String>), String> {
    let events = &response.transaction.events;

    let mut exercise_result = None;
    for event in events {
        if let Some(exercised) = crate::event_helpers::as_exercised_event(event) {
            if exercised.choice != common::consts::CHOICE_TRANSFER_FACTORY_TRANSFER {
                continue;
            }
            if let Some(Some(result)) = exercised.exercise_result.as_ref() {
                if result.is_object() {
                    exercise_result = Some(result);
                    break;
                }
            }
        }
    }

    let exercise_result = exercise_result.ok_or(format!(
        "Failed to find {} ExercisedEvent",
        common::consts::CHOICE_TRANSFER_FACTORY_TRANSFER
    ))?;

    let receiver_cids = exercise_result["output"]["value"]["receiverHoldingCids"]
        .as_array()
        .ok_or("Failed to extract receiver holding CIDs")?;

    let output_cid = match receiver_cids.as_slice() {
        [only] => only
            .as_str()
            .ok_or("Failed to extract output holding CID")?
            .to_string(),
        other => {
            return Err(format!(
                "Expected exactly one receiver holding CID in split response, got {}: {:?}",
                other.len(),
                other
            ));
        }
    };

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
///
/// Each amount is split in its own ledger transaction, so a failure part-way
/// through leaves the earlier splits committed. The [`Error`] carries the
/// holdings created before the failure.
pub async fn submit(params: Params) -> Result<SplitResult, Error> {
    let mut output_holding_cids = Vec::new();
    let mut current_holdings = params.input_holding_cids;
    let total_splits = params.amounts.len();

    // Split off each amount sequentially
    for (idx, amount) in params.amounts.into_iter().enumerate() {
        let step = split_once(
            params.party.clone(),
            amount,
            params.instrument_id.clone(),
            current_holdings.clone(),
            params.ledger_host.clone(),
            params.access_token.clone(),
            params.registry_url.clone(),
            params.decentralized_party_id.clone(),
        )
        .await;

        let (output_cid, change_cids) = match step {
            Ok(step) => step,
            Err(message) => {
                return Err(Error {
                    message,
                    partial: SplitResult {
                        output_holding_cids,
                        change_holding_cids: current_holdings,
                    },
                });
            }
        };

        output_holding_cids.push(output_cid);
        current_holdings = change_cids;

        if current_holdings.is_empty() && idx + 1 < total_splits {
            return Err(Error {
                message: "Insufficient funds for split".to_string(),
                partial: SplitResult {
                    output_holding_cids,
                    change_holding_cids: current_holdings,
                },
            });
        }
    }

    Ok(SplitResult {
        output_holding_cids,
        change_holding_cids: current_holdings,
    })
}

// Live coverage for `submit` runs through `TokenClient::split` in
// `crate::client`'s `integration_tests` module.

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
                                "00output-cid"
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
    fn skips_exercised_events_of_other_choices() {
        // An earlier object-valued result from a different choice must not
        // be mistaken for the TransferFactory_Transfer result.
        let response = transaction_response(
            "tx-1",
            json!([
                exercised_event_value(
                    "pkg:Some.Other:Template",
                    "SomeOther_Choice",
                    json!({ "output": { "value": { "receiverHoldingCids": ["00decoy"] } } }),
                ),
                exercised_event_value(
                    "pkg:Splice.Api.Token.TransferInstructionV1:TransferFactory",
                    "TransferFactory_Transfer",
                    json!({
                        "senderChangeCids": ["00change-1"],
                        "output": {
                            "tag": "TransferInstructionResult_Completed",
                            "value": { "receiverHoldingCids": ["00output-cid"] }
                        }
                    }),
                )
            ]),
        );

        let (output_cid, change_cids) = parse_split_response(&response).unwrap();
        assert_eq!(output_cid, "00output-cid");
        assert_eq!(change_cids, vec!["00change-1"]);
    }

    #[test]
    fn multiple_receiver_cids_return_err_listing_them() {
        let response = transaction_response(
            "tx-1",
            json!([exercised_event_value(
                "pkg:Splice.Api.Token.TransferInstructionV1:TransferFactory",
                "TransferFactory_Transfer",
                json!({
                    "senderChangeCids": ["00change-1"],
                    "output": {
                        "tag": "TransferInstructionResult_Completed",
                        "value": {
                            "receiverHoldingCids": ["00output-1", "00output-2"]
                        }
                    }
                }),
            )]),
        );

        let err = parse_split_response(&response).unwrap_err();
        assert!(err.contains("got 2"), "unexpected error: {err}");
        // The transaction has committed, so the error must carry the CIDs.
        assert!(
            err.contains("00output-1") && err.contains("00output-2"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_exercised_event_returns_err() {
        // Only a CreatedEvent — parser cannot find an ExercisedEvent.
        let response = transaction_response(
            "tx-x",
            json!([created_event_value("pkg:Some:Template", "00x", json!(null),)]),
        );

        let err = parse_split_response(&response).unwrap_err();
        assert!(
            err.contains("Failed to find TransferFactory_Transfer ExercisedEvent"),
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
            err.contains("Failed to find TransferFactory_Transfer ExercisedEvent"),
            "unexpected error: {err}"
        );
    }
}
