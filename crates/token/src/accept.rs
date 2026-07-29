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
    /// Decentralized party ID for CBTC
    pub decentralized_party_id: String,
}

/// Parameters for accepting all pending CBTC transfers for a party.
pub struct AcceptAllParams {
    /// The receiver party ID
    pub receiver_party: String,
    /// Ledger host URL
    pub ledger_host: String,
    /// Registry URL
    pub registry_url: String,
    /// Decentralized party ID for CBTC
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

/// Accept a CBTC transfer as the receiving party.
///
/// This function performs the following steps:
/// 1. Fetches the choice context from the registry for accepting the transfer
/// 2. Constructs the exercise command for TransferInstruction_Accept
/// 3. Submits the transaction to the ledger
///
/// # Example
/// ```no_run
/// use cbtc::accept;
///
/// let params = accept::Params {
///     transfer_offer_contract_id: "00abc123...".to_string(),
///     receiver_party: "receiver-party::1220...".to_string(),
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "cbtc-network::1220...".to_string(),
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
    let submission_request = common::submission::Submission {
        act_as: vec![params.receiver_party],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: accept_context.disclosed_contracts,
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

/// Accept all pending CBTC transfers for a party.
///
/// This function:
/// 1. Authenticates with Keycloak
/// 2. Fetches all pending TransferInstruction contracts for the party
/// 3. Filters for CBTC transfers where the party is the receiver
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

    // Fetch accept_context once (assumed to be the same for all CBTC transfers in this run)
    log::debug!("Fetching accept context (shared for all CBTC transfers)...");
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
    let num_batches = (total_transfers + BATCH_SIZE - 1) / BATCH_SIZE;

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

            if let Some(create_arg) = &transfer.created_event.create_argument {
                if let Some(transfer_data) = create_arg.get("transfer") {
                    if let Some(amt) = transfer_data.get("amount") {
                        amount = amt.as_str().map(|s| s.to_string());
                        log::debug!("Amount: {}", amt);
                    }
                    if let Some(sndr) = transfer_data.get("sender") {
                        sender = sndr.as_str().map(|s| s.to_string());
                        log::debug!("From: {}", sndr.as_str().unwrap_or("unknown"));
                    }
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

        let submission_request = common::submission::Submission {
            act_as: vec![params.receiver_party.clone()],
            read_as: None,
            command_id: uuid::Uuid::new_v4().to_string(),
            disclosed_contracts: accept_context.disclosed_contracts.clone(),
            commands: batch_commands,
            ..Default::default()
        };

        match ledger::submit::wait_for_transaction(ledger::submit::Params {
            ledger_host: params.ledger_host.clone(),
            access_token: auth.access_token.clone(),
            request: submission_request,
        })
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_accept_transfer() {
        // This test requires a valid transfer_offer_contract_id from an actual transfer
        // Load environment variables from .env file
        dotenvy::dotenv().ok();

        let transfer_offer_cid = std::env::var("LIB_TEST_TRANSFER_OFFER_CID").ok();

        if transfer_offer_cid.is_none() {
            log::debug!("Skipping test: LIB_TEST_TRANSFER_OFFER_CID not set");
            log::debug!(
                "To test this, first create a transfer and set LIB_TEST_TRANSFER_OFFER_CID to the TransferOffer contract ID"
            );
            return;
        }

        // Note: This would require:
        // 1. A valid transfer_offer_contract_id from a pending transfer
        // 2. Authentication as the receiver party
        // 3. The transfer must be in a state ready for acceptance
        log::debug!("Accept transfer test would run here with valid transfer offer");
    }
}
