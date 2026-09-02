/// Parameters for accepting a transfer.
/// The receiver party must provide authentication to accept the transfer.
pub struct Params {
    /// The contract ID of the TransferOffer/TransferInstruction to accept
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

/// Parameters for accepting all pending transfers of an instrument for a party.
pub struct AcceptAllParams {
    /// The receiver party ID
    pub receiver_party: String,
    /// The instrument whose pending transfers to accept
    pub instrument_id: common::transfer::InstrumentId,
    /// Ledger host URL
    pub ledger_host: String,
    /// Registry URL
    pub registry_url: String,
    /// The token's decentralized party ID (instrument admin)
    pub decentralized_party_id: String,
    // Keycloak authentication
    pub keycloak_client_id: String,
    pub keycloak_username: String,
    pub keycloak_password: String,
    pub keycloak_url: String,
}

/// Result of accepting a single transfer
#[derive(Debug, Clone)]
pub struct AcceptResult {
    pub success: bool,
    pub contract_id: String,
    pub amount: Option<String>,
    pub sender: Option<String>,
    pub error: Option<String>,
}

/// Result of accepting all pending transfers
#[derive(Debug)]
pub struct AcceptAllResult {
    pub results: Vec<AcceptResult>,
    pub successful_count: usize,
    pub failed_count: usize,
}

/// Accept a token transfer as the receiving party.
///
/// This function performs the following steps:
/// 1. Fetches the choice context from the registry for accepting the transfer
/// 2. Constructs the exercise command for TransferInstruction_Accept
/// 3. Submits the transaction to the ledger
///
/// # Example
/// ```ignore
/// use token::accept;
///
/// let params = accept::Params {
///     transfer_offer_contract_id: "00abc123...".to_string(),
///     receiver_party: "receiver-party::1220...".to_string(),
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "token-admin::1220...".to_string(),
/// };
///
/// accept::submit(params).await?;
/// ```
pub async fn submit(params: Params) -> Result<(), String> {
    // Get the choice context for accepting the transfer from the registry
    let accept_context = registry::accept_context::get(registry::accept_context::Params {
        registry_url: params.registry_url,
        decentralized_party_id: params.decentralized_party_id.clone(),
        transfer_offer_contract_id: params.transfer_offer_contract_id.clone(),
        request: registry::accept_context::Request {
            meta: registry::accept_context::Meta {
                values: String::new(),
            },
        },
    })
    .await?;

    // Construct the exercise command to accept the transfer
    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
            contract_id: params.transfer_offer_contract_id,
            choice: "TransferInstruction_Accept".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                common::accept::ChoiceArguments {
                    extra_args: common::accept::ExtraArgs {
                        context: common::accept::Context {
                            values: accept_context.choice_context_data.values,
                        },
                        meta: common::accept::Meta {
                            values: common::accept::MetaValue {},
                        },
                    },
                },
            ),
        },
    };

    // Submit the acceptance transaction
    let submission_request = crate::utils::build_submission(
        vec![params.receiver_party],
        accept_context.disclosed_contracts,
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

/// Accept all pending transfers of an instrument for a party.
///
/// This function:
/// 1. Authenticates with Keycloak
/// 2. Fetches all pending TransferInstruction contracts for the party
/// 3. Filters for transfers of the given instrument where the party is the receiver
/// 4. Batches acceptances into groups of 5 per submission
///
/// Returns a summary of successful and failed acceptances.
pub async fn accept_all(params: AcceptAllParams) -> Result<AcceptAllResult, String> {
    log::debug!("Authenticating with Keycloak...");
    let auth = keycloak::login::password(keycloak::login::PasswordParams {
        client_id: params.keycloak_client_id,
        username: params.keycloak_username,
        password: params.keycloak_password,
        url: params.keycloak_url,
    })
    .await
    .map_err(|e| format!("Authentication failed: {}", e))?;

    log::debug!("✓ Authenticated successfully");

    log::debug!(
        "Checking for pending transfers for party: {}",
        params.receiver_party
    );
    let pending_transfers = crate::utils::fetch_incoming_transfers(
        params.ledger_host.clone(),
        params.receiver_party.clone(),
        auth.access_token.clone(),
        params.instrument_id.clone(),
    )
    .await?;

    if pending_transfers.is_empty() {
        log::debug!("No pending transfers found");
        return Ok(AcceptAllResult {
            results: Vec::new(),
            successful_count: 0,
            failed_count: 0,
        });
    }

    log::debug!("Found {} pending transfer(s)", pending_transfers.len());

    // Fetch accept_context once (assumed to be the same for all transfers in this run)
    log::debug!("Fetching accept context (shared for all transfers)...");
    let first_contract_id = &pending_transfers[0].created_event.contract_id;
    let accept_context = registry::accept_context::get(registry::accept_context::Params {
        registry_url: params.registry_url.clone(),
        decentralized_party_id: params.decentralized_party_id.clone(),
        transfer_offer_contract_id: first_contract_id.clone(),
        request: registry::accept_context::Request {
            meta: registry::accept_context::Meta {
                values: String::new(),
            },
        },
    })
    .await?;
    log::debug!("✓ Accept context fetched\n");

    const BATCH_SIZE: usize = 5;
    let total_transfers = pending_transfers.len();
    let num_batches = total_transfers.div_ceil(BATCH_SIZE);

    log::debug!(
        "Submitting {} acceptances in {} batch(es) of up to {}...",
        total_transfers,
        num_batches,
        BATCH_SIZE
    );

    let mut results = Vec::new();
    let mut successful_count = 0;
    let mut failed_count = 0;

    // Process transfers in chunks of BATCH_SIZE
    for (batch_idx, batch_transfers) in pending_transfers.chunks(BATCH_SIZE).enumerate() {
        let batch_num = batch_idx + 1;
        let start_idx = batch_idx * BATCH_SIZE;
        let end_idx = std::cmp::min(start_idx + batch_transfers.len(), total_transfers);

        log::debug!(
            "--- Batch {}/{}: Preparing acceptances {}-{} ---",
            batch_num,
            num_batches,
            start_idx + 1,
            end_idx
        );

        // Build exercise commands for this batch
        let mut batch_commands = Vec::new();
        let mut batch_results = Vec::new();

        for (idx_in_batch, transfer) in batch_transfers.iter().enumerate() {
            let global_idx = start_idx + idx_in_batch;
            let contract_id = &transfer.created_event.contract_id;
            let short_id = if contract_id.len() > 16 {
                format!(
                    "{}...{}",
                    &contract_id[..8],
                    &contract_id[contract_id.len() - 8..]
                )
            } else {
                contract_id.clone()
            };

            log::debug!("{}. Preparing {}", global_idx + 1, short_id);

            // Extract transfer details from create_argument
            let mut amount = None;
            let mut sender = None;

            if let Some(create_arg) = &transfer.created_event.create_argument
                && let Some(transfer_data) = create_arg.get("transfer")
            {
                if let Some(amt) = transfer_data.get("amount") {
                    amount = amt.as_str().map(|s| s.to_string());
                    log::debug!("Amount: {}", amt);
                }
                if let Some(sndr) = transfer_data.get("sender") {
                    sender = sndr.as_str().map(|s| s.to_string());
                    log::debug!("From: {}", sndr.as_str().unwrap_or("unknown"));
                }
            }

            // Build exercise command using shared context
            let exercise_command = common::submission::ExerciseCommand {
                exercise_command: common::submission::ExerciseCommandData {
                    template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
                    contract_id: contract_id.clone(),
                    choice: "TransferInstruction_Accept".to_string(),
                    choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                        common::accept::ChoiceArguments {
                            extra_args: common::accept::ExtraArgs {
                                context: common::accept::Context {
                                    values: accept_context.choice_context_data.values.clone(),
                                },
                                meta: common::accept::Meta {
                                    values: common::accept::MetaValue {},
                                },
                            },
                        },
                    ),
                },
            };

            batch_commands.push(common::submission::Command::ExerciseCommand(
                exercise_command,
            ));

            // Prepare result tracking for this transfer
            batch_results.push(AcceptResult {
                success: false, // Will update after submission
                contract_id: contract_id.clone(),
                amount,
                sender,
                error: None,
            });
        }

        // Submit this batch
        log::debug!("Submitting batch {}/{}...", batch_num, num_batches);

        let submission_request = crate::utils::build_submission(
            vec![params.receiver_party.clone()],
            accept_context.disclosed_contracts.clone(),
            batch_commands,
        );

        match crate::utils::submit_and_wait(
            &params.ledger_host,
            &auth.access_token,
            submission_request,
        )
        .await
        {
            Ok(_) => {
                log::debug!("  ✓ Batch {}/{} successful", batch_num, num_batches);
                // Mark this batch's results as successful
                for (idx_in_batch, result) in batch_results.iter_mut().enumerate() {
                    result.success = true;
                    successful_count += 1;

                    let short_id = if result.contract_id.len() > 16 {
                        format!(
                            "{}...{}",
                            &result.contract_id[..8],
                            &result.contract_id[result.contract_id.len() - 8..]
                        )
                    } else {
                        result.contract_id.clone()
                    };
                    log::debug!(
                        "    {}. {} [SUCCESS]",
                        start_idx + idx_in_batch + 1,
                        short_id
                    );
                }
            }
            Err(e) => {
                log::debug!("  ✗ Batch {}/{} failed: {}", batch_num, num_batches, e);
                // Mark this batch's results as failed
                for (idx_in_batch, result) in batch_results.iter_mut().enumerate() {
                    result.error = Some(e.clone());
                    failed_count += 1;

                    let short_id = if result.contract_id.len() > 16 {
                        format!(
                            "{}...{}",
                            &result.contract_id[..8],
                            &result.contract_id[result.contract_id.len() - 8..]
                        )
                    } else {
                        result.contract_id.clone()
                    };
                    log::debug!(
                        "    {}. {} [FAILED]",
                        start_idx + idx_in_batch + 1,
                        short_id
                    );
                }
            }
        }

        // Append batch results to overall results
        results.extend(batch_results);
    }

    log::debug!(
        "Summary: Accepted: {}, Failed: {}",
        successful_count,
        failed_count
    );

    Ok(AcceptAllResult {
        successful_count,
        failed_count,
        results,
    })
}

