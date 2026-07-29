//! A convenience wrapper for working with one token.
//!
//! [`TokenClient`] stores the configuration that otherwise repeats on every
//! call — ledger host, registry URL, instrument, acting party, Keycloak
//! credentials — and forwards to the free functions in the operation modules.
//! It is purely additive: everything it does can also be done directly with
//! those modules.

use crate::{
    accept, active_contracts, cancel_offers, consolidate, distribute, reject, split, transfer,
    utils,
};
use common::decimal::DamlDecimal;
use common::transfer::InstrumentId;
use std::collections::HashMap;
use std::ops::Add;

/// Keycloak credentials for the acting party.
#[derive(Debug, Clone)]
pub struct KeycloakConfig {
    pub client_id: String,
    pub username: String,
    pub password: String,
    /// Token endpoint URL, e.g. `keycloak::login::password_url(host, realm)`.
    pub url: String,
}

/// Everything the client needs to operate on one token as one party.
#[derive(Debug, Clone)]
pub struct TokenClientConfig {
    /// Ledger API host of your participant node.
    pub ledger_host: String,
    /// DA Registry Utility URL, e.g. `registry::consts::DEVNET_REGISTRY_URL.to_string()`.
    pub registry_url: String,
    /// The token: its admin (decentralized) party and instrument id.
    pub instrument: InstrumentId,
    /// The acting party (sender/receiver of operations).
    pub party: String,
    pub keycloak: KeycloakConfig,
}

/// A client bound to one token, one party, and one participant.
///
/// Construct with [`TokenClient::connect`]; the access token is refreshed
/// automatically between calls.
pub struct TokenClient {
    config: TokenClientConfig,
    token: transfer::TokenState,
}

impl TokenClient {
    /// Authenticate with Keycloak and return a ready client.
    pub async fn connect(config: TokenClientConfig) -> Result<Self, String> {
        let token = transfer::TokenState::new(
            config.keycloak.username.clone(),
            config.keycloak.password.clone(),
            config.keycloak.client_id.clone(),
            config.keycloak.url.clone(),
        )
        .await?;
        Ok(Self { config, token })
    }

    /// The token this client operates on.
    pub fn instrument(&self) -> &InstrumentId {
        &self.config.instrument
    }

    /// The acting party.
    pub fn party(&self) -> &str {
        &self.config.party
    }

    /// The token's admin party, used as the registry's decentralized party id
    /// and as `expectedAdmin` on factory choices.
    fn admin(&self) -> String {
        self.config.instrument.admin.clone()
    }

    async fn fresh_token(&mut self) -> Result<String, String> {
        self.token.get_fresh_token().await
    }

    /// The party's active, unlocked holdings (UTXOs) of this token.
    pub async fn holdings(&mut self) -> Result<Vec<ledger::models::JsActiveContract>, String> {
        let access_token = self.fresh_token().await?;
        active_contracts::get(active_contracts::Params {
            ledger_host: self.config.ledger_host.clone(),
            party: self.config.party.clone(),
            access_token,
            instrument_id: self.config.instrument.clone(),
        })
        .await
    }

    /// Total spendable balance: the sum over all unlocked holdings.
    pub async fn balance(&mut self) -> Result<DamlDecimal, String> {
        let holdings = self.holdings().await?;
        Ok(holdings
            .iter()
            .filter_map(utils::extract_amount)
            .fold(DamlDecimal::ZERO, |acc, amount| acc.add(amount)))
    }

    /// Number of UTXOs currently held (Canton soft limit: 10 per party per token).
    pub async fn utxo_count(&mut self) -> Result<usize, String> {
        Ok(self.holdings().await?.len())
    }

