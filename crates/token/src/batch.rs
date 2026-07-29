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
mod tests {
    use super::*;
    use keycloak::login::password_url;
    use std::env;
    use std::io::Write;

    #[tokio::test]
    async fn test_batch_from_csv() {
        dotenvy::dotenv().ok();

        let receiver =
            env::var("LIB_TEST_RECEIVER_PARTY_ID").expect("LIB_TEST_RECEIVER_PARTY_ID must be set");

        // Create a temporary CSV file
        let csv_content = format!(
            "receiver,amount\n\
            {receiver},0.0001\n\
            {receiver},0.005\n\
            {receiver},1.5\n\
            {receiver},0.001\n"
        );

        let temp_path = "/tmp/test_batch_distribution.csv";
        let mut file = std::fs::File::create(temp_path).expect("Failed to create temp CSV file");
        file.write_all(csv_content.as_bytes())
            .expect("Failed to write CSV content");

        // Run batch distribution (authentication handled internally)
        let batch_params = Params {
            csv_path: temp_path.to_string(),
            sender: env::var("PARTY_ID").expect("PARTY_ID must be set"),
            instrument_id: common::transfer::InstrumentId {
                admin: common::consts::DEVNET_DECENTRALIZED_PARTY_ID.to_string(),
                id: "CBTC".to_string(),
            },
            ledger_host: env::var("LEDGER_HOST").expect("LEDGER_HOST must be set"),
            registry_url: env::var("REGISTRY_URL").expect("REGISTRY_URL must be set"),
            decentralized_party_id: env::var("DECENTRALIZED_PARTY_ID")
                .expect("DECENTRALIZED_PARTY_ID must be set"),
            keycloak_client_id: env::var("KEYCLOAK_CLIENT_ID")
                .expect("KEYCLOAK_CLIENT_ID must be set"),
            keycloak_username: env::var("KEYCLOAK_USERNAME")
                .expect("KEYCLOAK_USERNAME must be set"),
            keycloak_password: env::var("KEYCLOAK_PASSWORD")
                .expect("KEYCLOAK_PASSWORD must be set"),
            keycloak_url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
            reference_base: Some(format!("batch-test-{}", chrono::Utc::now().timestamp())),
        };

        submit_from_csv(batch_params).await.unwrap();

        // Clean up
        std::fs::remove_file(temp_path).ok();
    }
}