/// Token Standard V2 forms of the accept entry points.
///
/// `actors` is derived from `receiver_party`: the registry accepts exactly
/// `[receiver]` on `TransferInstruction_Accept`, checked at
/// `Splice/TokenStandard/Utils/Internal/Transfers.daml:154`.
pub mod v2 {
    use crate::utils::{build_submission, submit_and_wait};

    /// The ledger choice this module exercises. Owned here so the tests can
    /// read it back instead of restating the name.
    pub(crate) const CHOICE: &str = "TransferInstruction_Accept";
    /// The registry choice-context route this module fetches.
    pub(crate) const CONTEXT_CHOICE: registry::accept_context::v2::InstructionChoice =
        registry::accept_context::v2::InstructionChoice::Accept;

    pub struct Params {
        /// The contract ID of the TransferInstruction to accept.
        pub transfer_instruction_id: String,
        /// The receiver party ID; must match the transfer's receiver.
        pub receiver_party: String,
        pub ledger_host: String,
        pub access_token: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
    }

    pub struct AcceptAllParams {
        pub receiver_party: String,
        pub instrument_id: common::transfer::InstrumentId,
        pub ledger_host: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
        pub keycloak_client_id: String,
        pub keycloak_username: String,
        pub keycloak_password: String,
        pub keycloak_url: String,
    }

