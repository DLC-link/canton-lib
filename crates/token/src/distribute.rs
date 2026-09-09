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
        instrument_id: params.instrument_id.clone(),
        account: None,
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

/// Token Standard V2 form of the distribute entry point.
pub mod v2 {
    use crate::transfer;
    use crate::utils::require_owner;

    // `Debug` so a `Result<Vec<Recipient>, _>` can be unwrapped in a test.
    #[derive(Debug)]
    pub struct Recipient {
        pub receiver: common::transfer::v2::Account,
        pub amount: common::decimal::DamlDecimal,
    }

    pub struct Params {
        pub recipients: Vec<Recipient>,
        pub sender: common::transfer::v2::Account,
        pub instrument_id: common::transfer::InstrumentId,
        pub ledger_host: String,
        pub registry_url: String,
        pub decentralized_party_id: String,
        pub keycloak_client_id: String,
        pub keycloak_username: String,
        pub keycloak_password: String,
        pub keycloak_url: String,
        pub reference_base: Option<String>,
        pub on_transfer_complete: Option<Box<transfer::TransferResultCallback>>,
    }

    /// Distribute to many recipients with sequential chained transfers.
    ///
    /// Authenticates, fetches the sender's holdings once, then chains each
    /// transfer's change into the next.
    pub async fn submit(params: Params) -> Result<transfer::SequentialChainedResult, String> {
        let sender = require_owner(&params.sender, "sender")?;

        log::debug!("Distributing to {} recipients", params.recipients.len());

        let mut token_state = transfer::TokenState::new(
            params.keycloak_username,
            params.keycloak_password,
            params.keycloak_client_id.clone(),
            params.keycloak_url.clone(),
        )
        .await
        .map_err(|e| format!("Failed to initialize token state: {}", e))?;

        let access_token = token_state.get_fresh_token().await?;

        let contracts = crate::active_contracts::get(crate::active_contracts::Params {
            ledger_host: params.ledger_host.clone(),
            party: sender.clone(),
            access_token,
            instrument_id: params.instrument_id.clone(),
            // Distribute funds only from the sender's own account.
            account: Some(params.sender.clone()),
        })
        .await?;

        if contracts.is_empty() {
            return Err("No UTXOs available for transfers".to_string());
        }

        let initial_holding_cids: Vec<String> = contracts
            .iter()
            .map(|c| c.created_event.contract_id.clone())
            .collect();

        log::debug!("Using {} initial UTXOs", initial_holding_cids.len());

        let recipients: Vec<transfer::v2::Recipient> = params
            .recipients
            .into_iter()
            .map(|r| transfer::v2::Recipient {
                receiver: r.receiver,
                amount: r.amount,
                reference: None,
            })
            .collect();

        transfer::v2::submit_sequential_chained(
            transfer::v2::SequentialChainedParams {
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
}

#[cfg(test)]
mod v2_tests {
    use super::*;

    #[tokio::test]
    async fn v2_submit_rejects_a_special_sender_before_authenticating() {
        let err = v2::submit(v2::Params {
            recipients: vec![v2::Recipient {
                receiver: common::transfer::v2::Account::basic("bob::1220cd"),
                amount: common::decimal::DamlDecimal::parse("1.0").unwrap(),
            }],
            sender: common::transfer::v2::Account {
                owner: None,
                provider: None,
                id: String::new(),
            },
            instrument_id: common::transfer::InstrumentId {
                admin: "admin::1220ef".to_string(),
                id: "CBTC".to_string(),
            },
            // Unroutable on purpose: if the guard let the call through, the
            // error would name a Keycloak failure instead.
            ledger_host: "http://127.0.0.1:1".to_string(),
            registry_url: "http://127.0.0.1:1".to_string(),
            decentralized_party_id: "admin::1220ef".to_string(),
            keycloak_client_id: "unused".to_string(),
            keycloak_username: "unused".to_string(),
            keycloak_password: "unused".to_string(),
            keycloak_url: "http://127.0.0.1:1".to_string(),
            reference_base: None,
            on_transfer_complete: None,
        })
        .await
        .unwrap_err();

        assert!(
            err.contains("sender"),
            "the guard must name the parameter and precede the login, got {err}"
        );
    }
}
