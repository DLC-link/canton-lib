/// Example: Parallel Delete Executed Transfers
///
/// This example demonstrates how to:
/// 1. Fetch all ExecutedTransfer contracts for a party
/// 2. Call CBTCGovernanceRules_BatchDeleteExecutedTransfers choice in parallel batches
///
/// Run with: cargo run -p examples --bin delete_executed_transfers
///
/// Required environment variables:
/// - KEYCLOAK_HOST, KEYCLOAK_REALM, KEYCLOAK_CLIENT_ID, KEYCLOAK_CLIENT_SECRET
/// - LEDGER_HOST, PARTY_ID
///
/// Additional required environment variables:
/// - CHOICE_CONTRACT_TEMPLATE_ID: Template ID of the CBTCGovernanceRules contract
///   (use the `#cbtc-governance:...` package-name form so smart-upgrade picks the latest version)
/// - CHOICE_CONTRACT_ID: Contract ID of the CBTCGovernanceRules contract
/// - REGISTRAR_SERVICE_CID: Contract ID of the RegistrarService contract
///
/// Visibility for both the CBTCGovernanceRules and RegistrarService contracts comes from
/// read-as on the decentralized party — no disclosed_contracts entries are sent.
///
/// Optional environment variables:
/// - MAX_CONTRACTS: Maximum number of contracts to delete (default: unlimited)
/// - NUM_THREADS: Number of parallel threads (default: 4)
/// - CONTRACT_IDS_CSV: Path to CSV file containing contract IDs (skips chain fetch if set)
/// - PROCESSED_CSV: Path to CSV file to append successfully processed contract IDs
///   (default: processed_transfers.csv). Created if it does not exist; appended to otherwise.
/// - PACKAGE_ID_PREFERENCE: Comma-separated list of package ID hashes to send as
///   `packageIdSelectionPreference` on every submission. Required when the participant has
///   multiple vetted versions of `cbtc-governance` (or any other package); without it Canton
///   may resolve the package-name `#cbtc-governance:...` reference to an older version that
///   doesn't contain the new choice.
/// - BATCH_SIZE: Number of contract IDs sent per exercise call (default: 50). When a batch
///   contains contracts from heterogeneous stakeholder/package sets, Canton may fail with
///   PACKAGE_SELECTION_FAILED. The script bisects failed batches automatically (split in half
///   on each failure) so a few bad apples don't sink the whole batch — but you can still set
///   this to 1 to skip bisection entirely.
/// - FAILED_CSV: Path to CSV file to append contract IDs that failed individually (default:
///   failed_transfers.csv). Format matches PROCESSED_CSV/CONTRACT_IDS_CSV so it can be
///   re-fed once vetting is fixed.
use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_PROCESSED_CSV: &str = "processed_transfers.csv";
const DEFAULT_FAILED_CSV: &str = "failed_transfers.csv";

const DEFAULT_BATCH_SIZE: usize = 50;
const DEFAULT_NUM_THREADS: usize = 8;