    /// A V2 exercise command on a transfer instruction.
    ///
    /// One builder serves `TransferInstruction_Accept`, `_Reject` and
    /// `_Withdraw`: all three take `actors` plus `extraArgs` in V2. The choice
    /// names are unchanged from V1; only the interface id and the
    /// choice-argument shape differ.
    pub(crate) fn instruction_command(
        contract_id: &str,
        choice: &str,
        actors: Vec<String>,
        context: &registry::accept_context::Response,
    ) -> common::submission::Command {
        common::submission::Command::ExerciseCommand(common::submission::ExerciseCommand {
            exercise_command: common::submission::ExerciseCommandData {
                template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION_V2.to_string(),
                contract_id: contract_id.to_string(),
                choice: choice.to_string(),
                choice_argument: common::submission::ChoiceArgumentsVariations::AcceptV2(
                    common::accept::v2::ChoiceArguments {
                        actors,
                        extra_args: common::accept::ExtraArgs {
                            context: common::accept::Context {
                                values: context.choice_context_data.values.clone(),
                            },
                            meta: common::accept::Meta {
                                values: common::accept::MetaValue {},
                            },
                        },
                    },
                ),
            },
        })
    }

    /// Fetch a V2 choice context for one transfer instruction.
    pub(crate) async fn fetch_context(
        registry_url: &str,
        decentralized_party_id: &str,
        transfer_instruction_id: &str,
        choice: registry::accept_context::v2::InstructionChoice,
    ) -> Result<registry::accept_context::Response, String> {
        registry::accept_context::v2::get(registry::accept_context::v2::Params {
            registry_url: registry_url.to_string(),
            decentralized_party_id: decentralized_party_id.to_string(),
            transfer_instruction_id: transfer_instruction_id.to_string(),
            choice,
            request: registry::accept_context::Request {
                meta: registry::accept_context::Meta {
                    values: String::new(),
                },
            },
        })
        .await
    }

