use crate::common;
use canton_api_client::apis::configuration::Configuration;
use canton_api_client::apis::default_api as canton_api;
use canton_api_client::apis::{Error, ResponseContent};
use canton_api_client::models;
use serde::{Deserialize, Serialize, de::Error as _};
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
    let request = common::GetActiveContractsRequest {
        event_format: common::EventFormat::for_party(params.party, params.filter, false),
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
            log::warn!("post_v2_state_active_contracts: skipping entry with no contract_entry");
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

// `post_v2_state_active_contracts_page` and its error type are generated with
// openapi-generator 7.22.0 (`-g rust`) from the Canton 3.6.1 OpenAPI spec, the
// same way `canton-api-client` is built. `canton-api-client` 3.6.0 has only the
// GET form, which Canton 3.6.1 disables by default and Canton 3.7 removes. Only
// the module paths, the formatting and the lint allows differ from the generated
// code. `ContentType` is copied because the client keeps it private. When a
// newer `canton-api-client` has this function, delete this block and call the
// generated one.

/// struct for typed errors of method [`post_v2_state_active_contracts_page`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PostV2StateActiveContractsPageError {
    Status400(String),
    DefaultResponse(models::JsCantonError),
    UnknownValue(serde_json::Value),
}

#[allow(clippy::result_large_err, clippy::needless_return)]
pub async fn post_v2_state_active_contracts_page(
    configuration: &Configuration,
    get_active_contracts_page_request: models::GetActiveContractsPageRequest,
) -> Result<models::JsGetActiveContractsPageResponse, Error<PostV2StateActiveContractsPageError>> {
    // add a prefix to parameters to efficiently prevent name collisions
    let p_body_get_active_contracts_page_request = get_active_contracts_page_request;

    let uri_str = format!("{}/v2/state/active-contracts-page", configuration.base_path);
    let mut req_builder = configuration
        .client
        .request(reqwest::Method::POST, &uri_str);

    if let Some(ref user_agent) = configuration.user_agent {
        req_builder = req_builder.header(reqwest::header::USER_AGENT, user_agent.clone());
    }
    if let Some(ref token) = configuration.bearer_access_token {
        req_builder = req_builder.bearer_auth(token.to_owned());
    };
    if let Some(ref apikey) = configuration.api_key {
        let key = apikey.key.clone();
        let value = match apikey.prefix {
            Some(ref prefix) => format!("{} {}", prefix, key),
            None => key,
        };
        req_builder = req_builder.header("Sec-WebSocket-Protocol", value);
    };
    req_builder = req_builder.json(&p_body_get_active_contracts_page_request);

    let req = req_builder.build()?;
    let resp = configuration.client.execute(req).await?;

    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let content_type = ContentType::from(content_type);

    if !status.is_client_error() && !status.is_server_error() {
        let content = resp.text().await?;
        match content_type {
            ContentType::Json => serde_json::from_str(&content).map_err(Error::from),
            ContentType::Text => {
                return Err(Error::from(serde_json::Error::custom(
                    "Received `text/plain` content type response that cannot be converted to `models::JsGetActiveContractsPageResponse`",
                )));
            }
            ContentType::Unsupported(unknown_type) => {
                return Err(Error::from(serde_json::Error::custom(format!(
                    "Received `{unknown_type}` content type response that cannot be converted to `models::JsGetActiveContractsPageResponse`"
                ))));
            }
        }
    } else {
        let content = resp.text().await?;
        let entity: Option<PostV2StateActiveContractsPageError> =
            serde_json::from_str(&content).ok();
        Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity,
        }))
    }
}

/// A content type supported by this client.
#[allow(dead_code)]
enum ContentType {
    Json,
    Text,
    Unsupported(String),
}

#[allow(clippy::needless_return)]
impl From<&str> for ContentType {
    fn from(content_type: &str) -> Self {
        if content_type.starts_with("application") && content_type.contains("json") {
            return Self::Json;
        } else if content_type.starts_with("text/plain") {
            return Self::Text;
        } else {
            return Self::Unsupported(content_type.to_string());
        }
    }
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
    //!
    //! The query filters for the party's single `UserService` contract. A
    //! wildcard filter would 413 once the party's ACS outgrows the node's
    //! HTTP list limit.

