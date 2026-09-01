use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Request {
    #[serde(rename = "choiceArguments")]
    pub choice_arguments: common::transfer_factory::ChoiceArguments,
    #[serde(rename = "excludeDebugFields")]
    pub exclude_debug_fields: bool,
}

pub struct Params {
    pub registry_url: String,
    pub decentralized_party_id: String,
    pub request: Request,
}

/// The V1 transfer-factory route.
pub fn factory_url(registry_url: &str, decentralized_party_id: &str) -> String {
    format!(
        "{registry_url}/api/token-standard/v0/registrars/{decentralized_party_id}/registry/transfer-instruction/v1/transfer-factory"
    )
}

pub async fn get(params: Params) -> Result<common::transfer_factory::Response, String> {
    let client = reqwest::Client::new();

    let url = factory_url(&params.registry_url, &params.decentralized_party_id);
    let response = client
        .post(url)
        .json(&params.request)
        .send()
        .await
        .map_err(|e| format!("{}", e))?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(format!(
            "Transfer factory request failed [{}]: {:?}",
            status, body_raw
        ));
    }

    let body: common::transfer_factory::Response =
        serde_json::from_str(&body_raw).map_err(|e| format!("Failed to parse response: {}", e))?;
    Ok(body)
}

/// The V2 transfer-factory route.
///
/// V2 drops `expectedAdmin` from the choice arguments and adds `actors`. The
/// response envelope is identical to V1's, so
/// [`common::transfer_factory::Response`] is reused.
pub mod v2 {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct Request {
        #[serde(rename = "choiceArguments")]
        pub choice_arguments: common::transfer_factory::v2::ChoiceArguments,
        #[serde(rename = "excludeDebugFields")]
        pub exclude_debug_fields: bool,
    }

    pub struct Params {
        pub registry_url: String,
        pub decentralized_party_id: String,
        pub request: Request,
    }

    pub fn factory_url(registry_url: &str, decentralized_party_id: &str) -> String {
        format!(
            "{registry_url}/api/token-standard/v0/registrars/{decentralized_party_id}/registry/transfer-instruction/v2/transfer-factory"
        )
    }

    pub async fn get(params: Params) -> Result<common::transfer_factory::Response, String> {
        let client = reqwest::Client::new();

        let url = factory_url(&params.registry_url, &params.decentralized_party_id);
        let response = client
            .post(url)
            .json(&params.request)
            .send()
            .await
            .map_err(|e| format!("{e}"))?;

        let status = response.status();
        let body_raw = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if !status.is_success() {
            return Err(format!(
                "V2 transfer factory request failed [{status}]: {body_raw:?}"
            ));
        }

        serde_json::from_str(&body_raw).map_err(|e| format!("Failed to parse response: {e}"))
    }
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn v1_factory_url_keeps_its_v1_path() {
        assert_eq!(
            factory_url("https://registry.example", "admin::1220ab"),
            "https://registry.example/api/token-standard/v0/registrars/admin::1220ab/registry/transfer-instruction/v1/transfer-factory"
        );
    }

