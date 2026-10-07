pub mod active_contracts;
pub mod update;

use serde::Deserialize;

/// An error the Ledger API sends in place of a stream message.
#[derive(Deserialize)]
struct LedgerApiError {
    code: String,
    cause: String,
}

/// Returns the error that `text` carries, quoted as `<code>: <cause>`, or
/// `None` when `text` is an ordinary stream message.
///
/// `canton_api_client::models::JsCantonError` describes the same message, but
/// it also requires `context` and `errorCategory`. An error without them would
/// then read as data, so this checks only `code` and `cause`.
pub(crate) fn ledger_api_error(text: &str) -> Option<String> {
    serde_json::from_str::<LedgerApiError>(text)
        .ok()
        .map(|e| format!("Ledger API returned {}: {}", e.code, e.cause))
}

#[cfg(test)]
mod tests {
    //! Each test runs a function against a local websocket server that sends
    //! a fixed list of messages and then closes, as the participant does.

    use super::*;
    use crate::common;
    use futures_util::{SinkExt, StreamExt};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::http::HeaderValue;

    /// The message a Canton 3.6.1 participant sent to a request that carried
    /// the disabled `filter` field. The participant may send more fields;
    /// these are the ones it sent first.
    const DEPRECATED_API_DISABLED: &str = r#"{"code":"DEPRECATED_API_DISABLED","cause":"The fields filter/verbose of the /v2/state/active-contracts requests was deprecated in Canton 3.4, is disabled by default since Canton 3.6 and will be removed in Canton 3.7.","correlationId":null,"traceId":"53bc7da6f755bd6246307e"}"#;

    /// Serves one connection: reads the request, sends `messages`, closes.
    /// Returns the `http://` host to pass as `ledger_host`.
    // tungstenite's `Callback` trait fixes the handshake closure's error type.
    #[allow(clippy::result_large_err)]
    async fn serve_once(messages: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            // The client offers `daml.ws.auth`, so the server must name it.
            let ws =
                tokio_tungstenite::accept_hdr_async(stream, |_: &Request, mut resp: Response| {
                    resp.headers_mut().insert(
                        "Sec-WebSocket-Protocol",
                        HeaderValue::from_static("daml.ws.auth"),
                    );
                    Ok(resp)
                })
                .await
                .unwrap();
            let (mut write, mut read) = ws.split();
            read.next().await;
            for message in messages {
                write.send(Message::Text(message)).await.unwrap();
            }
            let _ = write.send(Message::Close(None)).await;
        });
        format!("http://{addr}")
    }

    fn contract_message() -> String {
        serde_json::to_string(&active_contracts::ContractMessage {
            workflow_id: None,
            contract_entry: Some(active_contracts::ContractEntry::default()),
        })
        .unwrap()
    }

    fn any_filter() -> common::IdentifierFilter {
        common::IdentifierFilter::TemplateIdentifierFilter(common::TemplateIdentifierFilter {
            template_filter: common::TemplateFilter {
                value: common::TemplateFilterValue {
                    template_id: Some("#pkg:Module:Template".to_string()),
                    include_created_event_blob: false,
                },
            },
        })
    }

    fn acs_params(ledger_host: String) -> active_contracts::Params {
        active_contracts::Params {
            ledger_host,
            party: "party::1220".to_string(),
            filter: any_filter(),
            access_token: "token".to_string(),
            ledger_end: 0,
        }
    }

    fn update_params(ledger_host: String) -> update::Params {
        update::Params {
            ledger_host,
            party: "party::1220".to_string(),
            filter: any_filter(),
            access_token: "token".to_string(),
            ledger_end: 0,
        }
    }

    #[tokio::test]
    async fn get_returns_the_contracts_the_server_sends() {
        let host = serve_once(vec![contract_message(), contract_message()]).await;

        let contracts = active_contracts::get(acs_params(host)).await.unwrap();

        assert_eq!(contracts.len(), 2);
    }

    #[tokio::test]
    async fn get_returns_the_servers_error_instead_of_an_empty_list() {
        let host = serve_once(vec![DEPRECATED_API_DISABLED.to_string()]).await;

        let err = active_contracts::get(acs_params(host)).await.unwrap_err();

        assert!(err.contains("DEPRECATED_API_DISABLED"), "{err}");
        assert!(
            err.contains("disabled by default since Canton 3.6"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn get_with_callback_passes_each_contract_message_to_the_callback() {
        let host = serve_once(vec![contract_message(), contract_message()]).await;
        let calls = Arc::new(AtomicUsize::new(0));

        let counter = calls.clone();
        active_contracts::get_with_callback(acs_params(host), move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            async {}
        })
        .await
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn get_with_callback_returns_the_servers_error_and_skips_the_callback() {
        let host = serve_once(vec![DEPRECATED_API_DISABLED.to_string()]).await;
        let calls = Arc::new(AtomicUsize::new(0));

        let counter = calls.clone();
        let err = active_contracts::get_with_callback(acs_params(host), move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
            async {}
        })
        .await
        .unwrap_err();

        assert!(err.contains("DEPRECATED_API_DISABLED"), "{err}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    static UPDATES_HANDLED: AtomicUsize = AtomicUsize::new(0);

    fn count_update(_: String) -> Result<(), String> {
        UPDATES_HANDLED.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn accept_any(_: String) -> Result<(), String> {
        Ok(())
    }

    #[tokio::test]
    async fn subscribe_passes_each_update_to_the_handler() {
        let host = serve_once(vec![r#"{"update":{}}"#.to_string(); 3]).await;

        update::subscribe(update_params(host), count_update)
            .await
            .unwrap();

        assert_eq!(UPDATES_HANDLED.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn subscribe_returns_the_servers_error_instead_of_ok() {
        let host = serve_once(vec![DEPRECATED_API_DISABLED.to_string()]).await;

        let err = update::subscribe(update_params(host), accept_any)
            .await
            .unwrap_err();

        assert!(err.contains("DEPRECATED_API_DISABLED"), "{err}");
    }
}