struct Config {
    party: String,
    ledger_host: String,
    choice_contract_template_id: String,
    choice_contract_id: String,
    decentralized_party_id: String,
    access_token: String,
    registrar_service_cid: String,
    package_id_selection_preference: Vec<String>,
    batch_size: usize,
    processed_csv: Mutex<std::fs::File>,
    failed_csv: Mutex<std::fs::File>,
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
    let batch_size: usize = env::var("BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_BATCH_SIZE);

    let decentralized_party_id: String = env::var("DECENTRALIZED_PARTY_ID").unwrap_or_else(|_| {
        "cbtc-network::12205af3b949a04776fc48cdcc05a060f6bda2e470632935f375d1049a8546a3b262"
            .to_string()
    });

    // RegistrarService contract ID — passed as a choice argument; visibility comes from
    // read-as on the decentralized party, so no disclosed_contracts entry is needed.
    let registrar_service_cid =
        env::var("REGISTRAR_SERVICE_CID").expect("REGISTRAR_SERVICE_CID must be set");

    // Pin package-name resolution to specific package hashes. Without this, Canton may pick
    // an older vetted version of the package that doesn't contain the new choice.
    let package_id_selection_preference: Vec<String> = env::var("PACKAGE_ID_PREFERENCE")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Optional CSV file path for contract IDs
    let contract_ids_csv = env::var("CONTRACT_IDS_CSV").ok();

    // Path to write successfully processed contract IDs to.
    let processed_csv_path =
        env::var("PROCESSED_CSV").unwrap_or_else(|_| DEFAULT_PROCESSED_CSV.to_string());
    let failed_csv_path = env::var("FAILED_CSV").unwrap_or_else(|_| DEFAULT_FAILED_CSV.to_string());

    // Authenticate using client credentials
    println!("Authenticating...");
    let login_params = keycloak::login::ClientCredentialsParams {
        client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
        client_secret: env::var("KEYCLOAK_CLIENT_SECRET")
            .expect("KEYCLOAK_CLIENT_SECRET must be set"),
        url: keycloak::login::token_url(
            &format!(
                "{}/auth",
                env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set")
            ),
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
    println!("Choice: CBTCGovernanceRules_BatchDeleteExecutedTransfers");
    println!("Target Contract: {}", truncate_id(&choice_contract_id));
    println!("RegistrarService: {}", truncate_id(&registrar_service_cid));
    if package_id_selection_preference.is_empty() {
        println!(
            "Package preference: <none> (WARNING: Canton may resolve #cbtc-governance to an older version)"
        );
    } else {
        println!(
            "Package preference: {}",
            package_id_selection_preference
                .iter()
                .map(|p| truncate_id(p))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("Processed CSV: {}", processed_csv_path);
    println!("Failed CSV:    {}", failed_csv_path);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Get contract IDs either from CSV or from chain
    let mut transfer_contract_ids: Vec<String> = if let Some(csv_path) = &contract_ids_csv {
        println!("Reading contract IDs from CSV: {}", csv_path);
        let ids = read_id_csv(csv_path)?;
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

    // Skip IDs that are already in the processed or failed CSVs from a previous (possibly
    // crashed) run. Those CSVs are append-only logs of "we tried this already".
    let already_done: HashSet<String> = read_id_csv(&processed_csv_path)
        .unwrap_or_default()
        .into_iter()
        .chain(read_id_csv(&failed_csv_path).unwrap_or_default())
        .collect();
    if !already_done.is_empty() {
        let before = transfer_contract_ids.len();
        transfer_contract_ids.retain(|id| !already_done.contains(id));
        let skipped = before - transfer_contract_ids.len();
        if skipped > 0 {
            println!(
                "Skipping {} ID(s) already present in {} / {}",
                skipped, processed_csv_path, failed_csv_path
            );
        }
    }

    if transfer_contract_ids.is_empty() {
        println!("No contracts to process. Nothing to do.");
        // Still rewrite the input CSV so it ends up clean even on a no-op run.
        if let Some(path) = &contract_ids_csv {
            rewrite_input_csv(path, &already_done)?;
        }
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
    let chunk_size = total.div_ceil(num_threads);
    let chunks: Vec<Vec<String>> = transfer_contract_ids
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let actual_threads = chunks.len();
    println!(
        "Processing {} contracts across {} thread(s) ({} per thread, batch size {})...\n",
        total, actual_threads, chunk_size, batch_size
    );

    // Open processed-IDs CSV in append mode (created if missing).
    let processed_csv_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&processed_csv_path)
        .map_err(|e| format!("Failed to open {}: {}", processed_csv_path, e))?;
    let failed_csv_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&failed_csv_path)
        .map_err(|e| format!("Failed to open {}: {}", failed_csv_path, e))?;

    // Create shared config
    let config = Arc::new(Config {
        party: party.clone(),
        ledger_host: ledger_host.clone(),
        choice_contract_template_id,
        choice_contract_id,
        decentralized_party_id,
        access_token: auth.access_token.clone(),
        registrar_service_cid,
        package_id_selection_preference,
        batch_size,
        processed_csv: Mutex::new(processed_csv_file),
        failed_csv: Mutex::new(failed_csv_file),
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
    println!("Processed IDs appended to: {}", processed_csv_path);
    if total_failed > 0 {
        println!("Failed IDs appended to:    {}", failed_csv_path);
    }

    // Rewrite the input CSV to remove anything we've now processed or failed, so a re-run
    // sees only the work that's still outstanding.
    if let Some(path) = &contract_ids_csv {
        let done: HashSet<String> = read_id_csv(&processed_csv_path)
            .unwrap_or_default()
            .into_iter()
            .chain(read_id_csv(&failed_csv_path).unwrap_or_default())
            .collect();
        rewrite_input_csv(path, &done)?;
    }

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
    let num_batches = total_in_chunk.div_ceil(config.batch_size);

    println!(
        "[Thread {}/{}] Starting: {} contracts in {} batch(es)",
        thread_num, total_threads, total_in_chunk, num_batches
    );

    for (batch_idx, batch) in contract_ids.chunks(config.batch_size).enumerate() {
        let batch_num = batch_idx + 1;
        let label = format!(
            "[Thread {}/{}] Batch {}/{}",
            thread_num, total_threads, batch_num, num_batches
        );
        process_batch_with_bisect(&label, batch, config, &mut result).await;
    }

    println!(
        "[Thread {}/{}] Done: {} successful, {} failed",
        thread_num, total_threads, result.successful_count, result.failed_count
    );

    result
}

async fn submit_batch(config: &Config, batch: &[String]) -> Result<String, String> {
    let choice_argument = serde_json::json!({
        "member": config.party,
        "registrarServiceCid": config.registrar_service_cid,
        "cids": batch,
    });

    let exercise_command = common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: config.choice_contract_template_id.clone(),
            contract_id: config.choice_contract_id.clone(),
            choice: "CBTCGovernanceRules_BatchDeleteExecutedTransfers".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Generic(
                choice_argument,
            ),
        },
    };

    let submission_request = common::submission::Submission {
        act_as: vec![config.party.clone()],
        read_as: Some(vec![config.decentralized_party_id.clone()]),
        command_id: uuid::Uuid::new_v4().to_string(),
        package_id_selection_preference: config.package_id_selection_preference.clone(),
        commands: vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
        ..Default::default()
    };

    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: config.ledger_host.clone(),
        access_token: config.access_token.clone(),
        request: submission_request,
    })
    .await
}

/// Submit `initial_batch`. On failure with >1 contract, split in half and retry each half
/// (depth-first stack-based bisection) so a few bad apples in a large batch don't fail the
/// whole batch — and the bad ones are isolated and written to FAILED_CSV.
async fn process_batch_with_bisect(
    label: &str,
    initial_batch: &[String],
    config: &Config,
    result: &mut ThreadResult,
) {
    // Stack of (sub-batch, depth) pairs; depth is just for log readability.
    let mut stack: Vec<(&[String], usize)> = vec![(initial_batch, 0)];
    while let Some((batch, depth)) = stack.pop() {
        let indent = "  ".repeat(depth);
        match submit_batch(config, batch).await {
            Ok(response_body) => {
                println!(
                    "{}{} OK ({} contracts) -> {}",
                    indent,
                    label,
                    batch.len(),
                    response_body
                );
                if let Err(e) = append_processed_ids(&config.processed_csv, batch).await {
                    eprintln!(
                        "{}{} WARNING: failed to write processed IDs: {}",
                        indent, label, e
                    );
                }
                result.successful_count += batch.len();
            }
            Err(e) if batch.len() > 1 => {
                let mid = batch.len() / 2;
                let (left, right) = (&batch[..mid], &batch[mid..]);
                println!(
                    "{}{} FAILED ({} contracts), bisecting into [{}] + [{}]: {}",
                    indent,
                    label,
                    batch.len(),
                    left.len(),
                    right.len(),
                    e
                );
                // Push right first so left is popped (and tried) first.
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
            Err(e) => {
                let bad = &batch[0];
                println!("{}{} FAILED single contract {}: {}", indent, label, bad, e);
                if let Err(write_err) = append_failed_ids(&config.failed_csv, batch).await {
                    eprintln!(
                        "{}{} WARNING: failed to write failed IDs: {}",
                        indent, label, write_err
                    );
                }
                result.failed_count += 1;
            }
        }
    }
}

async fn append_failed_ids(
    file: &Mutex<std::fs::File>,
    contract_ids: &[String],
) -> std::io::Result<()> {
    append_processed_ids(file, contract_ids).await
}

async fn append_processed_ids(
    file: &Mutex<std::fs::File>,
    contract_ids: &[String],
) -> std::io::Result<()> {
    let mut buf = String::with_capacity(contract_ids.iter().map(|id| id.len() + 1).sum());
    for id in contract_ids {
        buf.push_str(id);
        buf.push('\n');
    }
    let mut guard = file.lock().await;
    guard.write_all(buf.as_bytes())?;
    // Flush so a kill mid-run still leaves the IDs persisted.
    guard.flush()?;
    Ok(())
}

/// Parse a CSV/list file of contract IDs. Each non-empty, non-`#` line is treated as a row;
/// the first comma-separated column is taken (with surrounding quotes stripped). A
/// `contract_id` header row is silently skipped. Returns an empty Vec if the file does not
/// exist (so callers can use this on the processed/failed CSVs without checking first).
fn read_id_csv<P: AsRef<Path>>(path: P) -> Result<Vec<String>, String> {
    let path = path.as_ref();
    let csv_content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Failed to read {}: {}", path.display(), e)),
    };

    Ok(csv_content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            line.split(',')
                .next()
                .unwrap_or(line)
                .trim()
                .trim_matches('"')
                .to_string()
        })
        .filter(|id| {
            !matches!(
                id.to_ascii_lowercase().as_str(),
                "contract_id" | "contractid" | "id" | "contract id"
            )
        })
        .collect())
}

/// Rewrite the input CSV to contain only IDs that are NOT in `done`. Preserves a
/// `contract_id` header line so the file stays in the same shape callers / spreadsheets
/// expect. A temp-file + rename gives us atomic replacement on the same filesystem.
fn rewrite_input_csv<P: AsRef<Path>>(path: P, done: &HashSet<String>) -> Result<(), String> {
    let path = path.as_ref();
    let original = read_id_csv(path)?;
    let remaining: Vec<&String> = original.iter().filter(|id| !done.contains(*id)).collect();

    let tmp_path = path.with_extension("csv.tmp");
    let mut out = fs::File::create(&tmp_path)
        .map_err(|e| format!("Failed to create {}: {}", tmp_path.display(), e))?;
    writeln!(out, "contract_id").map_err(|e| format!("Failed to write header: {}", e))?;
    for id in &remaining {
        writeln!(out, "{}", id).map_err(|e| format!("Failed to write id: {}", e))?;
    }
    out.flush().map_err(|e| format!("Failed to flush: {}", e))?;
    drop(out);
    fs::rename(&tmp_path, path).map_err(|e| {
        format!(
            "Failed to rename {} -> {}: {}",
            tmp_path.display(),
            path.display(),
            e
        )
    })?;

    println!(
        "Rewrote {}: {} -> {} ID(s) remaining",
        path.display(),
        original.len(),
        remaining.len()
    );
    Ok(())
}

fn truncate_id(id: &str) -> String {
    if id.len() > 20 {
        format!("{}...{}", &id[..10], &id[id.len() - 10..])
    } else {
        id.to_string()
    }
}