    #[test]
    fn v2_factory_url_uses_the_v2_path() {
        assert_eq!(
            v2::factory_url("https://registry.example", "admin::1220ab"),
            "https://registry.example/api/token-standard/v0/registrars/admin::1220ab/registry/transfer-instruction/v2/transfer-factory"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    //! Live integration test for the registry transfer-factory endpoint.
    //! It needs these env vars (a `.env` file is loaded when present):
    //! `LEDGER_HOST`, `PARTY_ID_1`, `PARTY_ID_2`, `DECENTRALIZED_PARTY_ID`,
    //! `INSTRUMENT_ID`, `KEYCLOAK_CLIENT_ID`, `KEYCLOAK_USERNAME`,
    //! `KEYCLOAK_PASSWORD`, `KEYCLOAK_URL` (full token endpoint URL).

    use super::*;
    use crate::consts;
    use keycloak::login::{PasswordParams, password};
    use std::collections::HashMap;
    use std::env;
    use std::ops::Add;

    fn var(name: &str) -> String {
        env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_transfer_factory() {
        dotenvy::dotenv().ok();

        let decentralized_party_id = var("DECENTRALIZED_PARTY_ID");
        let instrument_id = var("INSTRUMENT_ID");
        let party_1 = var("PARTY_ID_1");

        let login = password(PasswordParams {
            client_id: var("KEYCLOAK_CLIENT_ID"),
            username: var("KEYCLOAK_USERNAME"),
            password: var("KEYCLOAK_PASSWORD"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak login failed");

        let contracts = get_active_contracts(ACParams {
            ledger_host: var("LEDGER_HOST"),
            party: party_1.clone(),
            access_token: login.access_token,
            instrument_id: instrument_id.clone(),
        })
        .await
        .expect("failed to fetch holdings");

        let input_contract_ids: Vec<String> = contracts
            .iter()
            .map(|contract| contract.created_event.contract_id.clone())
            .collect();

        let mut transfer_meta: HashMap<String, String> = HashMap::new();
        transfer_meta.insert(
            "splice.lfdecentralizedtrust.org/reason".to_string(),
            "".to_string(),
        );

        let params = Params {
            registry_url: consts::DEVNET_REGISTRY_URL.to_string(),
            decentralized_party_id: decentralized_party_id.clone(),
            request: Request {
                choice_arguments: common::transfer_factory::ChoiceArguments {
                    expected_admin: decentralized_party_id.clone(),
                    transfer: common::transfer::Transfer {
                        sender: party_1,
                        receiver: var("PARTY_ID_2"),
                        amount: common::decimal::DamlDecimal::parse("0.02").unwrap(),
                        instrument_id: common::transfer::InstrumentId {
                            admin: decentralized_party_id,
                            id: instrument_id,
                        },
                        requested_at: chrono::Utc::now().to_rfc3339(),
                        execute_before: chrono::Utc::now()
                            .add(chrono::Duration::hours(5))
                            .to_rfc3339(),
                        input_holding_cids: Some(input_contract_ids),
                        meta: Some(common::transfer::Meta {
                            values: Some(transfer_meta),
                        }),
                    },
                    extra_args: common::transfer_factory::ExtraArgs {
                        context: common::transfer_factory::Context {
                            values: HashMap::new(),
                        },
                        meta: common::transfer_factory::Meta {
                            values: common::transfer_factory::MetaValue {},
                        },
                    },
                },
                exclude_debug_fields: true,
            },
        };

        let _result = get(params).await.expect("transfer factory request failed");
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_transfer_factory_v2() {
        dotenvy::dotenv().ok();

        let decentralized_party_id = var("DECENTRALIZED_PARTY_ID");
        let instrument_id = var("INSTRUMENT_ID");
        let party_1 = var("PARTY_ID_1");

        let login = password(PasswordParams {
            client_id: var("KEYCLOAK_CLIENT_ID"),
            username: var("KEYCLOAK_USERNAME"),
            password: var("KEYCLOAK_PASSWORD"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak login failed");

        let contracts = get_active_contracts(ACParams {
            ledger_host: var("LEDGER_HOST"),
            party: party_1.clone(),
            access_token: login.access_token,
            instrument_id: instrument_id.clone(),
        })
        .await
        .expect("failed to fetch holdings");

        let input_contract_ids: Vec<String> = contracts
            .iter()
            .map(|contract| contract.created_event.contract_id.clone())
            .collect();

        let mut transfer_meta: HashMap<String, String> = HashMap::new();
        transfer_meta.insert(
            "splice.lfdecentralizedtrust.org/reason".to_string(),
            "".to_string(),
        );

        // This call settles spec 6.1: `owner` and `provider` are sent as
        // explicit nulls. A rejection here means the encoding changes.
        let result = v2::get(v2::Params {
            registry_url: consts::DEVNET_REGISTRY_URL.to_string(),
            decentralized_party_id: decentralized_party_id.clone(),
            request: v2::Request {
                choice_arguments: common::transfer_factory::v2::ChoiceArguments {
                    transfer: common::transfer::v2::Transfer {
                        sender: common::transfer::v2::Account::basic(party_1.clone()),
                        receiver: common::transfer::v2::Account::basic(var("PARTY_ID_2")),
                        amount: common::decimal::DamlDecimal::parse("0.02").unwrap(),
                        instrument_id: common::transfer::InstrumentId {
                            admin: decentralized_party_id,
                            id: instrument_id,
                        },
                        requested_at: chrono::Utc::now().to_rfc3339(),
                        execute_before: chrono::Utc::now()
                            .add(chrono::Duration::hours(5))
                            .to_rfc3339(),
                        input_holding_cids: Some(input_contract_ids),
                        meta: Some(common::transfer::Meta {
                            values: Some(transfer_meta),
                        }),
                    },
                    actors: vec![party_1],
                    extra_args: common::transfer_factory::ExtraArgs {
                        context: common::transfer_factory::Context {
                            values: HashMap::new(),
                        },
                        meta: common::transfer_factory::Meta {
                            values: common::transfer_factory::MetaValue {},
                        },
                    },
                },
                exclude_debug_fields: true,
            },
        })
        .await
        .expect("V2 transfer factory request failed");

        assert!(
            !result.factory_id.is_empty(),
            "the registry must return a factory contract id"
        );
    }

    #[derive(Debug, Clone)]
    pub struct ACParams {
        pub ledger_host: String,
        pub party: String,
        pub access_token: String,
        pub instrument_id: String,
    }

    async fn get_active_contracts(
        params: ACParams,
    ) -> Result<Vec<ledger::models::JsActiveContract>, String> {
        use ledger::ledger_end;
        use ledger::websocket::active_contracts;

        let ledger_end_result = ledger_end::get(ledger_end::Params {
            access_token: params.access_token.clone(),
            ledger_host: params.ledger_host.clone(),
        })
        .await?;

        let wanted_instrument = params.instrument_id;

        let result = active_contracts::get(active_contracts::Params {
            ledger_host: params.ledger_host,
            party: params.party,
            filter: ledger::common::IdentifierFilter::InterfaceIdentifierFilter(
                ledger::common::InterfaceIdentifierFilter {
                    interface_filter: ledger::common::InterfaceFilter {
                        value: ledger::common::InterfaceFilterValue {
                            interface_id: Some(common::consts::INTERFACE_HOLDING.to_string()),
                            include_interface_view: true,
                            include_created_event_blob: true,
                        },
                    },
                },
            ),
            access_token: params.access_token,
            ledger_end: ledger_end_result.offset,
        })
        .await?;

        let filtered: Vec<ledger::models::JsActiveContract> = result
            .into_iter()
            .filter(|ac| {
                // Note: Filter out the requested instrument's contracts only
                if let Some(view) = ac.created_event.interface_views.clone() {
                    for iv in view {
                        let value = iv.view_value.unwrap_or_default().unwrap_or_default();
                        let instrument_id = value.get("instrumentId").unwrap_or_default();
                        let instrument = instrument_id
                            .get("id")
                            .unwrap_or_default()
                            .as_str()
                            .unwrap_or_default();

                        let lock = value.get("lock").unwrap_or_default();

                        // Note: We have to check the lock value to be null
                        if instrument.eq_ignore_ascii_case(&wanted_instrument)
                            && lock.as_null().is_some()
                        {
                            return true;
                        }
                    }
                }
                false
            })
            .collect();
        Ok(filtered)
    }
}