    /// Accept one transfer instruction as the receiving party.
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

    /// Accept every pending incoming transfer of an instrument, in batches of 5.
    pub async fn accept_all(params: AcceptAllParams) -> Result<super::AcceptAllResult, String> {
        log::debug!("Authenticating with Keycloak...");
        let auth = keycloak::login::password(keycloak::login::PasswordParams {
            client_id: params.keycloak_client_id,
            username: params.keycloak_username,
            password: params.keycloak_password,
            url: params.keycloak_url,
        })
        .await
        .map_err(|e| format!("Authentication failed: {}", e))?;

        log::debug!(
            "Checking for pending transfers for party: {}",
            params.receiver_party
        );
        let pending_transfers = crate::utils::fetch_incoming_transfers(
            params.ledger_host.clone(),
            params.receiver_party.clone(),
            auth.access_token.clone(),
            params.instrument_id.clone(),
        )
        .await?;

        if pending_transfers.is_empty() {
            log::debug!("No pending transfers found");
            return Ok(super::AcceptAllResult {
                results: Vec::new(),
                successful_count: 0,
                failed_count: 0,
            });
        }

        log::debug!("Found {} pending transfer(s)", pending_transfers.len());

        // One context, shared across the run, as V1 does.
        let context = fetch_context(
            &params.registry_url,
            &params.decentralized_party_id,
            &pending_transfers[0].created_event.contract_id,
            CONTEXT_CHOICE,
        )
        .await?;

        let actors = vec![params.receiver_party.clone()];

        const BATCH_SIZE: usize = 5;
        let total_transfers = pending_transfers.len();
        let num_batches = total_transfers.div_ceil(BATCH_SIZE);

        let mut results = Vec::new();
        let mut successful_count = 0;
        let mut failed_count = 0;

        for (batch_idx, batch_transfers) in pending_transfers.chunks(BATCH_SIZE).enumerate() {
            let batch_num = batch_idx + 1;

            let mut batch_commands = Vec::new();
            let mut batch_results = Vec::new();

            for transfer in batch_transfers {
                let contract_id = &transfer.created_event.contract_id;

                let mut amount = None;
                let mut sender = None;
                if let Some(create_arg) = &transfer.created_event.create_argument
                    && let Some(transfer_data) = create_arg.get("transfer")
                {
                    amount = transfer_data
                        .get("amount")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    sender = transfer_data
                        .get("sender")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }

                batch_commands.push(instruction_command(
                    contract_id,
                    CHOICE,
                    actors.clone(),
                    &context,
                ));

                batch_results.push(super::AcceptResult {
                    success: false,
                    contract_id: contract_id.clone(),
                    amount,
                    sender,
                    error: None,
                });
            }

            log::debug!("Submitting batch {}/{}...", batch_num, num_batches);

            let submission = build_submission(
                actors.clone(),
                context.disclosed_contracts.clone(),
                batch_commands,
            );

            match submit_and_wait(&params.ledger_host, &auth.access_token, submission).await {
                Ok(_) => {
                    log::debug!("  ✓ Batch {}/{} successful", batch_num, num_batches);
                    for result in batch_results.iter_mut() {
                        result.success = true;
                        successful_count += 1;
                    }
                }
                Err(e) => {
                    log::debug!("  ✗ Batch {}/{} failed: {}", batch_num, num_batches, e);
                    for result in batch_results.iter_mut() {
                        result.error = Some(e.clone());
                        failed_count += 1;
                    }
                }
            }

            results.extend(batch_results);
        }

        log::debug!(
            "Summary: Accepted: {}, Failed: {}",
            successful_count,
            failed_count
        );

        Ok(super::AcceptAllResult {
            successful_count,
            failed_count,
            results,
        })
    }
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    fn context() -> registry::accept_context::Response {
        serde_json::from_value(serde_json::json!({
            "choiceContextData": { "values": { "k": "v" } },
            "disclosedContracts": []
        }))
        .unwrap()
    }

