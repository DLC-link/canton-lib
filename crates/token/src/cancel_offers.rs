/// Parameters for withdrawing a transfer.
/// The sender party must provide authentication to withdraw the transfer.
pub struct Params {
    /// The contract ID of the TransferOffer/TransferInstruction to withdraw
    pub transfer_offer_contract_id: String,
    /// The sender party ID (must match the transfer's sender)
    pub sender_party: String,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the sender party
    pub access_token: String,
    /// Registry URL
    pub registry_url: String,
    /// Decentralized party ID for CBTC
    pub decentralized_party_id: String,
}

/// Parameters for withdrawing a specific set of CBTC transfer offers (by contract
/// id) as the sending party, using an existing access token. Submissions are
/// batched; the registry withdraw-context is fetched once and shared.
pub struct WithdrawBatchParams {
    /// Contract IDs of the TransferOffer/TransferInstructions to withdraw
    pub contract_ids: Vec<String>,
    /// The sender party ID
    pub sender_party: String,
    /// Ledger host URL
    pub ledger_host: String,
    /// Access token for the sender party
    pub access_token: String,
    /// Registry URL
    pub registry_url: String,
    /// Decentralized party ID for CBTC
    pub decentralized_party_id: String,
}

/// Parameters for withdrawing all pending CBTC transfers for a party.
pub struct WithdrawAllParams {
    /// The sender party ID
    pub sender_party: String,
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

/// Result of withdrawing a single transfer
#[derive(Debug, Clone)]
pub struct WithdrawResult {
    pub success: bool,
    pub contract_id: String,
    pub amount: Option<String>,
    pub receiver: Option<String>,
    pub error: Option<String>,
}

/// Result of withdrawing all pending transfers
#[derive(Debug)]
pub struct WithdrawAllResult {
    pub results: Vec<WithdrawResult>,
    pub successful_count: usize,
    pub failed_count: usize,
}

/// Withdraw a CBTC transfer as the sending party.
///
/// This function performs the following steps:
/// 1. Fetches the choice context from the registry for withdrawing the transfer
/// 2. Constructs the exercise command for TransferInstruction_Withdraw
/// 3. Submits the transaction to the ledger
///
/// # Example
/// ```no_run
/// use cbtc::withdraw;
///
/// let params = withdraw::Params {
///     transfer_offer_contract_id: "00abc123...".to_string(),
///     sender_party: "sender-party::1220...".to_string(),
///     ledger_host: "https://participant.example.com".to_string(),
///     access_token: "eyJ...".to_string(),
///     registry_url: "https://api.utilities.digitalasset-dev.com".to_string(),
///     decentralized_party_id: "cbtc-network::1220...".to_string(),
/// };
///
/// withdraw::submit(params).await?;
/// ```
pub async fn submit(params: Params) -> Result<(), String> {
    // Get the choice context for withdrawing the transfer from the registry
    // Note: Using accept_context as the registry endpoint for withdraw context
    let withdraw_context = registry::accept_context::get(registry::accept_context::Params {
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

    // Construct the exercise command to withdraw the transfer
    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
            contract_id: params.transfer_offer_contract_id,
            choice: "TransferInstruction_Withdraw".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                common::accept::ChoiceArguments {
                    extra_args: common::accept::ExtraArgs {
                        context: common::accept::Context {
                            values: withdraw_context.choice_context_data.values,
                        },
                        meta: common::accept::Meta {
                            values: common::accept::MetaValue {},
                        },
                    },
                },
            ),
        },
    };

