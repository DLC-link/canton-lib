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
    /// Which registry API this client's calls use. `TokenStandardVersion::V1`
    /// reproduces the behaviour of every release before 0.7.0. There is no
    /// struct-level default: adding this field means every `TokenClientConfig`
    /// literal must name it.
    pub version: common::TokenStandardVersion,
}

/// Parameters for [`TokenClient::send`].
#[derive(Debug, Clone)]
pub struct SendParams {
    /// The receiving party.
    pub receiver: String,
    pub amount: DamlDecimal,
    /// Stored in the transfer's metadata when given.
    pub reference: Option<String>,
    /// Bounds how long the offer stays acceptable; one week when `None`.
    pub execute_before: Option<chrono::Duration>,
    /// Holdings that fund the transfer; the registry selects them when `None`.
    pub input_holding_cids: Option<Vec<String>>,
}

/// Parameters for [`TokenClient::split`].
#[derive(Debug, Clone)]
pub struct SplitParams {
    /// The amounts to split off; the remainder becomes change.
    pub amounts: Vec<DamlDecimal>,
    /// Holdings to split; all of the party's holdings when `None`.
    pub input_holding_cids: Option<Vec<String>>,
}

/// Parameters for [`TokenClient::distribute`].
pub struct DistributeParams {
    pub recipients: Vec<distribute::Recipient>,
    /// Base for each transfer's unique reference id when given.
    pub reference_base: Option<String>,
    /// Called after each transfer completes, with its success or failure.
    pub on_transfer_complete: Option<Box<transfer::TransferResultCallback>>,
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

    /// The acting party as a basic account: no provider, empty id.
    ///
    /// The V2 entry points take accounts. This client is bound to one party,
    /// so every account it needs is that party's basic account.
    fn account(&self) -> common::transfer::v2::Account {
        Self::account_for(&self.config)
    }

