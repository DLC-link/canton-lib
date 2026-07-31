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
    /// `execute_before` bounds how long the offer stays acceptable; it
    /// defaults to one week when `None`.
    pub async fn send(
        &mut self,
        receiver: String,
        amount: DamlDecimal,
        reference: Option<String>,
        execute_before: Option<chrono::Duration>,
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
                execute_before: (chrono::Utc::now()
                    + execute_before.unwrap_or(chrono::Duration::hours(168)))
                .to_rfc3339(),
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

#[cfg(test)]
// Each test holds SERIAL_LOCK across its awaits on purpose: the lock spans
// the whole test body so tests never interleave, and `#[tokio::test]` runs
// on a single-thread runtime, so a held std guard cannot deadlock it.
#[allow(clippy::await_holding_lock)]
mod integration_tests {
    //! Live integration tests for [`TokenClient`], the crate's outermost
    //! boundary. They run against a real participant and the devnet
    //! registry, so they are `#[ignore]`d and excluded from the unit suite.
    //!
    //! Run them with:
    //!
    //! ```text
    //! cargo test --workspace -- --ignored --test-threads=1 integration_
    //! ```
    //!
    //! Required env vars (a `.env` file is loaded when present):
    //! `PARTY_ID_1`, `PARTY_ID_2`, `DECENTRALIZED_PARTY_ID`, `INSTRUMENT_ID`,
    //! `LEDGER_HOST`, `KEYCLOAK_CLIENT_ID`, `KEYCLOAK_USER`,
    //! `KEYCLOAK_PASSWORD`, `KEYCLOAK_URL` (full token endpoint URL).
    //!
    //! The registry URL is pinned to devnet. Both parties share the Keycloak
    //! credentials and the participant.
    //!
    //! Every test tags its transfers with a fresh `test_uuid` reference, so
    //! ACS checks only see contracts from the current run and leftovers from
    //! a failed run are easy to find and cancel. Tests must never run in
    //! parallel: they mutate the same wallets, so each one holds
    //! [`SERIAL_LOCK`] for its whole body. A test that succeeds leaves both
    //! wallet balances exactly as it found them.

    use super::*;
    use crate::distribute::Recipient;
    use ledger::models::JsActiveContract;
    use std::env;
    use std::sync::Mutex;

    static SERIAL_LOCK: Mutex<()> = Mutex::new(());

    const REFERENCE_META_KEY: &str = "splice.lfdecentralizedtrust.org/reference";

    /// All configuration the integration tests need: the env-provided
    /// values plus the hardcoded devnet registry URL.
    struct IntegrationTestState {
        party_1: String,
        party_2: String,
        instrument: InstrumentId,
        ledger_host: String,
        registry_url: String,
        keycloak: KeycloakConfig,
    }

    impl IntegrationTestState {
        fn from_env() -> Self {
            dotenvy::dotenv().ok();
            let var = |name: &str| {
                env::var(name)
                    .unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
            };
            Self {
                party_1: var("PARTY_ID_1"),
                party_2: var("PARTY_ID_2"),
                instrument: InstrumentId {
                    admin: var("DECENTRALIZED_PARTY_ID"),
                    id: var("INSTRUMENT_ID"),
                },
                ledger_host: var("LEDGER_HOST"),
                registry_url: registry::consts::DEVNET_REGISTRY_URL.to_string(),
                keycloak: KeycloakConfig {
                    client_id: var("KEYCLOAK_CLIENT_ID"),
                    username: var("KEYCLOAK_USER"),
                    password: var("KEYCLOAK_PASSWORD"),
                    url: var("KEYCLOAK_URL"),
                },
            }
        }

        async fn client_for(&self, party: &str) -> TokenClient {
            TokenClient::connect(TokenClientConfig {
                ledger_host: self.ledger_host.clone(),
                registry_url: self.registry_url.clone(),
                instrument: self.instrument.clone(),
                party: party.to_string(),
                keycloak: self.keycloak.clone(),
            })
            .await
            .expect("failed to connect TokenClient")
        }
    }

    fn dec(s: &str) -> DamlDecimal {
        DamlDecimal::parse(s).expect("invalid decimal literal")
    }

    fn offer_reference(offer: &JsActiveContract) -> Option<&str> {
        offer
            .created_event
            .create_argument
            .as_ref()?
            .get("transfer")?
            .get("meta")?
            .get("values")?
            .get(REFERENCE_META_KEY)?
            .as_str()
    }

    fn offers_with_reference(
        offers: Vec<JsActiveContract>,
        reference: &str,
    ) -> Vec<JsActiveContract> {
        offers
            .into_iter()
            .filter(|offer| offer_reference(offer) == Some(reference))
            .collect()
    }

    fn offer_amount(offer: &JsActiveContract) -> DamlDecimal {
        offer
            .created_event
            .create_argument
            .as_ref()
            .and_then(|arg| arg.get("transfer")?.get("amount")?.as_str())
            .and_then(|s| DamlDecimal::parse(s).ok())
            .expect("offer has no parsable transfer.amount")
    }

    fn offer_cid(offer: &JsActiveContract) -> String {
        offer.created_event.contract_id.clone()
    }

    /// The reference `distribute()` writes on-ledger:
    /// `base64("{base}-{sender}-{receiver}")`. The raw base never appears.
    fn distribute_reference(base: &str, sender: &str, receiver: &str) -> String {
        use base64::{Engine as _, engine::general_purpose};
        general_purpose::STANDARD.encode(format!("{base}-{sender}-{receiver}").as_bytes())
    }

    async fn outgoing_with_reference(
        client: &mut TokenClient,
        reference: &str,
    ) -> Vec<JsActiveContract> {
        let offers = client
            .outgoing_offers()
            .await
            .expect("outgoing_offers failed");
        offers_with_reference(offers, reference)
    }

    async fn incoming_with_reference(
        client: &mut TokenClient,
        reference: &str,
    ) -> Vec<JsActiveContract> {
        let offers = client
            .incoming_offers()
            .await
            .expect("incoming_offers failed");
        offers_with_reference(offers, reference)
    }

    async fn balance(client: &mut TokenClient) -> DamlDecimal {
        client.balance().await.expect("balance failed")
    }

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
        p1.send(
            state.party_2.clone(),
            dec("1"),
            Some(test_uuid.clone()),
            None,
        )
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
        p2.send(
            state.party_1.clone(),
            dec("1"),
            Some(test_uuid.clone()),
            None,
        )
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
    async fn integration_transfer_offer_cancel_reject() {
        let _guard = SERIAL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let state = IntegrationTestState::from_env();
        let test_uuid = uuid::Uuid::new_v4().to_string();
        let mut p1 = state.client_for(&state.party_1).await;
        let mut p2 = state.client_for(&state.party_2).await;

        // Party 1 offers 1.24, then cancels its own offer.
        let party_1_balance = balance(&mut p1).await;
        p1.send(
            state.party_2.clone(),
            dec("1.24"),
            Some(test_uuid.clone()),
            None,
        )
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
        p1.send(
            state.party_2.clone(),
            dec("1.25"),
            Some(test_uuid.clone()),
            None,
        )
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
        p1.distribute(recipients, Some(test_uuid.clone()))
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
}