    use super::*;
    use crate::ledger_end;
    use keycloak::login::{ClientCredentialsParams, client_credentials};
    use std::env;

    const USER_SERVICE_TEMPLATE_ID: &str =
        "#utility-credential-app-v0:Utility.Credential.App.V0.Service.User:UserService";

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
            filter: common::IdentifierFilter::TemplateIdentifierFilter(
                common::TemplateIdentifierFilter {
                    template_filter: common::TemplateFilter {
                        value: common::TemplateFilterValue {
                            template_id: Some(USER_SERVICE_TEMPLATE_ID.to_string()),
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
            "party 1 should hold a UserService contract"
        );
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;

    /// What the participant received: the request headers and the JSON body.
    type Received = (axum::http::HeaderMap, serde_json::Value);

    /// A participant that serves only `POST /v2/state/active-contracts-page`
    /// and answers it with `status` and `body`. Any other method gets 405.
    async fn participant(
        status: axum::http::StatusCode,
        body: &'static str,
    ) -> (String, tokio::sync::mpsc::UnboundedReceiver<Received>) {
        let (sender, received) = tokio::sync::mpsc::unbounded_channel();
        let app = axum::Router::new().route(
            "/v2/state/active-contracts-page",
            axum::routing::post(
                move |headers: axum::http::HeaderMap,
                      axum::Json(request): axum::Json<serde_json::Value>| async move {
                    let _ = sender.send((headers, request));
                    (status, [("content-type", "application/json")], body)
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the test port must bind");
        let base = format!(
            "http://{}",
            listener.local_addr().expect("the port has an address")
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        (base, received)
    }

    fn configuration(base_path: String) -> Configuration {
        Configuration {
            base_path,
            bearer_access_token: Some("token".into()),
            ..Configuration::default()
        }
    }

    fn request() -> models::GetActiveContractsPageRequest {
        models::GetActiveContractsPageRequest {
            active_at_offset: Some(42),
            event_format: Box::new(models::EventFormat {
                filters_by_party: None,
                filters_for_any_party: None,
                verbose: Some(true),
            }),
            max_page_size: Some(100),
            page_token: Some("page-1".into()),
        }
    }

    #[tokio::test]
    async fn sends_post_with_the_token_and_the_request() {
        let (base, mut received) = participant(
            axum::http::StatusCode::OK,
            r#"{"activeContracts":[],"activeAtOffset":42}"#,
        )
        .await;

        let page = post_v2_state_active_contracts_page(&configuration(base), request())
            .await
            .expect("the page must read");
        let (headers, body) = received.try_recv().expect("the participant got the POST");

        assert_eq!(
            headers.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer token")
        );
        assert_eq!(body["activeAtOffset"], 42);
        assert_eq!(body["maxPageSize"], 100);
        assert_eq!(body["pageToken"], "page-1");
        assert_eq!(page.active_at_offset, 42);
        assert!(page.active_contracts.is_empty());
    }

    #[tokio::test]
    async fn a_refusal_keeps_the_status_and_the_canton_error() {
        let (base, _received) = participant(
            axum::http::StatusCode::BAD_REQUEST,
            r#"{"code":"DEPRECATED_API_DISABLED","cause":"disabled","context":{},"errorCategory":8}"#,
        )
        .await;

        let error = post_v2_state_active_contracts_page(&configuration(base), request())
            .await
            .expect_err("a refused read must fail");

        let Error::ResponseError(response) = error else {
            panic!("expected a ResponseError, got {error}");
        };
        assert_eq!(response.status.as_u16(), 400);
        assert!(response.content.contains("DEPRECATED_API_DISABLED"));
        let Some(PostV2StateActiveContractsPageError::DefaultResponse(canton_error)) =
            response.entity
        else {
            panic!("expected the Canton error, got {:?}", response.entity);
        };
        assert_eq!(canton_error.code, "DEPRECATED_API_DISABLED");
    }

    #[tokio::test]
    async fn a_page_that_does_not_parse_is_a_serde_error() {
        let (base, _received) = participant(axum::http::StatusCode::OK, "{}").await;

        let error = post_v2_state_active_contracts_page(&configuration(base), request())
            .await
            .expect_err("a page without its fields must fail");

        assert!(matches!(error, Error::Serde(_)), "got {error}");
    }
}
