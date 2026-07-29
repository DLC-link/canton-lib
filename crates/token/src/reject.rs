//! Reject a CBTC transfer offer as the receiver (`TransferInstruction_Reject`).
//!
//! Mirrors [`crate::accept`], but exercises the `TransferInstruction_Reject`
//! choice and fetches the matching `/choice-contexts/reject` registry context.
//! The `registry` crate only ships `accept_context`, so the reject choice-context
//! is fetched here directly (it has the same request/response shape).

/// Parameters for rejecting a transfer offer (receiver side).
pub struct Params {
    /// The contract ID of the TransferOffer/TransferInstruction to reject
    pub transfer_offer_contract_id: String,
    /// The receiver party ID (must match the transfer's receiver)
    pub receiver_party: String,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the receiver party
    pub access_token: String,
    /// Registry URL
    pub registry_url: String,
    /// Decentralized party ID for CBTC
    pub decentralized_party_id: String,
}

/// Fetch the reject choice-context from the registry.
///
/// Same request/response shape as [`registry::accept_context`], targeting the
/// `/choice-contexts/reject` endpoint instead of `/accept`.
///
/// # Errors
/// Returns an error string if the request fails or the response can't be parsed.
async fn reject_context(
    registry_url: &str,
    decentralized_party_id: &str,
    transfer_offer_contract_id: &str,
) -> Result<registry::accept_context::Response, String> {
    let url = format!(
        "{registry_url}/api/token-standard/v0/registrars/{decentralized_party_id}/registry/transfer-instruction/v1/{transfer_offer_contract_id}/choice-contexts/reject"
    );

    let request = registry::accept_context::Request {
        meta: registry::accept_context::Meta {
            values: String::new(),
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to registry: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read response body".to_string());
        return Err(format!("Registry request failed with status {status}: {body}"));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registry response: {e}"))
}

/// Reject a CBTC transfer offer as the receiving party.
///
/// 1. Fetches the reject choice-context from the registry.
/// 2. Constructs the `TransferInstruction_Reject` exercise command.
/// 3. Submits the transaction to the ledger.
///
/// # Errors
/// Returns an error string if the registry context fetch or ledger submission fails.
pub async fn submit(params: Params) -> Result<(), String> {
    let ctx = reject_context(
        &params.registry_url,
        &params.decentralized_party_id,
        &params.transfer_offer_contract_id,
    )
    .await?;

    // `TransferInstruction_Reject` takes the same `ExtraArgs` shape as Accept, so
    // the `Accept` choice-argument variant serializes the wire payload correctly.
    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
            contract_id: params.transfer_offer_contract_id,
            choice: "TransferInstruction_Reject".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                common::accept::ChoiceArguments {
                    extra_args: common::accept::ExtraArgs {
                        context: common::accept::Context {
                            values: ctx.choice_context_data.values,
                        },
                        meta: common::accept::Meta {
                            values: common::accept::MetaValue {},
                        },
                    },
                },
            ),
        },
    };

    let submission_request = common::submission::Submission {
        act_as: vec![params.receiver_party],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: ctx.disclosed_contracts,
        commands: vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
        ..Default::default()
    };

    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: params.ledger_host,
        access_token: params.access_token,
        request: submission_request,
    })
    .await?;

    Ok(())
}
