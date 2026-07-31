use crate::common;
use canton_api_client::apis::default_api as canton_api;
use canton_api_client::models;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub party: String,
    pub filter: common::IdentifierFilter,
    pub access_token: String,
    pub ledger_end: i64,
    pub unknown_contract_entry_handler: Option<fn(contract_entry: models::JsContractEntry)>,
}

pub async fn get_by_party(params: Params) -> Result<Vec<models::JsActiveContract>, String> {
    let cumulative_vec: Vec<common::CumulativeFilter> = vec![common::CumulativeFilter {
        identifier_filter: params.filter,
    }];

    let mut filters_by_party: HashMap<String, common::Filters> = HashMap::new();
    filters_by_party.insert(
        params.party.clone(),
        common::Filters {
            cumulative: Some(cumulative_vec),
        },
    );

    let request = common::GetActiveContractsRequest {
        filter: Some(common::TransactionFilter {
            filters_by_party,
            filters_for_any_party: None,
        }),
        verbose: false,
        active_at_offset: params.ledger_end,
    };

    let canton_client = crate::client::Client::new(params.access_token, params.ledger_host);
    let result = match canton_api::post_v2_state_active_contracts(
        &canton_client.configuration,
        common::convert_get_active_contracts_request(request),
        None,
        None,
    )
    .await
    {
        Ok(r) => r,
        Err(error) => {
            return Err(format!("post_v2_state_active_contracts failed: {}", error));
        }
    };

    let mut response: Vec<models::JsActiveContract> = Vec::new();
    for active_contract in result {
        let Some(contract_entry) = active_contract.contract_entry.as_deref() else {
            log::warn!(
                "post_v2_state_active_contracts: skipping entry with no contract_entry"
            );
            continue;
        };
        match contract_entry {
            models::JsContractEntry::JsContractEntryOneOf(a) => {
                response.push(*a.js_active_contract.clone());
            }
            models::JsContractEntry::JsContractEntryOneOf2(v) => {
                if let Some(handler) = params.unknown_contract_entry_handler {
                    handler(models::JsContractEntry::JsContractEntryOneOf2(v.clone()));
                }
            }
            models::JsContractEntry::JsContractEntryOneOf3(v) => {
                if let Some(handler) = params.unknown_contract_entry_handler {
                    handler(models::JsContractEntry::JsContractEntryOneOf3(v.clone()));
                }
            }
            models::JsContractEntry::JsContractEntryOneOf1(v) => {
                if let Some(handler) = params.unknown_contract_entry_handler {
                    handler(models::JsContractEntry::JsContractEntryOneOf1(v.clone()));
                }
            }
        }
    }

    Ok(response)
}

/// Filter active contracts based on CreateArgument values
#[allow(dead_code)]
fn filter_active_contracts_by_create_argument(
    contracts: Vec<models::JsActiveContract>,
    filters: &HashMap<String, String>,
) -> Vec<models::JsActiveContract> {
    contracts
        .into_iter()
        .filter(|contract| {
            // Navigate: Box<CreatedEvent> → Option<Value>
            if let Some(create_arg) = &contract.created_event.create_argument
                && let Some(obj) = create_arg.as_object()
            {
                return filters.iter().all(|(key, value)| {
                    obj.get(key)
                        .and_then(Value::as_str)
                        .map(|s| s == value)
                        .unwrap_or(false)
                });
            }
            false
        })
        .collect()
}

#[cfg(test)]
mod integration_tests {
    //! Live integration test for the active-contracts query. It
    //! authenticates with the client-credentials flow and needs these env
    //! vars (a `.env` file is loaded when present): `LEDGER_HOST`,
    //! `PARTY_ID_1`, `KEYCLOAK_URL` (full token endpoint URL),
    //! `KEYCLOAK_CLIENT_AUTH_CLIENT_ID`,
    //! `KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET`.

    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials};
    use std::env;

    fn var(name: &str) -> String {
        env::var(name).unwrap_or_else(|_| panic!("{name} must be set for integration tests"))
    }

    #[tokio::test]
    #[ignore = "integration test: requires live devnet and env vars"]
    async fn integration_get_by_party() {
        dotenvy::dotenv().ok();
        let ledger_host = var("LEDGER_HOST");

        let login = client_credentials(ClientCredentialsParams {
            client_id: var("KEYCLOAK_CLIENT_AUTH_CLIENT_ID"),
            client_secret: var("KEYCLOAK_CLIENT_AUTH_CLIENT_SECRET"),
            url: var("KEYCLOAK_URL"),
        })
        .await
        .expect("keycloak client-credentials login failed");

        let ledger_end_response = ledger_end::get(ledger_end::Params {
            access_token: login.access_token.clone(),
            ledger_host: ledger_host.clone(),
        })
        .await
        .expect("failed to get ledger end");

        let result = get_by_party(Params {
            ledger_host,
            party: var("PARTY_ID_1"),
            filter: common::IdentifierFilter::WildcardIdentifierFilter(
                common::WildcardIdentifierFilter {
                    wildcard_filter: common::WildcardFilter {
                        value: common::WildcardFilterValue {
                            include_created_event_blob: true,
                        },
                    },
                },
            ),
            access_token: login.access_token,
            ledger_end: ledger_end_response.offset,
            unknown_contract_entry_handler: None,
        })
        .await
        .expect("get_by_party failed");

        assert!(
            !result.is_empty(),
            "party 1 should hold at least one active contract"
        );
    }
}
