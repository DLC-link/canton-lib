/// Example: Parallel Delete Executed Transfers
///
/// This example demonstrates how to:
/// 1. Fetch all ExecutedTransfer contracts for a party
/// 2. Call CBTCGovernanceRules_DeleteExecutedTransfers choice in parallel batches
///
/// Run with: cargo run -p examples --bin delete_executed_transfers
///
/// Required environment variables:
/// - KEYCLOAK_HOST, KEYCLOAK_REALM, KEYCLOAK_CLIENT_ID, KEYCLOAK_CLIENT_SECRET
/// - LEDGER_HOST, PARTY_ID
///
/// Additional required environment variables:
/// - CHOICE_CONTRACT_TEMPLATE_ID: Template ID of the CBTCGovernanceRules contract
/// - CHOICE_CONTRACT_ID: Contract ID of the CBTCGovernanceRules contract
/// - DISCLOSED_CONTRACT_TEMPLATE_ID: Template ID for the disclosed contract
/// - DISCLOSED_CONTRACT_BLOB: Base64 blob for the disclosed contract
///
/// Optional environment variables:
/// - MAX_CONTRACTS: Maximum number of contracts to delete (default: unlimited)
/// - NUM_THREADS: Number of parallel threads (default: 4)
/// - CONTRACT_IDS_CSV: Path to CSV file containing contract IDs (skips chain fetch if set)
use std::env;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

const BATCH_SIZE: usize = 50;
const DEFAULT_NUM_THREADS: usize = 8;

#[derive(Clone)]
struct Config {
    party: String,
    ledger_host: String,
    choice_contract_template_id: String,
    choice_contract_id: String,
    decentralized_party_id: String,
    access_token: String,
    disclosed_contract_template_id: String,
    disclosed_contract_blob: String,
}

#[derive(Default)]
struct ThreadResult {
    successful_count: usize,
    failed_count: usize,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    env_logger::init();

    // Load configuration from environment
    let party = env::var("PARTY_ID").expect("PARTY_ID must be set");
    let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");

    let choice_contract_template_id =
        env::var("CHOICE_CONTRACT_TEMPLATE_ID").expect("CHOICE_CONTRACT_TEMPLATE_ID must be set");
    let choice_contract_id =
        env::var("CHOICE_CONTRACT_ID").expect("CHOICE_CONTRACT_ID must be set");
    let max_contracts: Option<usize> = env::var("MAX_CONTRACTS").ok().and_then(|s| s.parse().ok());
    let num_threads: usize = env::var("NUM_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_NUM_THREADS);

    let decentralized_party_id: String = env::var("DECENTRALIZED_PARTY_ID").unwrap_or_else(|_| {
        "cbtc-network::12205af3b949a04776fc48cdcc05a060f6bda2e470632935f375d1049a8546a3b262"
            .to_string()
    });

    // Disclosed contract configuration from environment
    let disclosed_contract_template_id = env::var("DISCLOSED_CONTRACT_TEMPLATE_ID")
        .expect("DISCLOSED_CONTRACT_TEMPLATE_ID must be set");
    let disclosed_contract_blob =
        env::var("DISCLOSED_CONTRACT_BLOB").expect("DISCLOSED_CONTRACT_BLOB must be set");

    // Optional CSV file path for contract IDs
    let contract_ids_csv = env::var("CONTRACT_IDS_CSV").ok();

    // Authenticate using client credentials
    println!("Authenticating...");
    let login_params = keycloak::login::ClientCredentialsParams {
        client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
        client_secret: env::var("KEYCLOAK_CLIENT_SECRET")
            .expect("KEYCLOAK_CLIENT_SECRET must be set"),
        url: keycloak::login::client_credentials_url(
            &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
            &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
        ),
    };

    let auth = keycloak::login::client_credentials(login_params)
        .await
        .map_err(|e| format!("Authentication failed: {}", e))?;

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Parallel Delete Executed Transfers");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Party: {}", party);
    println!("Threads: {}", num_threads);
    println!("Choice: CBTCGovernanceRules_DeleteExecutedTransfers");
    println!("Target Contract: {}", truncate_id(&choice_contract_id));
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Get contract IDs either from CSV or from chain
    let mut transfer_contract_ids: Vec<String> = if let Some(csv_path) = contract_ids_csv {
        // Read contract IDs from CSV file
        println!("Reading contract IDs from CSV: {}", csv_path);
        let csv_content =
            fs::read_to_string(&csv_path).map_err(|e| format!("Failed to read CSV file: {}", e))?;

        let ids: Vec<String> = csv_content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#')) // Skip empty lines and comments
            .map(|line| {
                // Handle CSV with multiple columns - take first column
                line.split(',').next().unwrap_or(line).trim().to_string()
            })
            .collect();

        println!("Loaded {} contract IDs from CSV", ids.len());
        ids
    } else {
        // Fetch from chain
        // Step 1: Get ledger end
        let ledger_end_result = ledger::ledger_end::get(ledger::ledger_end::Params {
            access_token: auth.access_token.clone(),
            ledger_host: ledger_host.clone(),
        })
        .await?;

        // Step 2: Fetch all ExecutedTransfer contracts
        println!("Fetching ExecutedTransfer contracts from chain...");
        let executed_transfers =
            ledger::websocket::active_contracts::get(ledger::websocket::active_contracts::Params {
                ledger_host: ledger_host.clone(),
                party: decentralized_party_id.clone(),
                filter: ledger::common::IdentifierFilter::TemplateIdentifierFilter(
                    ledger::common::TemplateIdentifierFilter {
                        template_filter: ledger::common::TemplateFilter {
                            value: ledger::common::TemplateFilterValue {
                                template_id: Some(
                                    common::consts::TEMPLATE_EXECUTED_TRANSFER.to_string(),
                                ),
                                include_created_event_blob: true,
                            },
                        },
                    },
                ),
                access_token: auth.access_token.clone(),
                ledger_end: ledger_end_result.offset,
            })
            .await?;

        println!(
            "Found {} ExecutedTransfer contracts on chain",
            executed_transfers.len()
        );

        executed_transfers
            .iter()
            .map(|c| c.created_event.contract_id.clone())
            .collect()
    };