    /// Send tokens to a receiver as a two-phase transfer (they must accept).
    /// `reference` is stored in the transfer's metadata when given.
    pub async fn send(
        &mut self,
        receiver: String,
        amount: DamlDecimal,
        reference: Option<String>,
    ) -> Result<(), String> {
        let access_token = self.fresh_token().await?;

        let meta = reference.map(|r| {
            let mut values = HashMap::new();
            values.insert(
                "splice.lfdecentralizedtrust.org/reason".to_string(),
                String::new(),
            );
            values.insert("splice.lfdecentralizedtrust.org/reference".to_string(), r);
            common::transfer::Meta {
                values: Some(values),
            }
        });

        transfer::submit(transfer::Params {
            transfer: common::transfer::Transfer {
                sender: self.config.party.clone(),
                receiver,
                amount,
                instrument_id: self.config.instrument.clone(),
                requested_at: chrono::Utc::now().to_rfc3339(),
                execute_before: (chrono::Utc::now() + chrono::Duration::hours(168)).to_rfc3339(),
                input_holding_cids: None,
                meta,
            },
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Accept one incoming transfer offer by contract id.
    pub async fn accept(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        accept::submit(accept::Params {
            transfer_offer_contract_id: transfer_offer_cid,
            receiver_party: self.config.party.clone(),
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Accept all pending incoming transfers of this token.
    pub async fn accept_all(&mut self) -> Result<accept::AcceptAllResult, String> {
        accept::accept_all(accept::AcceptAllParams {
            receiver_party: self.config.party.clone(),
            instrument_id: self.config.instrument.clone(),
            ledger_host: self.config.ledger_host.clone(),
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
            keycloak_client_id: self.config.keycloak.client_id.clone(),
            keycloak_username: self.config.keycloak.username.clone(),
            keycloak_password: self.config.keycloak.password.clone(),
            keycloak_url: self.config.keycloak.url.clone(),
        })
        .await
    }

    /// Reject one incoming transfer offer by contract id.
    pub async fn reject(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        reject::submit(reject::Params {
            transfer_offer_contract_id: transfer_offer_cid,
            receiver_party: self.config.party.clone(),
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Cancel (withdraw) one outgoing transfer offer by contract id.
    pub async fn cancel_offer(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        cancel_offers::submit(cancel_offers::Params {
            transfer_offer_contract_id: transfer_offer_cid,
            sender_party: self.config.party.clone(),
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Cancel all pending outgoing transfers of this token.
    pub async fn cancel_all_offers(&mut self) -> Result<cancel_offers::WithdrawAllResult, String> {
        cancel_offers::withdraw_all(cancel_offers::WithdrawAllParams {
            sender_party: self.config.party.clone(),
            instrument_id: self.config.instrument.clone(),
            ledger_host: self.config.ledger_host.clone(),
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
            keycloak_client_id: self.config.keycloak.client_id.clone(),
            keycloak_username: self.config.keycloak.username.clone(),
            keycloak_password: self.config.keycloak.password.clone(),
            keycloak_url: self.config.keycloak.url.clone(),
        })
        .await
    }

    /// Pending incoming transfer offers of this token (party is receiver).
    pub async fn incoming_offers(
        &mut self,
    ) -> Result<Vec<ledger::models::JsActiveContract>, String> {
        let access_token = self.fresh_token().await?;
        utils::fetch_incoming_transfers(
            self.config.ledger_host.clone(),
            self.config.party.clone(),
            access_token,
            self.config.instrument.clone(),
        )
        .await
    }

    /// Pending outgoing transfer offers of this token (party is sender).
    pub async fn outgoing_offers(
        &mut self,
    ) -> Result<Vec<ledger::models::JsActiveContract>, String> {
        let access_token = self.fresh_token().await?;
        utils::fetch_outgoing_transfers(
            self.config.ledger_host.clone(),
            self.config.party.clone(),
            access_token,
            self.config.instrument.clone(),
        )
        .await
    }

    /// Merge all holdings into a single UTXO. Returns the resulting holding CIDs.
    pub async fn consolidate(&mut self) -> Result<Vec<String>, String> {
        let access_token = self.fresh_token().await?;
        consolidate::consolidate_utxos(consolidate::ConsolidateParams {
            party: self.config.party.clone(),
            instrument_id: self.config.instrument.clone(),
            input_holding_cids: None,
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Consolidate only when the UTXO count reaches `threshold`.
    pub async fn check_and_consolidate(
        &mut self,
        threshold: usize,
    ) -> Result<consolidate::ConsolidationResult, String> {
        let access_token = self.fresh_token().await?;
        consolidate::check_and_consolidate(consolidate::CheckConsolidateParams {
            party: self.config.party.clone(),
            instrument_id: self.config.instrument.clone(),
            threshold,
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Split all holdings into the given amounts plus change.
    pub async fn split(&mut self, amounts: Vec<DamlDecimal>) -> Result<split::SplitResult, String> {
        let input_holding_cids: Vec<String> = self
            .holdings()
            .await?
            .into_iter()
            .map(|c| c.created_event.contract_id)
            .collect();
        let access_token = self.fresh_token().await?;
        split::submit(split::Params {
            party: self.config.party.clone(),
            amounts,
            instrument_id: self.config.instrument.clone(),
            input_holding_cids,
            ledger_host: self.config.ledger_host.clone(),
            access_token,
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
        })
        .await
    }

    /// Distribute to many recipients with sequential chained transfers.
    pub async fn distribute(
        &mut self,
        recipients: Vec<distribute::Recipient>,
        reference_base: Option<String>,
    ) -> Result<transfer::SequentialChainedResult, String> {
        distribute::submit(distribute::Params {
            recipients,
            sender: self.config.party.clone(),
            instrument_id: self.config.instrument.clone(),
            ledger_host: self.config.ledger_host.clone(),
            registry_url: self.config.registry_url.clone(),
            decentralized_party_id: self.admin(),
            keycloak_client_id: self.config.keycloak.client_id.clone(),
            keycloak_username: self.config.keycloak.username.clone(),
            keycloak_password: self.config.keycloak.password.clone(),
            keycloak_url: self.config.keycloak.url.clone(),
            reference_base,
            on_transfer_complete: None,
        })
        .await
    }
}
