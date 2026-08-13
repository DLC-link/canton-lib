use crate::distribute;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CsvRecord {
    receiver: String,
    amount: String,
}

pub struct Params {
    pub csv_path: String,
    pub sender: String,
    pub instrument_id: common::transfer::InstrumentId,
    pub ledger_host: String,
    pub registry_url: String,
    pub decentralized_party_id: String,
    // Keycloak authentication
    pub keycloak_client_id: String,
    pub keycloak_username: String,
    pub keycloak_password: String,
    pub keycloak_url: String,
    // Optional reference base for unique transfer IDs
    pub reference_base: Option<String>,
}

/// Process a CSV file of recipients and amounts, distributing tokens using
/// sequential chained transfers.
///
/// This function:
/// 1. Reads the CSV file
/// 2. Validates recipients and amounts
/// 3. Calls distribute which handles UTXO management automatically
///
/// Each transfer uses the change from the previous transfer, eliminating the
/// need for pre-splitting UTXOs.
pub async fn submit_from_csv(params: Params) -> Result<(), String> {
    // Read CSV file
    log::debug!("Reading CSV from: {}", params.csv_path);
    let mut reader = csv::Reader::from_path(&params.csv_path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;

    let mut recipients = Vec::new();
    let mut total_amount = common::decimal::DamlDecimal::ZERO;

    for result in reader.deserialize() {
        let record: CsvRecord = result.map_err(|e| format!("Failed to parse CSV record: {}", e))?;

        // Parse amount to DamlDecimal for validation
        let amount = common::decimal::DamlDecimal::parse(&record.amount)
            .map_err(|e| format!("Invalid amount '{}': {}", record.amount, e))?;
        total_amount += amount;

        recipients.push(distribute::Recipient {
            receiver: record.receiver,
            amount,
        });
    }

    if recipients.is_empty() {
        return Err("No recipients found in CSV file".to_string());
    }

    log::debug!(
        "Found {} recipients, total amount: {}",
        recipients.len(),
        total_amount
    );

    // Distribute tokens using sequential chained transfers
    // This will automatically authenticate and fetch UTXOs and chain the transfers
    let result = distribute::submit(distribute::Params {
        recipients,
        sender: params.sender,
        instrument_id: params.instrument_id,
        ledger_host: params.ledger_host,
        registry_url: params.registry_url,
        decentralized_party_id: params.decentralized_party_id,
        keycloak_client_id: params.keycloak_client_id,
        keycloak_username: params.keycloak_username,
        keycloak_password: params.keycloak_password,
        keycloak_url: params.keycloak_url,
        reference_base: params.reference_base,
        on_transfer_complete: None,
    })
    .await?;

    log::debug!("Batch distribution complete!");
    log::debug!("Successful transfers: {}", result.successful_count);
    if result.failed_count > 0 {
        log::debug!("Failed transfers: {}", result.failed_count);
        for transfer_result in result.results.iter().filter(|r| !r.success) {
            log::debug!(
                "Failed transfer: {} to {} ({}): {}",
                transfer_result.amount,
                transfer_result.receiver,
                transfer_result.transfer_index + 1,
                transfer_result
                    .error
                    .as_ref()
                    .unwrap_or(&"Unknown error".to_string())
            );
        }
    }

    Ok(())
}

#[cfg(test)]
// Each test holds SERIAL_LOCK across its awaits on purpose: the lock spans
// the whole test body so tests never interleave, and `#[tokio::test]` runs
// on a single-thread runtime, so a held std guard cannot deadlock it.
#[allow(clippy::await_holding_lock)]
mod integration_tests {
    //! Live integration test for CSV batch distribution. Shared setup, the
    //! required env vars, and the run command are documented in
    //! [`crate::test_utils`].

    use super::*;
    use crate::test_utils::{
        IntegrationTestState, SERIAL_LOCK, balance, dec, distribute_reference, offer_amount,
        offer_cid, outgoing_with_reference,
    };
    use common::decimal::DamlDecimal;
    use std::io::Write;

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_batch_from_csv() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;
        let mut p2 = state.client_for(&state.party_2).await;
        let party_1_balance = balance(&mut p1).await;
        let party_2_balance = balance(&mut p2).await;

        // Party 1 distributes four amounts to party 2 from a CSV file.
        let receiver = &state.party_2;
        let csv_content = format!(
            "receiver,amount\n\
            {receiver},0.0001\n\
            {receiver},0.005\n\
            {receiver},1.5\n\
            {receiver},0.001\n"
        );
        let csv_path = std::env::temp_dir().join(format!("batch-test-{test_uuid}.csv"));
        let mut file = std::fs::File::create(&csv_path).expect("failed to create temp CSV file");
        file.write_all(csv_content.as_bytes())
            .expect("failed to write CSV content");

        submit_from_csv(Params {
            csv_path: csv_path.to_string_lossy().into_owned(),
            sender: state.party_1.clone(),
            instrument_id: state.instrument.clone(),
            ledger_host: state.ledger_host.clone(),
            registry_url: state.registry_url.clone(),
            decentralized_party_id: state.instrument.admin.clone(),
            keycloak_client_id: state.keycloak.client_id.clone(),
            keycloak_username: state.keycloak.username.clone(),
            keycloak_password: state.keycloak.password.clone(),
            keycloak_url: state.keycloak.url.clone(),
            reference_base: Some(test_uuid.clone()),
        })
        .await
        .expect("batch distribution failed");
        std::fs::remove_file(&csv_path).ok();

        // All four offers go to the same receiver, so they share one
        // encoded reference.
        let reference = distribute_reference(&test_uuid, &state.party_1, &state.party_2);
        let outgoing = outgoing_with_reference(&mut p1, &reference).await;
        let mut amounts: Vec<DamlDecimal> = outgoing.iter().map(offer_amount).collect();
        amounts.sort();
        assert_eq!(
            amounts,
            vec![dec("0.0001"), dec("0.001"), dec("0.005"), dec("1.5")],
            "batch should create one offer per CSV row"
        );

        // Cancel every offer, restoring both balances.
        for offer in &outgoing {
            p1.cancel_offer(offer_cid(offer))
                .await
                .expect("cancel of batch offer failed");
        }
        let outgoing = outgoing_with_reference(&mut p1, &reference).await;
        assert!(outgoing.is_empty(), "cancelled offers should leave the ACS");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after cancelling all offers"
        );
        assert_eq!(
            balance(&mut p2).await,
            party_2_balance,
            "party 2 balance should be unchanged"
        );
    }
}
