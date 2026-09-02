//! Reject a token transfer offer as the receiver (`TransferInstruction_Reject`).
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
    /// The token's decentralized party ID (instrument admin)
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
        return Err(format!(
            "Registry request failed with status {status}: {body}"
        ));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registry response: {e}"))
}

/// Reject a token transfer offer as the receiving party.
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

    let submission_request = crate::utils::build_submission(
        vec![params.receiver_party],
        ctx.disclosed_contracts,
        vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
    );

    crate::utils::submit_and_wait(
        &params.ledger_host,
        &params.access_token,
        submission_request,
    )
    .await?;

    Ok(())
}

/// Token Standard V2 form of the reject entry point.
///
/// Unlike V1, this path needs no inline URL: the `registry` crate's V2
/// choice-context function takes the choice as an argument.
pub mod v2 {
    use crate::accept::v2::{fetch_context, instruction_command};
    use crate::utils::{build_submission, submit_and_wait};

    /// The ledger choice this module exercises. Owned here so the tests can
    /// read it back instead of restating the name.
    pub(crate) const CHOICE: &str = "TransferInstruction_Reject";
    /// The registry choice-context route this module fetches.
    pub(crate) const CONTEXT_CHOICE: registry::accept_context::v2::InstructionChoice =
        registry::accept_context::v2::InstructionChoice::Reject;

    pub struct Params {
        /// The contract ID of the TransferInstruction to reject.
        pub transfer_instruction_id: String,
        /// The receiver party ID; must match the transfer's receiver.
        pub receiver_party: String,
        pub ledger_host: String,
        pub access_token: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
    }

    /// Reject one transfer instruction as the receiving party.
    pub async fn submit(params: Params) -> Result<(), String> {
        let context = fetch_context(
            &params.registry_url,
            &params.decentralized_party_id,
            &params.transfer_instruction_id,
            CONTEXT_CHOICE,
        )
        .await?;

        let actors = vec![params.receiver_party];

        let submission = build_submission(
            actors.clone(),
            context.disclosed_contracts.clone(),
            vec![instruction_command(
                &params.transfer_instruction_id,
                CHOICE,
                actors,
                &context,
            )],
        );

        submit_and_wait(&params.ledger_host, &params.access_token, submission).await?;

        Ok(())
    }
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    fn context() -> registry::accept_context::Response {
        serde_json::from_value(serde_json::json!({
            "choiceContextData": { "values": {} },
            "disclosedContracts": []
        }))
        .unwrap()
    }

    #[test]
    fn v2_reject_uses_the_reject_choice_and_route() {
        // Read the module's own constants. Spelling the choice name out here
        // instead would assert only that the test agrees with itself.
        let command = crate::accept::v2::instruction_command(
            "00instruction",
            v2::CHOICE,
            vec!["bob::1220cd".to_string()],
            &context(),
        );
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json["ExerciseCommand"]["choice"],
            serde_json::json!("TransferInstruction_Reject")
        );

        assert_eq!(
            registry::accept_context::v2::context_url(
                "https://r.example",
                "admin::1220ab",
                "00instruction",
                v2::CONTEXT_CHOICE,
            )
            .rsplit('/')
            .next(),
            Some("reject")
        );
    }

    #[tokio::test]
    async fn v2_submit_sends_the_reject_choice_to_the_reject_route() {
        let server = crate::test_utils::stub::instruction_server().await;

        v2::submit(v2::Params {
            transfer_instruction_id: "00instruction".to_string(),
            receiver_party: "bob::1220cd".to_string(),
            ledger_host: server.uri(),
            access_token: "test-access-token".to_string(),
            registry_url: server.uri(),
            decentralized_party_id: "admin::1220ef".to_string(),
        })
        .await
        .expect("the stub answers both boundaries");

        // This is the V1 bug this path must not inherit: fetching one
        // choice's context and exercising another's choice.
        let sent = crate::test_utils::stub::submitted(&server).await;
        assert_eq!(sent.choice, "TransferInstruction_Reject");
        assert!(
            sent.context_path.ends_with("/choice-contexts/reject"),
            "reject must fetch its own context route, got {}",
            sent.context_path
        );
        assert!(
            sent.context_path.contains("/transfer-instruction/v2/"),
            "a V2 operation must fetch the V2 route, got {}",
            sent.context_path
        );
        assert_eq!(sent.actors, vec!["bob::1220cd".to_string()]);
        assert_eq!(sent.act_as, vec!["bob::1220cd".to_string()]);
    }
}