    #[test]
    fn v2_accept_command_names_the_v2_interface_and_derived_actors() {
        // Reads the module's own constant. Spelling the choice name out in the
        // call instead would assert only that the test agrees with itself.
        let command = v2::instruction_command(
            "00instruction",
            v2::CHOICE,
            vec!["bob::1220cd".to_string()],
            &context(),
        );

        let json = serde_json::to_value(&command).unwrap();
        let exercised = &json["ExerciseCommand"];
        assert_eq!(
            exercised["templateId"],
            serde_json::json!(common::consts::TEMPLATE_TRANSFER_INSTRUCTION_V2)
        );
        assert_eq!(
            exercised["choice"],
            serde_json::json!("TransferInstruction_Accept")
        );
        assert_eq!(
            exercised["choiceArgument"]["actors"],
            serde_json::json!(["bob::1220cd"])
        );
        assert_eq!(
            exercised["choiceArgument"]["extraArgs"]["context"]["values"]["k"],
            serde_json::json!("v")
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
            Some("accept")
        );
    }

    #[test]
    fn v2_command_round_trips_as_the_accept_v2_variant() {
        // Guards the untagged variant order from the consumer's side: a V2
        // instruction payload must not deserialize back as V1 `Accept`.
        let command = v2::instruction_command(
            "00instruction",
            v2::CHOICE,
            vec!["bob::1220cd".to_string()],
            &context(),
        );
        let json = serde_json::to_value(&command).unwrap();
        let parsed: common::submission::ChoiceArgumentsVariations =
            serde_json::from_value(json["ExerciseCommand"]["choiceArgument"].clone()).unwrap();

        assert!(matches!(
            parsed,
            common::submission::ChoiceArgumentsVariations::AcceptV2(_)
        ));
    }

    #[test]
    fn v2_submission_envelope_matches_the_pinned_shape() {
        // `utils::helper_tests` pins the envelope against a V1 command. This
        // asserts the same envelope fields around a V2 one, so a V2 path
        // cannot quietly grow an extra top-level field.
        let context = context();
        let actors = vec!["bob::1220cd".to_string()];
        let submission = crate::utils::build_submission(
            actors.clone(),
            context.disclosed_contracts.clone(),
            vec![v2::instruction_command(
                "00instruction",
                v2::CHOICE,
                actors,
                &context,
            )],
        );

        let json = serde_json::to_value(&submission).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["actAs", "commandId", "commands", "disclosedContracts"],
            "the V2 envelope must carry exactly the fields V1 carries"
        );
        assert_eq!(json["actAs"], serde_json::json!(["bob::1220cd"]));
        assert_eq!(json["commands"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn v2_submit_sends_the_accept_choice_to_the_accept_route() {
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

        let sent = crate::test_utils::stub::submitted(&server).await;
        assert_eq!(sent.choice, "TransferInstruction_Accept");
        assert!(
            sent.context_path.ends_with("/choice-contexts/accept"),
            "accept must fetch its own context route, got {}",
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
