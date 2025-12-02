use crate::common;
use canton_api_client::apis::default_api as canton_api;
use canton_api_client::models;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;

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
        match active_contract.contract_entry.deref() {
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
            // Navigate: Box<CreatedEvent> → Option<Option<Value>>
            if let Some(Some(create_arg)) = &contract.created_event.create_argument
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
mod tests {
    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;

    #[tokio::test]
    async fn test_get_by_party() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = ClientCredentialsParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            client_secret: env::var("LIB_TEST_LEDGER_END_CLIENT_SECRET")
                .expect("LIB_TEST_LEDGER_END_CLIENT_SECRET must be set"),
            url: client_credentials_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = client_credentials(params).await.unwrap();

        let ledger_end_response = ledger_end::get(ledger_end::Params {
            access_token: login_response.access_token.clone(),
            ledger_host: ledger_host.to_string(),
        })
        .await
        .unwrap();

        let result = get_by_party(Params {
            ledger_host: ledger_host.to_string(),
            party: party_id,
            filter: common::IdentifierFilter::WildcardIdentifierFilter(
                common::WildcardIdentifierFilter {
                    wildcard_filter: common::WildcardFilter {
                        value: common::WildcardFilterValue {
                            include_created_event_blob: true,
                        },
                    },
                },
            ),
            access_token: login_response.access_token,
            ledger_end: ledger_end_response.offset,
            unknown_contract_entry_handler: None,
        })
        .await
        .unwrap();

        assert!(!result.is_empty());
    }
}
