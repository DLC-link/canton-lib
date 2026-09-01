use crate::active_contracts;
use crate::holding::Holding;
use common::decimal::DamlDecimal;
use ledger::models::JsSubmitAndWaitForTransactionResponse;
use std::collections::HashMap;
use std::ops::Add;

/// Result of a consolidation operation
#[derive(Debug)]
pub struct ConsolidationResult {
    /// Whether consolidation was performed
    pub consolidated: bool,
    /// The resulting holding contract IDs after consolidation
    pub holding_cids: Vec<String>,
    /// The number of UTXOs before consolidation
    pub utxos_before: usize,
    /// The number of UTXOs after consolidation
    pub utxos_after: usize,
}

/// Parameters for checking and consolidating UTXOs
pub struct CheckConsolidateParams {
    /// The party ID whose UTXOs to check and consolidate
    pub party: String,
    /// The instrument whose UTXOs to check and consolidate
    pub instrument_id: common::transfer::InstrumentId,
    /// The threshold number of UTXOs. If the party has >= this many UTXOs, consolidation will be performed.
    /// Canton has a soft requirement of max 10 UTXOs per party per token type.
    pub threshold: usize,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the party
    pub access_token: String,
    /// Registry URL
    pub registry_url: String,
    /// The token's decentralized party ID (instrument admin)
    pub decentralized_party_id: String,
}

/// Parameters for getting UTXO count
pub struct GetUtxoCountParams {
    /// The party ID whose UTXOs to count
    pub party: String,
    /// The instrument whose UTXOs to count
    pub instrument_id: common::transfer::InstrumentId,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the party
    pub access_token: String,
    /// Counts only this account's holdings when `Some`. Must match whatever
    /// the caller will then consolidate.
    pub account: Option<common::transfer::v2::Account>,
}

/// Parameters for consolidating UTXOs
pub struct ConsolidateParams {
    /// The party ID whose UTXOs to consolidate
    pub party: String,
    /// The instrument to consolidate
    pub instrument_id: common::transfer::InstrumentId,
    /// Optional specific holding CIDs to consolidate. If None, all holdings will be consolidated.
    pub input_holding_cids: Option<Vec<String>>,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the party
    pub access_token: String,
    /// Registry URL
    pub registry_url: String,
    /// The token's decentralized party ID (instrument admin)
    pub decentralized_party_id: String,
}

/// Get the count of an instrument's UTXOs for a party.
///
/// # Example
/// ```ignore
/// use token::consolidate;
///
/// let params = consolidate::GetUtxoCountParams {
///     party: "party::1220...".to_string(),
///     instrument_id: common::transfer::InstrumentId {
///         admin: "token-admin::1220...".to_string(),
///         id: "TOKEN".to_string(),
///     },
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     account: None,
/// };
///
/// let count = consolidate::get_utxo_count(params).await?;
/// log::debug!("Party has {} UTXOs", count);
/// ```
pub async fn get_utxo_count(params: GetUtxoCountParams) -> Result<usize, String> {
    let contracts = active_contracts::get(active_contracts::Params {
        ledger_host: params.ledger_host,
        party: params.party,
        access_token: params.access_token,
        instrument_id: params.instrument_id,
        account: params.account,
    })
    .await?;

    Ok(contracts.len())
}

