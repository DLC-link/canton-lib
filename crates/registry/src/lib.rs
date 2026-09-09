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

/// POST `body` to `url`, then parse the registry's JSON answer.
///
/// The four registry `get` functions differ only in their URL and their
/// response type, so each one is now this call plus a URL. Keeping the
/// status check here means one test covers the failure path of all four.
///
/// The body is read as text before parsing, so a non-success status can
/// report what the registry actually said. A response is never retried,
/// whatever its status — see [`post_json`].
pub(crate) async fn post_and_parse<B, T>(url: &str, body: &B) -> Result<T, String>
where
    B: serde::Serialize + ?Sized,
    T: serde::de::DeserializeOwned,
{
    let response = post_json(url, body)
        .await
        .map_err(|e| format!("Failed to send request to registry: {e}"))?;

    let status = response.status();
    let body_raw = response
        .text()
        .await
        .map_err(|e| format!("Failed to read registry response: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "Registry request failed with status {status}: {body_raw}"
        ));
    }

    serde_json::from_str(&body_raw).map_err(|e| format!("Failed to parse registry response: {e}"))
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

    /// The case the retry exists for: the first attempt never reaches the host,
    /// the second one answers, and the caller sees a success.
    ///
    /// The other three tests all end in a failure or a single request, so an
    /// implementation that retried and then returned the first error would
    /// pass them. This one fails against that implementation.
    #[tokio::test]
    async fn a_transport_failure_recovers_on_the_next_attempt() {
        let server = MockServer::start().await;

        // First attempt: answers later than the per-attempt timeout allows, so
        // this client gives up on it and retries.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second attempt: falls through to this one and answers at once.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body()))
            .mount(&server)
            .await;

        let response = post_json_within(&server.uri(), &body(), Duration::from_millis(100))
            .await
            .expect("the second attempt answers, so the call must succeed");
        assert_eq!(response.status(), 200);

        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests.len(),
            2,
            "the first attempt must be retried exactly once, not abandoned or repeated to the budget"
        );
        assert_eq!(
            requests[0].body, requests[1].body,
            "the retry must carry the same body as the attempt it replaces"
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