    /// [`Self::account`] without a connected client, so a unit test can reach it.
    pub(crate) fn account_for(config: &TokenClientConfig) -> common::transfer::v2::Account {
        common::transfer::v2::Account::basic(config.party.clone())
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
            account: None,
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
    /// See [`SendParams`] for the optional fields and their defaults.
    pub async fn send(&mut self, params: SendParams) -> Result<(), String> {
        let access_token = self.fresh_token().await?;

        let meta = params.reference.map(|r| {
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

        let requested_at = chrono::Utc::now().to_rfc3339();
        let execute_before = (chrono::Utc::now()
            + params
                .execute_before
                .unwrap_or(chrono::Duration::hours(168)))
        .to_rfc3339();

        match self.config.version {
            common::TokenStandardVersion::V1 => {
                transfer::submit(transfer::Params {
                    transfer: common::transfer::Transfer {
                        sender: self.config.party.clone(),
                        receiver: params.receiver,
                        amount: params.amount,
                        instrument_id: self.config.instrument.clone(),
                        requested_at,
                        execute_before,
                        input_holding_cids: params.input_holding_cids,
                        meta,
                    },
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
            common::TokenStandardVersion::V2 => {
                transfer::v2::submit(transfer::v2::Params {
                    transfer: common::transfer::v2::Transfer {
                        sender: self.account(),
                        receiver: common::transfer::v2::Account::basic(params.receiver),
                        amount: params.amount,
                        instrument_id: self.config.instrument.clone(),
                        requested_at,
                        execute_before,
                        input_holding_cids: params.input_holding_cids,
                        meta,
                    },
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Accept one incoming transfer offer by contract id.
    pub async fn accept(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                accept::v2::submit(accept::v2::Params {
                    transfer_instruction_id: transfer_offer_cid,
                    receiver_party: self.config.party.clone(),
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Accept all pending incoming transfers of this token.
    pub async fn accept_all(&mut self) -> Result<accept::AcceptAllResult, String> {
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                accept::v2::accept_all(accept::v2::AcceptAllParams {
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
        }
    }

    /// Reject one incoming transfer offer by contract id.
    pub async fn reject(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                reject::v2::submit(reject::v2::Params {
                    transfer_instruction_id: transfer_offer_cid,
                    receiver_party: self.config.party.clone(),
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Cancel (withdraw) one outgoing transfer offer by contract id.
    pub async fn cancel_offer(&mut self, transfer_offer_cid: String) -> Result<(), String> {
        let access_token = self.fresh_token().await?;
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                cancel_offers::v2::submit(cancel_offers::v2::Params {
                    transfer_instruction_id: transfer_offer_cid,
                    sender_party: self.config.party.clone(),
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Cancel all pending outgoing transfers of this token.
    pub async fn cancel_all_offers(&mut self) -> Result<cancel_offers::WithdrawAllResult, String> {
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                cancel_offers::v2::withdraw_all(cancel_offers::v2::WithdrawAllParams {
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
        }
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
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                consolidate::v2::consolidate_utxos(consolidate::v2::ConsolidateParams {
                    account: self.account(),
                    instrument_id: self.config.instrument.clone(),
                    input_holding_cids: None,
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Consolidate only when the UTXO count reaches `threshold`.
    pub async fn check_and_consolidate(
        &mut self,
        threshold: usize,
    ) -> Result<consolidate::ConsolidationResult, String> {
        let access_token = self.fresh_token().await?;
        match self.config.version {
            common::TokenStandardVersion::V1 => {
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
            common::TokenStandardVersion::V2 => {
                consolidate::v2::check_and_consolidate(consolidate::v2::CheckConsolidateParams {
                    account: self.account(),
                    instrument_id: self.config.instrument.clone(),
                    threshold,
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Split holdings into the given amounts plus change.
    /// See [`SplitParams`] for the optional fields and their defaults.
    ///
    /// Each amount is split in its own ledger transaction, so the operation
    /// can complete partially; the [`split::Error`] carries the holdings
    /// created before the failure.
    pub async fn split(&mut self, params: SplitParams) -> Result<split::SplitResult, split::Error> {
        match self.config.version {
            common::TokenStandardVersion::V1 => {
                let input_holding_cids = match params.input_holding_cids {
                    Some(cids) => cids,
                    None => self
                        .holdings()
                        .await?
                        .into_iter()
                        .map(|c| c.created_event.contract_id)
                        .collect(),
                };
                let access_token = self.fresh_token().await?;
                split::submit(split::Params {
                    party: self.config.party.clone(),
                    amounts: params.amounts,
                    instrument_id: self.config.instrument.clone(),
                    input_holding_cids,
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
            common::TokenStandardVersion::V2 => {
                let account = self.account();
                let access_token = self.fresh_token().await?;
                // Not `self.holdings()`: that passes `account: None`, and
                // handing label-blind cids to a basic-account self-transfer
                // would sweep a labelled holding into it. Decision 11.
                let input_holding_cids = match params.input_holding_cids {
                    Some(cids) => cids,
                    None => active_contracts::get(active_contracts::Params {
                        ledger_host: self.config.ledger_host.clone(),
                        party: self.config.party.clone(),
                        access_token: access_token.clone(),
                        instrument_id: self.config.instrument.clone(),
                        account: Some(account.clone()),
                    })
                    .await?
                    .into_iter()
                    .map(|c| c.created_event.contract_id)
                    .collect(),
                };
                split::v2::submit(split::v2::Params {
                    account,
                    amounts: params.amounts,
                    instrument_id: self.config.instrument.clone(),
                    input_holding_cids,
                    ledger_host: self.config.ledger_host.clone(),
                    access_token,
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                })
                .await
            }
        }
    }

    /// Distribute to many recipients with sequential chained transfers.
    /// See [`DistributeParams`] for the optional fields and their defaults.
    pub async fn distribute(
        &mut self,
        params: DistributeParams,
    ) -> Result<transfer::SequentialChainedResult, String> {
        match self.config.version {
            common::TokenStandardVersion::V1 => {
                distribute::submit(distribute::Params {
                    recipients: params.recipients,
                    sender: self.config.party.clone(),
                    instrument_id: self.config.instrument.clone(),
                    ledger_host: self.config.ledger_host.clone(),
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                    keycloak_client_id: self.config.keycloak.client_id.clone(),
                    keycloak_username: self.config.keycloak.username.clone(),
                    keycloak_password: self.config.keycloak.password.clone(),
                    keycloak_url: self.config.keycloak.url.clone(),
                    reference_base: params.reference_base,
                    on_transfer_complete: params.on_transfer_complete,
                })
                .await
            }
            common::TokenStandardVersion::V2 => {
                let recipients = params
                    .recipients
                    .into_iter()
                    .map(|r| distribute::v2::Recipient {
                        receiver: common::transfer::v2::Account::basic(r.receiver),
                        amount: r.amount,
                    })
                    .collect();
                distribute::v2::submit(distribute::v2::Params {
                    recipients,
                    sender: self.account(),
                    instrument_id: self.config.instrument.clone(),
                    ledger_host: self.config.ledger_host.clone(),
                    registry_url: self.config.registry_url.clone(),
                    decentralized_party_id: self.admin(),
                    keycloak_client_id: self.config.keycloak.client_id.clone(),
                    keycloak_username: self.config.keycloak.username.clone(),
                    keycloak_password: self.config.keycloak.password.clone(),
                    keycloak_url: self.config.keycloak.url.clone(),
                    reference_base: params.reference_base,
                    on_transfer_complete: params.on_transfer_complete,
                })
                .await
            }
        }
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use wiremock::matchers::{method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const PARTY: &str = "alice::1220ab";
    const ADMIN: &str = "admin::1220ef";

    // --- canned third-party responses ---

    fn keycloak_token() -> serde_json::Value {
        serde_json::json!({
            "access_token": "test-access-token",
            "refresh_token": "test-refresh-token",
            "expires_in": 3600
        })
    }

    fn factory_response() -> serde_json::Value {
        serde_json::json!({
            "factoryId": "00factory",
            "transferKind": "offer",
            "choiceContext": {
                "choiceContextData": { "values": {} },
                "disclosedContracts": []
            }
        })
    }

    fn choice_context() -> serde_json::Value {
        serde_json::json!({
            "choiceContextData": { "values": {} },
            "disclosedContracts": []
        })
    }

    /// A server that answers Keycloak and every registry route, and 404s the
    /// rest.
    ///
    /// One `POST /token` mock serves both grants: `TokenState::new` sets its
    /// expiry with `checked_sub` (`transfer.rs:106-110`), so the first
    /// `get_fresh_token` always takes the refresh path, and `refresh` posts to
    /// the same URL as `password` (`keycloak/src/login.rs:226`).
    ///
    /// The ledger submission is left unmocked and therefore fails. That is
    /// deliberate: these tests assert what reached the registry, not what the
    /// ledger did with it.
    async fn stub() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(keycloak_token()))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex(r".*/transfer-factory$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(factory_response()))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex(r".*/choice-contexts/[a-z]+$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(choice_context()))
            .mount(&server)
            .await;

        server
    }

    fn config(server: &MockServer, version: common::TokenStandardVersion) -> TokenClientConfig {
        TokenClientConfig {
            ledger_host: server.uri(),
            registry_url: server.uri(),
            instrument: InstrumentId {
                admin: ADMIN.to_string(),
                id: "CBTC".to_string(),
            },
            party: PARTY.to_string(),
            keycloak: KeycloakConfig {
                client_id: "test-client".to_string(),
                username: "test-user".to_string(),
                password: "test-password".to_string(),
                url: format!("{}/token", server.uri()),
            },
            version,
        }
    }

    async fn client(server: &MockServer, version: common::TokenStandardVersion) -> TokenClient {
        TokenClient::connect(config(server, version))
            .await
            .expect("connect against the stub")
    }

    /// The path and JSON body of the one registry request a call made.
    ///
    /// Asserting there was exactly one also catches a method that skipped the
    /// registry, and one that called it twice.
    async fn registry_call(server: &MockServer) -> (String, serde_json::Value) {
        let requests = server
            .received_requests()
            .await
            .expect("wiremock records requests by default");
        let registry: Vec<_> = requests
            .iter()
            .filter(|r| r.url.path().contains("/token-standard/"))
            .collect();
        assert_eq!(
            registry.len(),
            1,
            "expected exactly one registry call, saw {}",
            registry.len()
        );
        (
            registry[0].url.path().to_string(),
            serde_json::from_slice(&registry[0].body).expect("registry body must be JSON"),
        )
    }

    fn dec(s: &str) -> DamlDecimal {
        DamlDecimal::parse(s).expect("invalid decimal literal")
    }

    fn send_params() -> SendParams {
        SendParams {
            receiver: "bob::1220cd".to_string(),
            amount: dec("1.0"),
            reference: None,
            execute_before: None,
            // Supplied so `send` reaches its dispatch without querying the ACS,
            // which is a websocket the stub cannot serve.
            input_holding_cids: Some(vec!["00abc".to_string()]),
        }
    }

    fn split_params() -> SplitParams {
        SplitParams {
            amounts: vec![dec("1.0")],
            input_holding_cids: Some(vec!["00abc".to_string()]),
        }
    }

    // --- the config field itself ---

    #[test]
    fn version_defaults_to_v1() {
        assert_eq!(
            common::TokenStandardVersion::default(),
            common::TokenStandardVersion::V1,
            "a caller that does not name a version must keep V1 behaviour"
        );
    }

    #[tokio::test]
    async fn account_lifts_the_config_party_to_a_basic_account() {
        let server = stub().await;
        assert_eq!(
            TokenClient::account_for(&config(&server, common::TokenStandardVersion::V2)),
            common::transfer::v2::Account::basic(PARTY)
        );
    }

    // --- the factory path: send and split ---

    #[tokio::test]
    async fn send_dispatches_to_the_v1_factory() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V1)
            .await
            .send(send_params())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(
            path.ends_with("/registry/transfer-instruction/v1/transfer-factory"),
            "{path}"
        );
        let args = &body["choiceArguments"];
        assert_eq!(args["expectedAdmin"], serde_json::json!(ADMIN));
        assert_eq!(args["transfer"]["sender"], serde_json::json!(PARTY));
        assert!(args["actors"].is_null(), "V1 has no actors field");
    }

    #[tokio::test]
    async fn send_dispatches_to_the_v2_factory() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V2)
            .await
            .send(send_params())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(
            path.ends_with("/registry/transfer-instruction/v2/transfer-factory"),
            "{path}"
        );
        let args = &body["choiceArguments"];
        assert!(args["expectedAdmin"].is_null(), "V2 drops expectedAdmin");
        assert_eq!(
            args["actors"],
            serde_json::json!([PARTY]),
            "the factory accepts exactly the sender's owner"
        );
        assert_eq!(
            args["transfer"]["sender"],
            serde_json::json!({ "owner": PARTY, "provider": null, "id": "" }),
            "spec 6.1: owner and provider serialize explicitly, null included"
        );
        assert_eq!(
            args["transfer"]["receiver"],
            serde_json::json!({ "owner": "bob::1220cd", "provider": null, "id": "" })
        );
    }

    #[tokio::test]
    async fn split_dispatches_to_the_v1_factory() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V1)
            .await
            .split(split_params())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(path.contains("/transfer-instruction/v1/"), "{path}");
        assert_eq!(
            body["choiceArguments"]["transfer"]["sender"],
            serde_json::json!(PARTY)
        );
    }

    #[tokio::test]
    async fn split_dispatches_to_the_v2_factory_as_a_self_transfer() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V2)
            .await
            .split(split_params())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(path.contains("/transfer-instruction/v2/"), "{path}");
        let transfer = &body["choiceArguments"]["transfer"];
        assert_eq!(
            transfer["sender"], transfer["receiver"],
            "the registry detects a merge-split by comparing whole accounts"
        );
    }

    // --- the instruction path: accept, reject, cancel_offer ---
    //
    // V1 and V2 send an identical request body to these routes, so the URL is
    // the whole signal. The body assertion pins that sameness.

    #[tokio::test]
    async fn accept_dispatches_to_the_v1_accept_route() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V1)
            .await
            .accept("00instruction".to_string())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v1/00instruction/choice-contexts/accept"),
            "{path}"
        );
        assert_eq!(body, serde_json::json!({ "meta": { "values": "" } }));
    }

    #[tokio::test]
    async fn accept_dispatches_to_the_v2_accept_route() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V2)
            .await
            .accept("00instruction".to_string())
            .await;

        let (path, body) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v2/00instruction/choice-contexts/accept"),
            "{path}"
        );
        assert_eq!(body, serde_json::json!({ "meta": { "values": "" } }));
    }

    #[tokio::test]
    async fn reject_dispatches_to_the_v1_reject_route() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V1)
            .await
            .reject("00instruction".to_string())
            .await;

        let (path, _) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v1/00instruction/choice-contexts/reject"),
            "{path}"
        );
    }

    #[tokio::test]
    async fn reject_dispatches_to_the_v2_reject_route() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V2)
            .await
            .reject("00instruction".to_string())
            .await;

        let (path, _) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v2/00instruction/choice-contexts/reject"),
            "{path}"
        );
    }

    #[tokio::test]
    async fn cancel_offer_v1_fetches_the_accept_route() {
        // Not a typo, and not a defect this change introduces. V1 fetches the
        // accept context and then exercises withdraw against it; follow-up 1
        // files that. This test pins the behaviour so the follow-up's fix is
        // a visible change rather than a silent one.
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V1)
            .await
            .cancel_offer("00instruction".to_string())
            .await;

        let (path, _) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v1/00instruction/choice-contexts/accept"),
            "{path}"
        );
    }

    #[tokio::test]
    async fn cancel_offer_v2_fetches_the_withdraw_route() {
        let server = stub().await;
        let _ = client(&server, common::TokenStandardVersion::V2)
            .await
            .cancel_offer("00instruction".to_string())
            .await;

        let (path, _) = registry_call(&server).await;
        assert!(
            path.ends_with("/transfer-instruction/v2/00instruction/choice-contexts/withdraw"),
            "{path}"
        );
    }
}

#[cfg(test)]
// Each test holds SERIAL_LOCK across its awaits on purpose: the lock spans
// the whole test body so tests never interleave, and `#[tokio::test]` runs
// on a single-thread runtime, so a held std guard cannot deadlock it.
#[allow(clippy::await_holding_lock)]
mod integration_tests {
    //! Live integration tests for [`TokenClient`], the crate's outermost
    //! boundary. Shared setup, the required env vars, and the run command
    //! are documented in [`crate::test_utils`].

    use super::*;
    use crate::distribute::Recipient;
    use crate::test_utils::{
        IntegrationTestState, SERIAL_LOCK, balance, dec, distribute_reference,
        incoming_with_reference, offer_amount, offer_cid, outgoing_with_reference,
    };

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_transfer_offer_accept() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;
        let mut p2 = state.client_for(&state.party_2).await;

        // Party 1 offers 1 to party 2.
        let party_1_balance = balance(&mut p1).await;
        p1.send(SendParams {
            receiver: state.party_2.clone(),
            amount: dec("1"),
            reference: Some(test_uuid.clone()),
            execute_before: None,
            input_holding_cids: None,
        })
        .await
        .expect("party 1 send failed");
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(outgoing.len(), 1, "party 1 should have one outgoing offer");

        // Party 2 accepts it.
        let incoming = incoming_with_reference(&mut p2, &test_uuid).await;
        assert_eq!(incoming.len(), 1, "party 2 should have one incoming offer");
        let party_2_balance = balance(&mut p2).await;
        p2.accept(offer_cid(&incoming[0]))
            .await
            .expect("party 2 accept failed");
        assert_eq!(
            balance(&mut p2).await,
            party_2_balance + dec("1"),
            "party 2 balance should grow by 1 after accepting"
        );

        // Party 2 offers 1 back to party 1.
        p2.send(SendParams {
            receiver: state.party_1.clone(),
            amount: dec("1"),
            reference: Some(test_uuid.clone()),
            execute_before: None,
            input_holding_cids: None,
        })
        .await
        .expect("party 2 send failed");
        let outgoing = outgoing_with_reference(&mut p2, &test_uuid).await;
        assert_eq!(outgoing.len(), 1, "party 2 should have one outgoing offer");

        // Party 1 accepts it, restoring both balances.
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance - dec("1"),
            "party 1 balance should be down 1 while its transfer is pending"
        );
        let incoming = incoming_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(incoming.len(), 1, "party 1 should have one incoming offer");
        p1.accept(offer_cid(&incoming[0]))
            .await
            .expect("party 1 accept failed");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after the round trip"
        );
        let incoming = incoming_with_reference(&mut p1, &test_uuid).await;
        assert!(incoming.is_empty(), "no incoming offers should remain");
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_transfer_accept_all() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;
        let mut p2 = state.client_for(&state.party_2).await;

        // The "all" calls below act on every pending offer, not only this
        // test's, so cancel any leftovers from earlier failed runs first to
        // make the counts and balance deltas exact.
        p1.cancel_all_offers()
            .await
            .expect("setup cancel_all_offers failed");

        // Party 1 offers 1 and 1.1 to party 2.
        let party_1_balance = balance(&mut p1).await;
        for amount in ["1", "1.1"] {
            p1.send(SendParams {
                receiver: state.party_2.clone(),
                amount: dec(amount),
                reference: Some(test_uuid.clone()),
                execute_before: None,
                input_holding_cids: None,
            })
            .await
            .unwrap_or_else(|e| panic!("party 1 send of {amount} failed: {e}"));
        }

        // Party 2 accepts both in one call.
        let party_2_balance = balance(&mut p2).await;
        let incoming = incoming_with_reference(&mut p2, &test_uuid).await;
        assert_eq!(incoming.len(), 2, "party 2 should have two incoming offers");
        let result = p2.accept_all().await.expect("accept_all failed");
        assert_eq!(result.failed_count, 0, "accept_all should fail no offer");
        assert_eq!(
            result.successful_count, 2,
            "accept_all should accept exactly the two offers"
        );
        assert_eq!(
            balance(&mut p2).await,
            party_2_balance + dec("2.1"),
            "party 2 balance should grow by 2.1 after accepting all"
        );
        let incoming = incoming_with_reference(&mut p2, &test_uuid).await;
        assert!(incoming.is_empty(), "no incoming offers should remain");

        // Party 2 offers 2.1 back; party 1 accepts, restoring both balances.
        p2.send(SendParams {
            receiver: state.party_1.clone(),
            amount: dec("2.1"),
            reference: Some(test_uuid.clone()),
            execute_before: None,
            input_holding_cids: None,
        })
        .await
        .expect("party 2 send failed");
        let incoming = incoming_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(incoming.len(), 1, "party 1 should have one incoming offer");
        p1.accept(offer_cid(&incoming[0]))
            .await
            .expect("party 1 accept failed");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after the round trip"
        );
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_transfer_offer_cancel_reject() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;
        let mut p2 = state.client_for(&state.party_2).await;

        // Party 1 offers 1.24, then cancels its own offer.
        let party_1_balance = balance(&mut p1).await;
        p1.send(SendParams {
            receiver: state.party_2.clone(),
            amount: dec("1.24"),
            reference: Some(test_uuid.clone()),
            execute_before: None,
            input_holding_cids: None,
        })
        .await
        .expect("party 1 send of 1.24 failed");
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(outgoing.len(), 1, "party 1 should have one outgoing offer");
        p1.cancel_offer(offer_cid(&outgoing[0]))
            .await
            .expect("party 1 cancel failed");
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert!(outgoing.is_empty(), "cancelled offer should leave the ACS");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after cancel"
        );

        // Party 1 offers 1.25; party 2 rejects it.
        p1.send(SendParams {
            receiver: state.party_2.clone(),
            amount: dec("1.25"),
            reference: Some(test_uuid.clone()),
            execute_before: None,
            input_holding_cids: None,
        })
        .await
        .expect("party 1 send of 1.25 failed");
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(outgoing.len(), 1, "party 1 should have one outgoing offer");

        let party_2_balance = balance(&mut p2).await;
        let incoming = incoming_with_reference(&mut p2, &test_uuid).await;
        assert_eq!(incoming.len(), 1, "party 2 should have one incoming offer");
        p2.reject(offer_cid(&incoming[0]))
            .await
            .expect("party 2 reject failed");
        let incoming = incoming_with_reference(&mut p2, &test_uuid).await;
        assert!(incoming.is_empty(), "rejected offer should leave the ACS");
        assert_eq!(
            balance(&mut p2).await,
            party_2_balance,
            "party 2 balance should be unchanged after reject"
        );

        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert!(outgoing.is_empty(), "no outgoing offers should remain");
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_cancel_all() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;

        // cancel_all_offers acts on every pending offer, not only this
        // test's, so cancel any leftovers from earlier failed runs first to
        // make the counts and balance checks exact.
        p1.cancel_all_offers()
            .await
            .expect("setup cancel_all_offers failed");

        // Party 1 offers 1 and 1.1 to party 2.
        let party_1_balance = balance(&mut p1).await;
        for amount in ["1", "1.1"] {
            p1.send(SendParams {
                receiver: state.party_2.clone(),
                amount: dec(amount),
                reference: Some(test_uuid.clone()),
                execute_before: None,
                input_holding_cids: None,
            })
            .await
            .unwrap_or_else(|e| panic!("party 1 send of {amount} failed: {e}"));
        }
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert_eq!(outgoing.len(), 2, "party 1 should have two outgoing offers");

        // Party 1 cancels both in one call.
        let result = p1
            .cancel_all_offers()
            .await
            .expect("cancel_all_offers failed");
        assert_eq!(
            result.failed_count, 0,
            "cancel_all_offers should fail no offer"
        );
        assert_eq!(
            result.successful_count, 2,
            "cancel_all_offers should cancel exactly the two offers"
        );
        let outgoing = outgoing_with_reference(&mut p1, &test_uuid).await;
        assert!(outgoing.is_empty(), "cancelled offers should leave the ACS");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after cancelling all offers"
        );
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_distribute() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;

        let party_1_balance = balance(&mut p1).await;
        let recipients = ["1", "2", "3"]
            .into_iter()
            .map(|amount| Recipient {
                receiver: state.party_2.clone(),
                amount: dec(amount),
            })
            .collect();
        p1.distribute(DistributeParams {
            recipients,
            reference_base: Some(test_uuid.clone()),
            on_transfer_complete: None,
        })
        .await
        .expect("distribute failed");

        // All three offers share one encoded reference: the sender and the
        // receiver are the same for each of them.
        let reference = distribute_reference(&test_uuid, &state.party_1, &state.party_2);
        let outgoing = outgoing_with_reference(&mut p1, &reference).await;
        let mut amounts: Vec<DamlDecimal> = outgoing.iter().map(offer_amount).collect();
        amounts.sort();
        assert_eq!(
            amounts,
            vec![dec("1"), dec("2"), dec("3")],
            "distribute should create offers of exactly 1, 2 and 3"
        );

        for offer in &outgoing {
            p1.cancel_offer(offer_cid(offer))
                .await
                .expect("cancel of distribute offer failed");
        }
        let outgoing = outgoing_with_reference(&mut p1, &reference).await;
        assert!(outgoing.is_empty(), "cancelled offers should leave the ACS");
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "party 1 balance should be restored after cancelling all offers"
        );
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_utxo_count() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let mut p1 = state.client_for(&state.party_1).await;

        let count = p1.utxo_count().await.expect("utxo_count failed");
        let holdings = p1.holdings().await.expect("holdings failed");
        assert_eq!(
            count,
            holdings.len(),
            "utxo_count should match the number of unlocked holdings"
        );
        assert!(
            count >= 1,
            "party 1 should hold at least one UTXO to fund the other tests"
        );
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_check_and_consolidate() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let mut p1 = state.client_for(&state.party_1).await;
        let party_1_balance = balance(&mut p1).await;

        // Below the threshold nothing happens.
        let count = p1.utxo_count().await.expect("utxo_count failed");
        let result = p1
            .check_and_consolidate(count + 1)
            .await
            .expect("check_and_consolidate below threshold failed");
        assert!(
            !result.consolidated,
            "should not consolidate below the threshold"
        );
        assert_eq!(result.utxos_before, count);
        assert_eq!(result.utxos_after, count);

        // Make sure there are at least two UTXOs, then consolidate at the
        // threshold.
        if count < 2 {
            assert!(
                party_1_balance > dec("1"),
                "party 1 needs more than 1 to split off a second UTXO"
            );
            p1.split(SplitParams {
                amounts: vec![dec("1")],
                input_holding_cids: None,
            })
            .await
            .expect("setup split failed");
        }
        let result = p1
            .check_and_consolidate(2)
            .await
            .expect("check_and_consolidate at threshold failed");
        assert!(result.consolidated, "should consolidate at the threshold");
        assert!(result.utxos_before >= 2);
        assert_eq!(
            result.utxos_after, 1,
            "all holdings should merge into one UTXO"
        );
        assert_eq!(
            p1.utxo_count().await.expect("utxo_count failed"),
            1,
            "the ACS should hold a single UTXO after consolidation"
        );
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "consolidation should preserve the balance"
        );
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_split() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let mut p1 = state.client_for(&state.party_1).await;

        let party_1_balance = balance(&mut p1).await;
        let amounts = vec![dec("1"), dec("2"), dec("0.5")];
        assert!(
            party_1_balance > dec("3.5"),
            "party 1 needs more than 3.5 to split with change"
        );

        let result = p1
            .split(SplitParams {
                amounts: amounts.clone(),
                input_holding_cids: None,
            })
            .await
            .expect("split failed");
        assert_eq!(
            result.output_holding_cids.len(),
            amounts.len(),
            "split should produce one output UTXO per requested amount"
        );
        assert!(
            !result.change_holding_cids.is_empty(),
            "split should leave change"
        );

        // Each output CID is a live holding of exactly the requested amount;
        // the outputs come back in the order of the requested amounts.
        let holdings = p1.holdings().await.expect("holdings failed");
        for (cid, amount) in result.output_holding_cids.iter().zip(&amounts) {
            let holding = holdings
                .iter()
                .find(|h| &h.created_event.contract_id == cid)
                .expect("split output holding not found in the ACS");
            assert_eq!(
                utils::extract_amount(holding),
                Some(*amount),
                "split output holding should carry the requested amount"
            );
        }
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "split should preserve the balance"
        );

        // Merge the pieces back so the wallet keeps its shape for other tests.
        p1.consolidate().await.expect("cleanup consolidate failed");
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_split_total() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let mut p1 = state.client_for(&state.party_1).await;

        let party_1_balance = balance(&mut p1).await;
        assert!(
            party_1_balance > dec("1"),
            "party 1 needs more than 1 to split the total in two"
        );
        // Both amounts are passed explicitly so they sum to the whole
        // balance and the final split consumes its input exactly, leaving
        // no change. Passing only `1` would split off the same two UTXOs
        // but via the change path, which would not cover the exact split.
        let amounts = vec![dec("1"), party_1_balance - dec("1")];

        let result = p1
            .split(SplitParams {
                amounts: amounts.clone(),
                input_holding_cids: None,
            })
            .await
            .expect("split failed");
        assert_eq!(
            result.output_holding_cids.len(),
            amounts.len(),
            "split should produce one output UTXO per requested amount"
        );
        assert!(
            result.change_holding_cids.is_empty(),
            "an exact split should leave no change"
        );

        // The ACS should hold exactly the two outputs, each carrying its
        // requested amount, and nothing else.
        let holdings = p1.holdings().await.expect("holdings failed");
        assert_eq!(
            holdings.len(),
            amounts.len(),
            "the ACS should hold exactly the split outputs"
        );
        for (cid, amount) in result.output_holding_cids.iter().zip(&amounts) {
            let holding = holdings
                .iter()
                .find(|h| &h.created_event.contract_id == cid)
                .expect("split output holding not found in the ACS");
            assert_eq!(
                utils::extract_amount(holding),
                Some(*amount),
                "split output holding should carry the requested amount"
            );
        }
        assert_eq!(
            balance(&mut p1).await,
            party_1_balance,
            "split should preserve the balance"
        );

        // Merge the pieces back so the wallet keeps its shape for other tests.
        p1.consolidate().await.expect("cleanup consolidate failed");
    }
}
