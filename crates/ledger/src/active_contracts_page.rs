//! The POST form of `/v2/state/active-contracts-page`.
//!
//! `canton-api-client` 3.6.0 has only `get_v2_state_active_contracts_page`.
//! Canton 3.6.1 disables that GET form by default, and Canton 3.7 removes it.
//! This function sends the same request with POST. It has the same name style,
//! arguments and error type as a generated function, so a caller can move to
//! the generated one when the client has it.

use canton_api_client::apis::configuration::Configuration;
use canton_api_client::apis::{Error, ResponseContent};
use canton_api_client::models;

/// Sends `POST /v2/state/active-contracts-page` and returns one page.
///
/// A failed send or body read is `Error::Reqwest`, a non-2xx answer is
/// `Error::ResponseError` with the status and the body, and a page that does
/// not parse is `Error::Serde`.
pub async fn post_v2_state_active_contracts_page(
    configuration: &Configuration,
    request: &models::GetActiveContractsPageRequest,
) -> Result<models::JsGetActiveContractsPageResponse, Error<()>> {
    let url = format!("{}/v2/state/active-contracts-page", configuration.base_path);
    let mut builder = configuration.client.post(&url).json(request);
    if let Some(user_agent) = &configuration.user_agent {
        builder = builder.header("user-agent", user_agent);
    }
    if let Some(token) = &configuration.bearer_access_token {
        builder = builder.bearer_auth(token);
    }
    let response = builder.send().await?;
    let status = response.status();
    let content = response.text().await?;
    if !status.is_success() {
        return Err(Error::ResponseError(ResponseContent {
            status,
            content,
            entity: None,
        }));
    }
    Ok(serde_json::from_str(&content)?)
}

#[cfg(test)]
mod tests {
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

        let page = post_v2_state_active_contracts_page(&configuration(base), &request())
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
    async fn a_refusal_keeps_the_status_and_the_body() {
        let (base, _received) = participant(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            r#"{"code":"DEPRECATED_API_DISABLED"}"#,
        )
        .await;

        let error = post_v2_state_active_contracts_page(&configuration(base), &request())
            .await
            .expect_err("a refused read must fail");

        let Error::ResponseError(response) = error else {
            panic!("expected a ResponseError, got {error}");
        };
        assert_eq!(response.status.as_u16(), 500);
        assert!(response.content.contains("DEPRECATED_API_DISABLED"));
    }

    #[tokio::test]
    async fn a_page_that_does_not_parse_is_a_serde_error() {
        let (base, _received) = participant(axum::http::StatusCode::OK, "{}").await;

        let error = post_v2_state_active_contracts_page(&configuration(base), &request())
            .await
            .expect_err("a page without its fields must fail");

        assert!(matches!(error, Error::Serde(_)), "got {error}");
    }
}
