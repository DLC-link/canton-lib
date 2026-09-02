pub mod accept_context;
pub mod allocation_context;
pub mod allocation_factory;
pub mod consts;
pub mod transfer_factory;

use std::time::Duration;

/// Attempts a registry POST gets, including the first.
const ATTEMPTS: u32 = 3;

/// How long one attempt waits for the registry to answer.
///
/// The calls this crate makes return in well under a second. Without a bound a
/// hung connection stalls the caller forever, and `is_timeout` never fires, so
/// the retry below could not see the failure it exists for.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// POST `body` to `url`, retrying only when the request never reached the host.
///
/// **A response is never retried, whatever its status.** A 4xx or a 5xx is the
/// registry's answer, and repeating the call would hide it. Only a failure that
/// produced no answer gets another attempt: a refused connection, a timeout, or
/// a request this client could not build.
///
/// The retries are immediate. The failure this exists for is a dropped
/// connection to a load-balanced host, where the next attempt reaches a
/// different backend and a delay buys nothing. A backoff would need a timer,
/// and this crate carries no async runtime of its own.
pub(crate) async fn post_json<B: serde::Serialize + ?Sized>(
    url: &str,
    body: &B,
) -> Result<reqwest::Response, reqwest::Error> {
    post_json_within(url, body, REQUEST_TIMEOUT).await
}

/// [`post_json`] with the per-attempt timeout named, so a test can drive the
/// retry without waiting [`REQUEST_TIMEOUT`] for each attempt.
pub(crate) async fn post_json_within<B: serde::Serialize + ?Sized>(
    url: &str,
    body: &B,
    timeout: Duration,
) -> Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let mut attempt = 1;
    loop {
        match client.post(url).json(body).send().await {
            Ok(response) => return Ok(response),
            Err(e) if attempt < ATTEMPTS && reached_no_host(&e) => attempt += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Did this error mean the registry never answered?
fn reached_no_host(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// A body, so the helper's `Serialize` bound has something to take.
    fn body() -> serde_json::Value {
        serde_json::json!({ "choiceArguments": {} })
    }

    #[tokio::test]
    async fn a_server_error_is_not_retried() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let response = post_json(&server.uri(), &body())
            .await
            .expect("a 500 is an answer, so the call must succeed at this layer");
        assert_eq!(response.status(), 500);

        assert_eq!(
            server.received_requests().await.unwrap().len(),
            1,
            "retrying a status would hide the registry's own rejection"
        );
    }

    #[tokio::test]
    async fn a_timeout_is_retried_to_the_budget() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .mount(&server)
            .await;

        let error = post_json_within(&server.uri(), &body(), Duration::from_millis(100))
            .await
            .expect_err("every attempt times out");
        assert!(error.is_timeout());

        assert_eq!(
            server.received_requests().await.unwrap().len(),
            ATTEMPTS as usize,
            "a timeout must be retried, and only up to the budget"
        );
    }

    #[tokio::test]
    async fn a_refused_connection_reads_as_no_host() {
        // Port 1 on loopback: nothing listens, so the connection is refused.
        let error = post_json_within("http://127.0.0.1:1/", &body(), Duration::from_millis(100))
            .await
            .expect_err("nothing listens on port 1");

        assert!(
            reached_no_host(&error),
            "a refused connection must be retryable"
        );
    }
}
