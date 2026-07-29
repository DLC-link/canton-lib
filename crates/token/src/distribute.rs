use crate::{active_contracts, transfer};

pub struct Recipient {
    pub receiver: String,
    pub amount: common::decimal::DamlDecimal,
}

pub struct Params {
    pub recipients: Vec<Recipient>,
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
    // Optional reference base for unique transfer IDs (run ID)
    pub reference_base: Option<String>,
    // Optional callback for handling each transfer result
    pub on_transfer_complete: Option<Box<transfer::TransferResultCallback>>,
}

/// Distribute tokens to multiple recipients using sequential chained transfers.
///
/// This function:
/// 1. Authenticates with Keycloak
/// 2. Fetches all available UTXOs once
/// 3. Creates transfers for each recipient
/// 4. Submits transfers sequentially with JWT auto-refresh, chaining change outputs
///
/// Each transfer automatically uses the change from the previous transfer,
/// eliminating the need for UTXO selection or pre-splitting.
///
/// If reference_base is provided, each transfer gets a unique ID:
/// base64(reference_base + sender + receiver) in the meta field.
pub async fn submit(params: Params) -> Result<transfer::SequentialChainedResult, String> {
    log::debug!("Distributing to {} recipients", params.recipients.len());

    // Authenticate with Keycloak
    let mut token_state = transfer::TokenState::new(
        params.keycloak_username,
        params.keycloak_password,
        params.keycloak_client_id.clone(),
        params.keycloak_url.clone(),
    )
    .await
    .map_err(|e| format!("Failed to initialize token state: {}", e))?;

    let access_token = token_state.get_fresh_token().await?;

    // Fetch all active contracts once
    let contracts = active_contracts::get(active_contracts::Params {
        ledger_host: params.ledger_host.clone(),
        party: params.sender.clone(),
        access_token: access_token.clone(),
    })
    .await?;

    if contracts.is_empty() {
        return Err("No UTXOs available for transfers".to_string());
    }

    // Collect all UTXO contract IDs as initial holdings
    let initial_holding_cids: Vec<String> = contracts
        .iter()
        .map(|c| c.created_event.contract_id.clone())
        .collect();

    log::debug!("Using {} initial UTXOs", initial_holding_cids.len());

    // Generate run reference if reference_base is provided
    if let Some(ref reference_base) = params.reference_base {
        log::debug!("Using reference base: {}", reference_base);
    }

    // Convert recipients to the format expected by submit_sequential_chained
    let recipients: Vec<transfer::Recipient> = params
        .recipients
        .into_iter()
        .map(|r| transfer::Recipient {
            receiver: r.receiver,
            amount: r.amount,
            reference: None,
        })
        .collect();

    // Submit all transfers sequentially with JWT auto-refresh, chaining the change outputs
    transfer::submit_sequential_chained(
        transfer::SequentialChainedParams {
            recipients,
            sender: params.sender,
            instrument_id: params.instrument_id,
            initial_holding_cids,
            ledger_host: params.ledger_host,
            registry_url: params.registry_url,
            decentralized_party_id: params.decentralized_party_id,
            reference_base: params.reference_base,
            on_transfer_complete: params.on_transfer_complete,
            registry_response: None,
        },
        &mut token_state,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycloak::login::password_url;
    use std::env;

    #[tokio::test]
    async fn test_distribute() {
        dotenvy::dotenv().ok();

        let recipients = vec![
            Recipient {
                receiver: env::var("LIB_TEST_RECEIVER_PARTY_ID")
                    .expect("LIB_TEST_RECEIVER_PARTY_ID must be set"),
                amount: common::decimal::DamlDecimal::parse("0.01").unwrap(),
            },
            Recipient {
                receiver: env::var("LIB_TEST_RECEIVER_PARTY_ID")
                    .expect("LIB_TEST_RECEIVER_PARTY_ID must be set"),
                amount: common::decimal::DamlDecimal::parse("0.01").unwrap(),
            },
        ];

        let params = Params {
            recipients,
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
            reference_base: Some("test-distribute-run-001".to_string()),
            on_transfer_complete: None,
        };

        let result = submit(params).await.unwrap();

        log::debug!("Distribution complete!");
        log::debug!("Successful: {}", result.successful_count);
        log::debug!("Failed: {}", result.failed_count);

        assert!(
            result.successful_count > 0,
            "At least one transfer should succeed"
        );
    }
}
