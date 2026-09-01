//! Shared setup and helpers for the crate's live integration tests.
//!
//! The tests run against a real participant and the devnet registry, so
//! they are `#[ignore]`d and excluded from the unit suite. Run them with:
//!
//! ```text
//! cargo test --workspace -- --ignored --test-threads=1 integration_
//! ```
//!
//! Required env vars (a `.env` file is loaded when present):
//! `PARTY_ID_1`, `PARTY_ID_2`, `DECENTRALIZED_PARTY_ID`, `INSTRUMENT_ID`,
//! `LEDGER_HOST`, `KEYCLOAK_CLIENT_ID`, `KEYCLOAK_USERNAME`,
//! `KEYCLOAK_PASSWORD`, `KEYCLOAK_URL` (full token endpoint URL).
//!
//! The registry URL is pinned to devnet. Both parties share the Keycloak
//! credentials and the participant.
//!
//! Every test that creates transfer offers tags them with a fresh
//! `test_uuid` reference, so ACS checks only see contracts from the
//! current run and leftovers from a failed run are easy to find and
//! cancel. Tests must never run in parallel: they mutate the same
//! wallets, so each one holds [`SERIAL_LOCK`] for its whole body. A test
//! that succeeds leaves both wallet balances exactly as it found them.
//!
//! Since 0.7.0 most of these tests run twice, once per Token Standard
//! version. Each test body is a plain `async fn` taking a
//! [`common::TokenStandardVersion`], called from an
//! `integration_<name>_v1` and an `integration_<name>_v2` wrapper, and it
//! builds its clients with [`IntegrationTestState::client_for_version`].
//! Running the pair back to back is safe because each body restores both
//! wallet balances before it returns. Two tests are deliberately
//! unpaired: `client.rs`'s `integration_utxo_count`, whose two calls do
//! not dispatch on the version, and `registry`'s
//! `integration_transfer_factory_v2`, which has a separate V1 twin
//! because the two routes take different choice arguments.

use crate::client::{KeycloakConfig, TokenClient, TokenClientConfig};
use common::decimal::DamlDecimal;
use common::transfer::InstrumentId;
use ledger::models::JsActiveContract;
use std::env;
use std::sync::Mutex;

pub(crate) static SERIAL_LOCK: Mutex<()> = Mutex::new(());

const REFERENCE_META_KEY: &str = "splice.lfdecentralizedtrust.org/reference";

/// All configuration the integration tests need: the env-provided
/// values plus the hardcoded devnet registry URL.
pub(crate) struct IntegrationTestState {
    pub(crate) party_1: String,
    pub(crate) party_2: String,
    pub(crate) instrument: InstrumentId,
    pub(crate) ledger_host: String,
    pub(crate) registry_url: String,
    pub(crate) keycloak: KeycloakConfig,
}

impl IntegrationTestState {
    pub(crate) fn from_env() -> Self {
        dotenvy::dotenv().ok();
        let var = |name: &str| {
            env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
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
                username: var("KEYCLOAK_USERNAME"),
                password: var("KEYCLOAK_PASSWORD"),
                url: var("KEYCLOAK_URL"),
            },
        }
    }

    /// Log in with the password flow and return a bearer token, for tests
    /// that call module functions directly instead of through
    /// [`TokenClient`].
    pub(crate) async fn access_token(&self) -> String {
        keycloak::login::password(keycloak::login::PasswordParams {
            client_id: self.keycloak.client_id.clone(),
            username: self.keycloak.username.clone(),
            password: self.keycloak.password.clone(),
            url: self.keycloak.url.clone(),
        })
        .await
        .expect("keycloak login failed")
        .access_token
    }

    pub(crate) async fn client_for(&self, party: &str) -> TokenClient {
        self.client_for_version(party, common::TokenStandardVersion::V1)
            .await
    }

    pub(crate) async fn client_for_version(
        &self,
        party: &str,
        version: common::TokenStandardVersion,
    ) -> TokenClient {
        TokenClient::connect(TokenClientConfig {
            ledger_host: self.ledger_host.clone(),
            registry_url: self.registry_url.clone(),
            instrument: self.instrument.clone(),
            party: party.to_string(),
            keycloak: self.keycloak.clone(),
            version,
        })
        .await
        .expect("failed to connect TokenClient")
    }
}

pub(crate) fn dec(s: &str) -> DamlDecimal {
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

fn offers_with_reference(offers: Vec<JsActiveContract>, reference: &str) -> Vec<JsActiveContract> {
    offers
        .into_iter()
        .filter(|offer| offer_reference(offer) == Some(reference))
        .collect()
}

pub(crate) fn offer_amount(offer: &JsActiveContract) -> DamlDecimal {
    offer
        .created_event
        .create_argument
        .as_ref()
        .and_then(|arg| arg.get("transfer")?.get("amount")?.as_str())
        .and_then(|s| DamlDecimal::parse(s).ok())
        .expect("offer has no parsable transfer.amount")
}

pub(crate) fn offer_cid(offer: &JsActiveContract) -> String {
    offer.created_event.contract_id.clone()
}

/// The reference `distribute()` writes on-ledger:
/// `base64("{base}-{sender}-{receiver}")`. The raw base never appears.
pub(crate) fn distribute_reference(base: &str, sender: &str, receiver: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(format!("{base}-{sender}-{receiver}").as_bytes())
}

pub(crate) async fn outgoing_with_reference(
    client: &mut TokenClient,
    reference: &str,
) -> Vec<JsActiveContract> {
    let offers = client
        .outgoing_offers()
        .await
        .expect("outgoing_offers failed");
    offers_with_reference(offers, reference)
}

pub(crate) async fn incoming_with_reference(
    client: &mut TokenClient,
    reference: &str,
) -> Vec<JsActiveContract> {
    let offers = client
        .incoming_offers()
        .await
        .expect("incoming_offers failed");
    offers_with_reference(offers, reference)
}

pub(crate) async fn balance(client: &mut TokenClient) -> DamlDecimal {
    client.balance().await.expect("balance failed")
}
