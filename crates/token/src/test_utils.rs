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
//! Optional, and needed whenever the two parties authenticate separately:
//! `KEYCLOAK_CLIENT_ID_2`, `KEYCLOAK_USERNAME_2`, `KEYCLOAK_PASSWORD_2`.
//! A participant grants a token access to its own party only, so on a
//! deployment where the parties have separate Keycloak users, reusing
//! party 1's token for party 2 fails every party-2 read with gRPC
//! `PERMISSION_DENIED`. Each variable falls back to its party-1 twin, so
//! an environment where the two really do share an account needs none of
//! them. The token endpoint is shared either way.
//!
//! The registry URL is pinned to devnet, and both parties are on the same
//! participant.
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

/// All configuration the integration tests need: the env-provided
/// values plus the hardcoded devnet registry URL.
pub(crate) struct IntegrationTestState {
    pub(crate) party_1: String,
    pub(crate) party_2: String,
    pub(crate) instrument: InstrumentId,
    pub(crate) ledger_host: String,
    pub(crate) registry_url: String,
    pub(crate) keycloak: KeycloakConfig,
    /// Party 2's credentials. Equal to [`Self::keycloak`] unless the
    /// environment supplies the `_2` variables.
    pub(crate) keycloak_2: KeycloakConfig,
}

impl IntegrationTestState {
    pub(crate) fn from_env() -> Self {
        dotenvy::dotenv().ok();
        let var = |name: &str| {
            env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
        };
        let url = var("KEYCLOAK_URL");
        let keycloak = KeycloakConfig {
            client_id: var("KEYCLOAK_CLIENT_ID"),
            username: var("KEYCLOAK_USERNAME"),
            password: var("KEYCLOAK_PASSWORD"),
            url: url.clone(),
        };
        // Fall back to party 1's value per field, so an environment where
        // both parties share one Keycloak account needs no `_2` variables.
        let or_party_1 =
            |name: &str, fallback: &str| env::var(name).unwrap_or_else(|_| fallback.to_string());
        let keycloak_2 = KeycloakConfig {
            client_id: or_party_1("KEYCLOAK_CLIENT_ID_2", &keycloak.client_id),
            username: or_party_1("KEYCLOAK_USERNAME_2", &keycloak.username),
            password: or_party_1("KEYCLOAK_PASSWORD_2", &keycloak.password),
            url,
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
            keycloak,
            keycloak_2,
        }
    }

    /// The credentials that can act as `party`.
    ///
    /// A participant authorises a token for one party, so building party
    /// 2's client with party 1's credentials makes every party-2 read fail
    /// with `PERMISSION_DENIED` rather than with anything that names the
    /// real cause.
    pub(crate) fn keycloak_for(&self, party: &str) -> &KeycloakConfig {
        if party == self.party_2 {
            &self.keycloak_2
        } else {
            &self.keycloak
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
            keycloak: self.keycloak_for(party).clone(),
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
        .get(crate::utils::REFERENCE_META_KEY)?
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

/// Stubs for the crate's unit tests, as against the live tests above.
///
/// An instruction operation crosses two HTTP boundaries and queries no
/// active-contract set: it fetches a choice context from the registry, then
/// submits to the ledger. Serving both lets a unit test read back the whole
/// submission, including the choice name and the acting parties.
#[cfg(test)]
pub(crate) mod stub {
    use wiremock::matchers::{method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub(crate) const SUBMIT_PATH: &str = "/v2/commands/submit-and-wait-for-transaction";

    /// What the run actually sent, read back off the stub.
    pub(crate) struct Submitted {
        pub(crate) choice: String,
        pub(crate) actors: Vec<String>,
        pub(crate) act_as: Vec<String>,
        pub(crate) context_path: String,
    }

    /// A server answering every transfer-instruction choice-context route and
    /// the ledger submit endpoint.
    pub(crate) async fn instruction_server() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r".*/choice-contexts/[a-z]+$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choiceContextData": { "values": {} },
                "disclosedContracts": []
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path(SUBMIT_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        server
    }

    /// Read back the one command the run submitted, and the choice-context
    /// route it fetched to build it.
    pub(crate) async fn submitted(server: &MockServer) -> Submitted {
        let requests = server
            .received_requests()
            .await
            .expect("wiremock records requests by default");

        let context_path = requests
            .iter()
            .find(|r| r.url.path().contains("/choice-contexts/"))
            .expect("the operation must fetch a choice context")
            .url
            .path()
            .to_string();

        let submit = requests
            .iter()
            .find(|r| r.url.path() == SUBMIT_PATH)
            .expect("the operation must submit to the ledger");
        let body: serde_json::Value =
            serde_json::from_slice(&submit.body).expect("the submission must be JSON");

        // `wait_for_transaction` wraps the whole `Submission` in an outer
        // `commands` field, so the command list sits two levels down.
        let submission = &body["commands"];
        let command = &submission["commands"][0]["ExerciseCommand"];

        let strings = |value: &serde_json::Value| -> Vec<String> {
            value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|i| i.as_str().unwrap_or_default().to_string())
                        .collect()
                })
                .unwrap_or_default()
        };

        Submitted {
            choice: command["choice"]
                .as_str()
                .expect("a command must name its choice")
                .to_string(),
            actors: strings(&command["choiceArgument"]["actors"]),
            act_as: strings(&submission["actAs"]),
            context_path,
        }
    }
}