    if transfer_contract_ids.is_empty() {
        println!("No contracts found. Nothing to do.");
        return Ok(());
    }

    if let Some(max) = max_contracts
        && transfer_contract_ids.len() > max
    {
        println!("Limiting to {} contracts (MAX_CONTRACTS)", max);
        transfer_contract_ids.truncate(max);
    }

    let total = transfer_contract_ids.len();
    println!();

    // Step 3: Split contracts into chunks for parallel processing
    let chunk_size = (total + num_threads - 1) / num_threads;
    let chunks: Vec<Vec<String>> = transfer_contract_ids
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let actual_threads = chunks.len();
    println!(
        "Processing {} contracts across {} thread(s) ({} per thread, batch size {})...\n",
        total, actual_threads, chunk_size, BATCH_SIZE
    );

    // Create shared config
    let config = Arc::new(Config {
        party: party.clone(),
        ledger_host: ledger_host.clone(),
        choice_contract_template_id,
        choice_contract_id,
        decentralized_party_id,
        access_token: auth.access_token.clone(),
        disclosed_contract_template_id,
        disclosed_contract_blob,
    });

    // Spawn parallel tasks
    let mut handles = Vec::new();
    let results = Arc::new(Mutex::new(Vec::new()));

    for (thread_idx, chunk) in chunks.into_iter().enumerate() {
        let config = Arc::clone(&config);
        let results = Arc::clone(&results);
        let thread_num = thread_idx + 1;

        let handle = tokio::spawn(async move {
            let result = process_chunk(thread_num, actual_threads, chunk, &config).await;
            results.lock().await.push(result);
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.await.map_err(|e| format!("Thread panic: {}", e))?;
    }

    // Aggregate results
    let results = results.lock().await;
    let mut total_successful = 0;
    let mut total_failed = 0;

    for result in results.iter() {
        total_successful += result.successful_count;
        total_failed += result.failed_count;
    }

    // Summary
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Parallel Delete Complete");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total contracts processed: {}", total);
    println!("Successful: {}", total_successful);
    println!("Failed: {}", total_failed);

    if total_failed > 0 {
        return Err(format!("Completed with {} failures", total_failed));
    }

    Ok(())
}

async fn process_chunk(
    thread_num: usize,
    total_threads: usize,
    contract_ids: Vec<String>,
    config: &Config,
) -> ThreadResult {
    let mut result = ThreadResult::default();
    let total_in_chunk = contract_ids.len();
    let num_batches = (total_in_chunk + BATCH_SIZE - 1) / BATCH_SIZE;

    println!(
        "[Thread {}/{}] Starting: {} contracts in {} batch(es)",
        thread_num, total_threads, total_in_chunk, num_batches
    );

    for (batch_idx, batch) in contract_ids.chunks(BATCH_SIZE).enumerate() {
        let batch_num = batch_idx + 1;

        // Build the choice argument
        let choice_argument = serde_json::json!({
            "member": config.party,
            "executedTransferCids": batch
        });

        // Build exercise command
        let exercise_command = common::submission::ExerciseCommand {
            exercise_command: common::submission::ExerciseCommandData {
                template_id: config.choice_contract_template_id.clone(),
                contract_id: config.choice_contract_id.clone(),
                choice: "CBTCGovernanceRules_DeleteExecutedTransfers".to_string(),
                choice_argument: common::submission::ChoiceArgumentsVariations::Generic(
                    choice_argument,
                ),
            },
        };

        let disclosed_contracts = vec![common::transfer::DisclosedContract {
            contract_id: config.choice_contract_id.clone(),
            template_id: config.disclosed_contract_template_id.clone(),
            created_event_blob: config.disclosed_contract_blob.clone(),
            synchronizer_id: "".to_string(),
        }];

        // Submit the command
        let submission_request = common::submission::Submission {
            act_as: vec![config.party.clone()],
            read_as: Some(vec![config.decentralized_party_id.clone()]),
            command_id: uuid::Uuid::new_v4().to_string(),
            disclosed_contracts,
            commands: vec![common::submission::Command::ExerciseCommand(
                exercise_command,
            )],
        };

        match ledger::submit::wait_for_transaction_tree(ledger::submit::Params {
            ledger_host: config.ledger_host.clone(),
            access_token: config.access_token.clone(),
            request: submission_request,
        })
        .await
        {
            Ok(_) => {
                println!(
                    "[Thread {}/{}] Batch {}/{} OK ({} contracts)",
                    thread_num,
                    total_threads,
                    batch_num,
                    num_batches,
                    batch.len()
                );
                result.successful_count += batch.len();
            }
            Err(e) => {
                println!(
                    "[Thread {}/{}] Batch {}/{} FAILED: {}",
                    thread_num, total_threads, batch_num, num_batches, e
                );
                result.failed_count += batch.len();
            }
        }
    }

    println!(
        "[Thread {}/{}] Done: {} successful, {} failed",
        thread_num, total_threads, result.successful_count, result.failed_count
    );

    result
}

fn truncate_id(id: &str) -> String {
    if id.len() > 20 {
        format!("{}...{}", &id[..10], &id[id.len() - 10..])
    } else {
        id.to_string()
    }
}