    // Submit the withdrawal transaction
    let submission_request = common::submission::Submission {
        act_as: vec![params.sender_party],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: withdraw_context.disclosed_contracts,
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

/// Build a single `TransferInstruction_Withdraw` exercise command from a shared context.
fn build_withdraw_command(
    contract_id: &str,
    context: &registry::accept_context::Response,
) -> common::submission::Command {
    common::submission::Command::ExerciseCommand(common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
            contract_id: contract_id.to_string(),
            choice: "TransferInstruction_Withdraw".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                common::accept::ChoiceArguments {
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

/// Submit a withdraw for the given contract ids as one (atomic) transaction.
async fn submit_withdraws(
    contract_ids: &[String],
    sender_party: &str,
    ledger_host: &str,
    access_token: &str,
    context: &registry::accept_context::Response,
) -> Result<(), String> {
    let commands = contract_ids
        .iter()
        .map(|cid| build_withdraw_command(cid, context))
        .collect();
    let submission_request = common::submission::Submission {
        act_as: vec![sender_party.to_string()],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: context.disclosed_contracts.clone(),
        commands,
        ..Default::default()
    };
    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: ledger_host.to_string(),
        access_token: access_token.to_string(),
        request: submission_request,
    })
    .await
    .map(|_| ())
}

/// Record one offer's outcome into the running result tally.
fn record_withdraw(
    results: &mut Vec<WithdrawResult>,
    successful_count: &mut usize,
    failed_count: &mut usize,
    contract_id: &str,
    outcome: Result<(), String>,
) {
    let (success, error) = match outcome {
        Ok(()) => {
            *successful_count += 1;
            (true, None)
        }
        Err(e) => {
            *failed_count += 1;
            (false, Some(e))
        }
    };
    results.push(WithdrawResult {
        success,
        contract_id: contract_id.to_string(),
        amount: None,
        receiver: None,
        error,
    });
}

/// Withdraw a specific set of CBTC transfer offers by contract id, batched.
///
/// Like [`withdraw_all`] but operates on a provided list of contract ids and an
/// existing access token (no re-authentication). The registry withdraw-context
/// is fetched once (shared across CBTC transfers) and reused for every command;
/// commands are submitted in batches of 5. Because a Canton transaction is
/// atomic, a batch containing a non-withdrawable offer is retried per-offer so
/// the withdrawable offers still succeed and only the offender(s) fail.
///
/// # Errors
/// Returns an error string only if the shared registry context cannot be fetched.
/// Individual offer failures are recorded in the returned result (with the error)
/// rather than aborting the whole run.
pub async fn withdraw_batch(params: WithdrawBatchParams) -> Result<WithdrawAllResult, String> {
    if params.contract_ids.is_empty() {
        return Ok(WithdrawAllResult {
            results: Vec::new(),
            successful_count: 0,
            failed_count: 0,
        });
    }

    // Fetch the withdraw context once (same for all CBTC transfers).
    let withdraw_context = registry::accept_context::get(registry::accept_context::Params {
        registry_url: params.registry_url.clone(),
        decentralized_party_id: params.decentralized_party_id.clone(),
        transfer_offer_contract_id: params.contract_ids[0].clone(),
        request: registry::accept_context::Request {
            meta: registry::accept_context::Meta {
                values: String::new(),
            },
        },
    })
    .await?;

    const BATCH_SIZE: usize = 5;
    let mut results = Vec::new();
    let mut successful_count = 0;
    let mut failed_count = 0;

    for batch in params.contract_ids.chunks(BATCH_SIZE) {
        let outcome = submit_withdraws(
            batch,
            &params.sender_party,
            &params.ledger_host,
            &params.access_token,
            &withdraw_context,
        )
        .await;

        match outcome {
            Ok(()) => {
                for cid in batch {
                    record_withdraw(&mut results, &mut successful_count, &mut failed_count, cid, Ok(()));
                }
            }
            // A Canton transaction is atomic: one non-withdrawable offer fails the
            // whole batch. Retry each offer individually so the good ones still get
            // cancelled and the offender(s) fail in isolation with their own error.
            Err(_) if batch.len() > 1 => {
                for cid in batch {
                    let single = submit_withdraws(
                        std::slice::from_ref(cid),
                        &params.sender_party,
                        &params.ledger_host,
                        &params.access_token,
                        &withdraw_context,
                    )
                    .await;
                    record_withdraw(&mut results, &mut successful_count, &mut failed_count, cid, single);
                }
            }
            Err(e) => {
                record_withdraw(&mut results, &mut successful_count, &mut failed_count, &batch[0], Err(e));
            }
        }
    }

    Ok(WithdrawAllResult {
        results,
        successful_count,
        failed_count,
    })
}

/// Withdraw all pending CBTC transfers for a party (transfers sent by this party).
///
/// This function:
/// 1. Authenticates with Keycloak
/// 2. Fetches all pending TransferInstruction contracts sent by the party
/// 3. Filters for CBTC transfers where the party is the sender
/// 4. Batches withdrawals into groups of 5 per submission
///
/// Returns a summary of successful and failed withdrawals.
pub async fn withdraw_all(params: WithdrawAllParams) -> Result<WithdrawAllResult, String> {
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
        "Checking for pending transfers sent by party: {}",
        params.sender_party
    );

    // Fetch pending transfer instructions sent by this party
    let pending_transfers = crate::utils::fetch_outgoing_transfers(
        params.ledger_host.clone(),
        params.sender_party.clone(),
        auth.access_token.clone(),
    )
    .await?;

    if pending_transfers.is_empty() {
        log::debug!("No pending outgoing transfers found");
        return Ok(WithdrawAllResult {
            results: Vec::new(),
            successful_count: 0,
            failed_count: 0,
        });
    }

    log::debug!(
        "Found {} pending outgoing transfer(s)",
        pending_transfers.len()
    );

    // Fetch withdraw_context once (same for all CBTC transfers)
    log::debug!("Fetching withdraw context (shared for all CBTC transfers)...");
    let first_contract_id = &pending_transfers[0].created_event.contract_id;
    let withdraw_context = registry::accept_context::get(registry::accept_context::Params {
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
    log::debug!("✓ Withdraw context fetched\n");

    // Build and submit commands in batches of 5
    const BATCH_SIZE: usize = 5;
    let total_transfers = pending_transfers.len();
    let num_batches = (total_transfers + BATCH_SIZE - 1) / BATCH_SIZE;

    log::debug!(
        "\nSubmitting {} withdrawals in {} batch(es) of up to {}...",
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
            "--- Batch {}/{}: Preparing withdrawals {}-{} ---",
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

            log::debug!("  {}. Preparing {}", global_idx + 1, short_id);

            // Extract transfer details from create_argument
            let mut amount = None;
            let mut receiver = None;

            if let Some(create_arg) = &transfer.created_event.create_argument {
                if let Some(transfer_data) = create_arg.get("transfer") {
                    if let Some(amt) = transfer_data.get("amount") {
                        amount = amt.as_str().map(|s| s.to_string());
                        log::debug!("     Amount: {}", amt);
                    }
                    if let Some(rcvr) = transfer_data.get("receiver") {
                        receiver = rcvr.as_str().map(|s| s.to_string());
                        log::debug!("     To: {}", rcvr.as_str().unwrap_or("unknown"));
                    }
                }
            }

            // Build exercise command using shared context
            let exercise_command = common::submission::ExerciseCommand {
                exercise_command: common::submission::ExerciseCommandData {
                    template_id: common::consts::TEMPLATE_TRANSFER_INSTRUCTION.to_string(),
                    contract_id: contract_id.clone(),
                    choice: "TransferInstruction_Withdraw".to_string(),
                    choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                        common::accept::ChoiceArguments {
                            extra_args: common::accept::ExtraArgs {
                                context: common::accept::Context {
                                    values: withdraw_context.choice_context_data.values.clone(),
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
            batch_results.push(WithdrawResult {
                success: false, // Will update after submission
                contract_id: contract_id.clone(),
                amount,
                receiver,
                error: None,
            });
        }

        // Submit this batch
        log::debug!("  Submitting batch {}/{}...", batch_num, num_batches);

        let submission_request = common::submission::Submission {
            act_as: vec![params.sender_party.clone()],
            read_as: None,
            command_id: uuid::Uuid::new_v4().to_string(),
            disclosed_contracts: withdraw_context.disclosed_contracts.clone(),
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
        "Summary: Withdrawn: {}, Failed: {}",
        successful_count,
        failed_count
    );

    Ok(WithdrawAllResult {
        successful_count,
        failed_count,
        results,
    })
}