/// Consolidate all of an instrument's UTXOs into a single UTXO via self-transfer.
///
/// This performs a "merge-split" operation where the party sends all their
/// holdings to themselves, resulting in a single consolidated UTXO.
///
/// # Example
/// ```ignore
/// use token::consolidate;
///
/// let params = consolidate::ConsolidateParams {
///     party: "party::1220...".to_string(),
///     instrument_id: common::transfer::InstrumentId {
///         admin: "token-admin::1220...".to_string(),
///         id: "TOKEN".to_string(),
///     },
///     input_holding_cids: None, // Consolidate all holdings
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "token-admin::1220...".to_string(),
/// };
///
/// let result_cids = consolidate::consolidate_utxos(params).await?;
/// log::debug!("Consolidated into {} UTXO(s)", result_cids.len());
/// ```
pub async fn consolidate_utxos(params: ConsolidateParams) -> Result<Vec<String>, String> {
    // Get the holdings to consolidate
    let input_holding_cids = if let Some(cids) = params.input_holding_cids {
        cids
    } else {
        let contracts = active_contracts::get(active_contracts::Params {
            ledger_host: params.ledger_host.clone(),
            party: params.party.clone(),
            access_token: params.access_token.clone(),
            instrument_id: params.instrument_id.clone(),
            account: None,
        })
        .await?;

        contracts
            .iter()
            .map(|c| c.created_event.contract_id.clone())
            .collect()
    };

    if input_holding_cids.is_empty() {
        return Err("No holdings to consolidate".to_string());
    }

    if input_holding_cids.len() == 1 {
        // Already consolidated to a single UTXO
        return Ok(input_holding_cids);
    }

    // Calculate total amount to consolidate
    let contracts = active_contracts::get(active_contracts::Params {
        ledger_host: params.ledger_host.clone(),
        party: params.party.clone(),
        access_token: params.access_token.clone(),
        instrument_id: params.instrument_id.clone(),
        account: None,
    })
    .await?;

    let zero = DamlDecimal::ZERO;

    let holdings: Vec<Holding> = contracts
        .iter()
        .filter(|c| input_holding_cids.contains(&c.created_event.contract_id))
        .map(Holding::from_active_contract)
        .collect::<Result<Vec<_>, _>>()?;

    let total_amount: DamlDecimal = holdings.iter().map(|h| h.amount).sum();

    if total_amount == zero {
        return Err("Total amount to consolidate is zero".to_string());
    }

    // Create metadata with the MergeSplit transaction kind
    let transfer_meta = crate::utils::merge_split_meta("UTXO consolidation");

    // Create a self-transfer to consolidate (sender == receiver)
    let transfer = common::transfer::Transfer {
        sender: params.party.clone(),
        receiver: params.party.clone(), // Self-transfer triggers consolidation
        amount: total_amount,
        instrument_id: params.instrument_id,
        requested_at: chrono::Utc::now().to_rfc3339(),
        execute_before: chrono::Utc::now()
            .add(chrono::Duration::hours(5))
            .to_rfc3339(),
        input_holding_cids: Some(input_holding_cids),
        meta: Some(transfer_meta),
    };

    // Get registry information for the transfer
    let additional_information =
        registry::transfer_factory::get(registry::transfer_factory::Params {
            registry_url: params.registry_url,
            decentralized_party_id: params.decentralized_party_id.clone(),
            request: registry::transfer_factory::Request {
                choice_arguments: common::transfer_factory::ChoiceArguments {
                    expected_admin: params.decentralized_party_id.clone(),
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

    // Submit the consolidation transaction
    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_FACTORY.to_string(),
            contract_id: additional_information.factory_id,
            choice: common::consts::CHOICE_TRANSFER_FACTORY_TRANSFER.to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::TransferFactory(
                common::transfer_factory::ChoiceArguments {
                    expected_admin: params.decentralized_party_id,
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

    let submission_request = crate::utils::build_submission(
        vec![transfer.sender.clone()],
        additional_information.choice_context.disclosed_contracts,
        vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
    );

    let response_raw = crate::utils::submit_and_wait(
        &params.ledger_host,
        &params.access_token,
        submission_request,
    )
    .await?;

    // Parse the response to extract the resulting holding CID(s)
    let response: JsSubmitAndWaitForTransactionResponse = serde_json::from_str(&response_raw)
        .map_err(|e| format!("Failed to parse submit response: {e}"))?;

    parse_consolidate_response(&response)
}

/// Extract the resulting `receiverHoldingCids` from a flat-shaped submit
/// response for a UTXO consolidation (self-transfer) operation.
///
/// Walks `transaction.events`, finds the `ExercisedEvent` of the
/// `TransferFactory_Transfer` choice, and pulls
/// `exerciseResult.output.value.receiverHoldingCids`. The `exercise_result`
/// payload stays as raw `serde_json::Value` because its inner shape is a
/// Daml-encoded variant that isn't part of the Ledger API schema.
fn parse_consolidate_response(
    response: &JsSubmitAndWaitForTransactionResponse,
) -> Result<Vec<String>, String> {
    let events = &response.transaction.events;

    let mut result_cids = Vec::new();
    for event in events {
        if let Some(exercised) = crate::event_helpers::as_exercised_event(event) {
            if exercised.choice != common::consts::CHOICE_TRANSFER_FACTORY_TRANSFER {
                continue;
            }
            if let Some(Some(result)) = exercised.exercise_result.as_ref() {
                // Extract receiverHoldingCids from the Daml-encoded payload
                if let Some(receiver_cids) =
                    result["output"]["value"]["receiverHoldingCids"].as_array()
                {
                    for cid in receiver_cids {
                        if let Some(cid_str) = cid.as_str() {
                            result_cids.push(cid_str.to_string());
                        }
                    }
                }
                break;
            }
        }
    }

    if result_cids.is_empty() {
        return Err(
            "Failed to extract result holding CIDs from consolidation response".to_string(),
        );
    }

    Ok(result_cids)
}

/// Check the UTXO count for a party and consolidate if it meets or exceeds the threshold.
///
/// This is the main function teams should use to ensure they don't exceed Canton's
/// soft limit of 10 UTXOs per party per token type.
///
/// # Example
/// ```ignore
/// use token::consolidate;
///
/// let params = consolidate::CheckConsolidateParams {
///     party: "party::1220...".to_string(),
///     instrument_id: common::transfer::InstrumentId {
///         admin: "token-admin::1220...".to_string(),
///         id: "TOKEN".to_string(),
///     },
///     threshold: 10,
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "token-admin::1220...".to_string(),
/// };
///
/// let result = consolidate::check_and_consolidate(params).await?;
/// if result.consolidated {
///     log::debug!("Consolidated {} UTXOs into {}", result.utxos_before, result.utxos_after);
/// } else {
///     log::debug!("No consolidation needed. Party has {} UTXOs", result.utxos_before);
/// }
/// ```
pub async fn check_and_consolidate(
    params: CheckConsolidateParams,
) -> Result<ConsolidationResult, String> {
    // Get current UTXO count
    let utxo_count = get_utxo_count(GetUtxoCountParams {
        party: params.party.clone(),
        instrument_id: params.instrument_id.clone(),
        ledger_host: params.ledger_host.clone(),
        access_token: params.access_token.clone(),
        account: None,
    })
    .await?;

    log::debug!(
        "Party has {} UTXOs (threshold: {})",
        utxo_count,
        params.threshold
    );

    // Check if consolidation is needed
    if utxo_count < params.threshold {
        return Ok(ConsolidationResult {
            consolidated: false,
            holding_cids: vec![],
            utxos_before: utxo_count,
            utxos_after: utxo_count,
        });
    }

    log::debug!("Threshold met or exceeded. Consolidating UTXOs...");

    // Perform consolidation
    let result_cids = consolidate_utxos(ConsolidateParams {
        party: params.party,
        instrument_id: params.instrument_id,
        input_holding_cids: None, // Consolidate all holdings
        ledger_host: params.ledger_host,
        access_token: params.access_token,
        registry_url: params.registry_url,
        decentralized_party_id: params.decentralized_party_id,
    })
    .await?;

    Ok(ConsolidationResult {
        consolidated: true,
        holding_cids: result_cids.clone(),
        utxos_before: utxo_count,
        utxos_after: result_cids.len(),
    })
}

// Live coverage for `get_utxo_count` and `check_and_consolidate` runs
// through `TokenClient` in `crate::client`'s `integration_tests` module.

/// Token Standard V2 forms of the consolidation entry points.
///
/// `get_utxo_count` stays version-neutral, but Task 4a gives it the same
/// registry, so these entry points call it with the account's derived owner.
pub mod v2 {
    use crate::split::v2::self_transfer;
    use crate::transfer::v2::{factory_command, fetch_factory};
    use crate::utils::{build_submission, require_owner, submit_and_wait};
    use common::decimal::DamlDecimal;

    pub struct ConsolidateParams {
        pub account: common::transfer::v2::Account,
        pub instrument_id: common::transfer::InstrumentId,
        pub input_holding_cids: Option<Vec<String>>,
        pub ledger_host: String,
        pub access_token: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
    }

    pub struct CheckConsolidateParams {
        pub account: common::transfer::v2::Account,
        pub instrument_id: common::transfer::InstrumentId,
        pub threshold: usize,
        pub ledger_host: String,
        pub access_token: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
    }

    /// Merge all of an instrument's holdings into a single UTXO via a
    /// merge-split self-transfer.
    pub async fn consolidate_utxos(params: ConsolidateParams) -> Result<Vec<String>, String> {
        let owner = require_owner(&params.account, "account")?;
        let actors = vec![owner.clone()];

        let input_holding_cids = if let Some(cids) = params.input_holding_cids {
            cids
        } else {
            let contracts = crate::active_contracts::get(crate::active_contracts::Params {
                ledger_host: params.ledger_host.clone(),
                party: owner.clone(),
                access_token: params.access_token.clone(),
                instrument_id: params.instrument_id.clone(),
                // Task 4a. Consolidate only within the caller's own account.
                account: Some(params.account.clone()),
            })
            .await?;

            contracts
                .iter()
                .map(|c| c.created_event.contract_id.clone())
                .collect()
        };

        if input_holding_cids.is_empty() {
            return Err("No holdings to consolidate".to_string());
        }

        if input_holding_cids.len() == 1 {
            return Ok(input_holding_cids);
        }

        let contracts = crate::active_contracts::get(crate::active_contracts::Params {
            ledger_host: params.ledger_host.clone(),
            party: owner,
            access_token: params.access_token.clone(),
            instrument_id: params.instrument_id.clone(),
            account: Some(params.account.clone()),
        })
        .await?;

        let holdings: Vec<crate::holding::Holding> = contracts
            .iter()
            .filter(|c| input_holding_cids.contains(&c.created_event.contract_id))
            .map(crate::holding::Holding::from_active_contract)
            .collect::<Result<Vec<_>, _>>()?;

        // The query above filters by account label; `input_holding_cids` does
        // not. A caller who supplies holdings from another label would
        // otherwise get them silently dropped here — and a *partial* mismatch
        // is worse than a total one, because the transfer would go to the
        // ledger spending inputs the amount does not cover.
        if holdings.len() != input_holding_cids.len() {
            let found: Vec<&String> = holdings.iter().map(|h| &h.contract_id).collect();
            let missing: Vec<&String> = input_holding_cids
                .iter()
                .filter(|cid| !found.contains(cid))
                .collect();
            return Err(format!(
                "{} of {} input holdings do not belong to this account: {missing:?}",
                missing.len(),
                input_holding_cids.len()
            ));
        }

        let total_amount: DamlDecimal = holdings.iter().map(|h| h.amount).sum();

        if total_amount == DamlDecimal::ZERO {
            return Err("Total amount to consolidate is zero".to_string());
        }

        let transfer = self_transfer(
            &params.account,
            total_amount,
            params.instrument_id,
            input_holding_cids,
            "UTXO consolidation",
        );

        let additional_information = fetch_factory(
            &params.registry_url,
            &params.decentralized_party_id,
            transfer.clone(),
            actors.clone(),
        )
        .await?;

        let submission = build_submission(
            actors.clone(),
            additional_information.choice_context.disclosed_contracts,
            vec![factory_command(
                additional_information.factory_id,
                transfer,
                actors,
                additional_information.choice_context.choice_context_data,
            )],
        );

        let response_raw =
            submit_and_wait(&params.ledger_host, &params.access_token, submission).await?;

        let response: ledger::models::JsSubmitAndWaitForTransactionResponse =
            serde_json::from_str(&response_raw)
                .map_err(|e| format!("Failed to parse submit response: {e}"))?;

        super::parse_consolidate_response(&response)
    }

    /// Consolidate only when the UTXO count reaches `threshold`.
    pub async fn check_and_consolidate(
        params: CheckConsolidateParams,
    ) -> Result<super::ConsolidationResult, String> {
        let owner = require_owner(&params.account, "account")?;

        let utxo_count = super::get_utxo_count(super::GetUtxoCountParams {
            party: owner,
            instrument_id: params.instrument_id.clone(),
            ledger_host: params.ledger_host.clone(),
            access_token: params.access_token.clone(),
            // Task 4a. Count the same holdings the consolidation will merge.
            // Passing `None` here would compare a whole-party count against a
            // per-account consolidation, and the threshold would fire wrongly.
            account: Some(params.account.clone()),
        })
        .await?;

        log::debug!(
            "Party has {} UTXOs (threshold: {})",
            utxo_count,
            params.threshold
        );

        if utxo_count < params.threshold {
            return Ok(super::ConsolidationResult {
                consolidated: false,
                holding_cids: vec![],
                utxos_before: utxo_count,
                utxos_after: utxo_count,
            });
        }

        log::debug!("Threshold met or exceeded. Consolidating UTXOs...");

        let result_cids = consolidate_utxos(ConsolidateParams {
            account: params.account,
            instrument_id: params.instrument_id,
            input_holding_cids: None,
            ledger_host: params.ledger_host,
            access_token: params.access_token,
            registry_url: params.registry_url,
            decentralized_party_id: params.decentralized_party_id,
        })
        .await?;

        Ok(super::ConsolidationResult {
            consolidated: true,
            holding_cids: result_cids.clone(),
            utxos_before: utxo_count,
            utxos_after: result_cids.len(),
        })
    }
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    fn special_account() -> common::transfer::v2::Account {
        common::transfer::v2::Account {
            owner: None,
            provider: None,
            id: String::new(),
        }
    }

    fn consolidate_params(input_holding_cids: Option<Vec<String>>) -> v2::ConsolidateParams {
        v2::ConsolidateParams {
            account: common::transfer::v2::Account::basic("alice::1220ab"),
            instrument_id: common::transfer::InstrumentId {
                admin: "admin::1220ef".to_string(),
                id: "CBTC".to_string(),
            },
            input_holding_cids,
            ledger_host: "http://127.0.0.1:1".to_string(),
            access_token: "unused".to_string(),
            registry_url: "http://127.0.0.1:1".to_string(),
            decentralized_party_id: "admin::1220ef".to_string(),
        }
    }

    #[tokio::test]
    async fn v2_consolidate_rejects_a_special_account_before_any_request() {
        let err = v2::consolidate_utxos(v2::ConsolidateParams {
            account: special_account(),
            instrument_id: common::transfer::InstrumentId {
                admin: "admin::1220ef".to_string(),
                id: "CBTC".to_string(),
            },
            input_holding_cids: Some(vec!["00abc".to_string(), "00def".to_string()]),
            ledger_host: "http://127.0.0.1:1".to_string(),
            access_token: "unused".to_string(),
            registry_url: "http://127.0.0.1:1".to_string(),
            decentralized_party_id: "admin::1220ef".to_string(),
        })
        .await
        .unwrap_err();

        assert!(
            err.contains("account"),
            "must name the parameter, got {err}"
        );
    }

    #[tokio::test]
    async fn v2_check_and_consolidate_rejects_a_special_account_before_any_request() {
        let err = v2::check_and_consolidate(v2::CheckConsolidateParams {
            account: special_account(),
            instrument_id: common::transfer::InstrumentId {
                admin: "admin::1220ef".to_string(),
                id: "CBTC".to_string(),
            },
            threshold: 10,
            ledger_host: "http://127.0.0.1:1".to_string(),
            access_token: "unused".to_string(),
            registry_url: "http://127.0.0.1:1".to_string(),
            decentralized_party_id: "admin::1220ef".to_string(),
        })
        .await
        .unwrap_err();

        assert!(
            err.contains("account"),
            "must name the parameter, got {err}"
        );
    }

    #[tokio::test]
    async fn v2_consolidate_rejects_an_empty_holding_list_before_any_request() {
        let err = v2::consolidate_utxos(consolidate_params(Some(vec![])))
            .await
            .unwrap_err();
        assert_eq!(err, "No holdings to consolidate");
    }

    #[tokio::test]
    async fn v2_consolidate_returns_a_single_holding_untouched() {
        // The only success path that submits nothing. A regression here would
        // send a one-input merge-split to the registry.
        let cids = v2::consolidate_utxos(consolidate_params(Some(vec!["00abc".to_string()])))
            .await
            .unwrap();
        assert_eq!(cids, vec!["00abc".to_string()]);
    }
}

#[cfg(test)]
mod parser_tests {
    //! Pure-data fixture tests for the flat-event parser used by
    //! `consolidate_utxos` (`parse_consolidate_response`).

    use super::*;
    use crate::utils::test_fixtures::{
        created_event_value, exercised_event_value, transaction_response,
    };
    use serde_json::json;

    #[test]
    fn happy_path_extracts_receiver_holding_cids() {
        let response = transaction_response(
            "tx-1",
            json!([exercised_event_value(
                "pkg:Splice.Api.Token.TransferInstructionV1:TransferFactory",
                "TransferFactory_Transfer",
                json!({
                    "senderChangeCids": [],
                    "output": {
                        "tag": "TransferInstructionResult_Completed",
                        "value": {
                            "receiverHoldingCids": [
                                "00recv-1",
                                "00recv-2"
                            ]
                        }
                    }
                }),
            )]),
        );

        let cids = parse_consolidate_response(&response).unwrap();
        assert_eq!(cids, vec!["00recv-1", "00recv-2"]);
    }

    #[test]
    fn missing_exercised_event_returns_err() {
        // Only a CreatedEvent present — no ExercisedEvent at all.
        let response = transaction_response(
            "tx-x",
            json!([created_event_value("pkg:Some:Template", "00x", json!(null),)]),
        );

        let err = parse_consolidate_response(&response).unwrap_err();
        assert!(
            err.contains("Failed to extract result holding CIDs"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_events_returns_err() {
        // `events` is required on the wire now, so an empty list stands in for
        // "missing events"; the parser falls through to its post-loop check.
        let response = transaction_response("tx-x", json!(null));
        let err = parse_consolidate_response(&response).unwrap_err();
        assert!(
            err.contains("Failed to extract result holding CIDs"),
            "unexpected error: {err}"
        );
    }
}
