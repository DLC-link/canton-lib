use base64::Engine;
use serde::Deserialize;

pub struct ClientCredentialsParams {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
}

pub struct PasswordParams {
    pub client_id: String,
    pub username: String,
    pub password: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct Response {
    pub access_token: String,
    #[serde(default)]
    pub expires_in: u32,
    #[serde(default)]
    pub refresh_token: String,
}

impl Response {
    /// Extract the user ID (subject claim) from the access token JWT
    /// Returns the 'sub' claim which is typically the user's UUID
    pub fn get_user_id(&self) -> Result<String, String> {
        self.get_claim("sub").and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "'sub' claim is not a string".to_string())
        })
    }

    /// Extract an arbitrary claim from the access token JWT (decode-only, no signature verification).
    ///
    /// This only decodes the JWT payload; it does not verify the signature or validate
    /// standard claims (exp, aud, iss). Do not use for authorization decisions —
    /// rely on server-side token validation for that.
    pub fn get_claim(&self, claim_name: &str) -> Result<serde_json::Value, String> {
        // JWT format: header.payload.signature
        let parts: Vec<&str> = self.access_token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT format".to_string());
        }

        // Decode the payload (second part)
        let payload = parts[1];

        // URL-safe base64 without padding - try URL_SAFE_NO_PAD first, fall back to URL_SAFE with padding
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .or_else(|_| {
                let padding_needed = (4 - (payload.len() % 4)) % 4;
                let padded = format!("{}{}", payload, "=".repeat(padding_needed));
                base64::engine::general_purpose::URL_SAFE.decode(&padded)
            })
            .map_err(|e| format!("Failed to decode JWT payload: {}", e))?;

        // Parse JSON
        let json: serde_json::Value = serde_json::from_slice(&decoded)
            .map_err(|e| format!("Failed to parse JWT payload JSON: {}", e))?;

        json.get(claim_name)
            .cloned()
            .ok_or_else(|| format!("JWT does not contain '{}' claim", claim_name))
    }
}

pub struct RefreshParams {
    pub client_id: String,
    pub refresh_token: String,
    pub url: String,
}

pub async fn client_credentials(params: ClientCredentialsParams) -> Result<Response, String> {
    let client = reqwest::Client::new();
    client_credentials_with_client(params, &client).await
}

pub async fn client_credentials_with_client(
    params: ClientCredentialsParams,
    client: &reqwest::Client,
) -> Result<Response, String> {
    let form = [
        ("grant_type", "client_credentials"),
        ("client_id", &*params.client_id),
        ("client_secret", &*params.client_secret),
    ];

    let res = client
        .post(params.url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Keycloak client_credentials login request error: {}", e))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| format!("Failed to read response (client_credentials): {}", e))?;
    if !status.is_success() {
        return Err(format!(
            "Failed to get token (client_credentials) [{}]: {}",
            status, body
        ));
    }
    let response: Response = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response (client_credentials): {}", e))?;

    Ok(response)
}

pub async fn password(params: PasswordParams) -> Result<Response, String> {
    let client = reqwest::Client::new();
    password_with_client(params, &client).await
}

pub async fn password_with_client(
    params: PasswordParams,
    client: &reqwest::Client,
) -> Result<Response, String> {
    let form = [
        ("grant_type", "password"),
        ("client_id", &*params.client_id),
        ("username", &*params.username),
        ("password", &*params.password),
    ];
    let res = client
        .post(params.url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Keycloak password login request error: {}", e))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    if !status.is_success() {
        return Err(format!(
            "Failed to get token (password) [{}]: {}",
            status, body
        ));
    }
    let response: Response = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response (password): {}", e))?;

    Ok(response)
}

/// Build the OIDC token endpoint URL for a realm on a Keycloak server
/// that exposes the legacy `/auth` context root (Keycloak ≤ 16, or 17+
/// started with `--http-relative-path=/auth`).
///
/// Keycloak serves a single token endpoint per realm that handles every
/// grant type (`client_credentials`, `password`, `refresh_token`, …); this
/// helper is named for its caller in this module ([`client_credentials`])
/// but the URL itself is not grant-specific.
#[deprecated(
    since = "0.5.1",
    note = "use `token_url(host, realm)` instead; for legacy `/auth` deployments pass `{host}/auth` as the host"
)]
pub fn client_credentials_url(host: &str, realm: &str) -> String {
    format!(
        "{}/auth/realms/{}/protocol/openid-connect/token",
        host, realm
    )
}

/// Build the OIDC token endpoint URL for a realm on a Keycloak server
/// that exposes the legacy `/auth` context root. Alias of
/// [`client_credentials_url`] kept for call-site readability — the
/// underlying token endpoint is shared across all grant types.
#[deprecated(
    since = "0.5.1",
    note = "use `token_url(host, realm)` instead; for legacy `/auth` deployments pass `{host}/auth` as the host"
)]
pub fn password_url(host: &str, realm: &str) -> String {
    format!(
        "{}/auth/realms/{}/protocol/openid-connect/token",
        host, realm
    )
}

/// Build the OIDC token endpoint URL for the `master` realm on a Keycloak
/// server that exposes the legacy `/auth` context root. The endpoint is
/// shared across grant types.
#[deprecated(
    since = "0.5.1",
    note = "use `master_token_url(host)` instead; for legacy `/auth` deployments pass `{host}/auth` as the host"
)]
pub fn password_master_url(host: &str) -> String {
    format!("{}/auth/realms/master/protocol/openid-connect/token", host)
}

/// Build the OIDC token endpoint URL for a Keycloak realm using the
/// Keycloak 17+ (Quarkus) path layout — no `/auth` context root.
///
/// Produces `{host}/realms/{realm}/protocol/openid-connect/token`, which is
/// the default for Keycloak 17 and later. A trailing `/` on `host` is
/// trimmed so callers can pass either `https://kc.example.com` or
/// `https://kc.example.com/`. If a deployment still serves the legacy
/// `/auth/realms/...` paths (Keycloak ≤ 16, or 17+ started with
/// `--http-relative-path=/auth`), include `/auth` in the `host` argument,
/// e.g. `https://kc.example.com/auth`.
pub fn token_url(host: &str, realm: &str) -> String {
    let host = host.trim_end_matches('/');
    format!("{host}/realms/{realm}/protocol/openid-connect/token")
}

/// Build the OIDC token endpoint URL for the `master` realm using the
/// Keycloak 17+ (Quarkus) path layout — no `/auth` context root.
///
/// A trailing `/` on `host` is trimmed. See [`token_url`] for notes on
/// legacy `/auth` deployments.
pub fn master_token_url(host: &str) -> String {
    let host = host.trim_end_matches('/');
    format!("{host}/realms/master/protocol/openid-connect/token")
}

pub async fn refresh(params: RefreshParams) -> Result<Response, String> {
    let client = reqwest::Client::new();
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", &*params.client_id),
        ("refresh_token", &*params.refresh_token),
    ];

    let res = client
        .post(params.url)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Keycloak refresh token request error: {}", e))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| format!("Failed to read response (refresh): {}", e))?;
    if !status.is_success() {
        return Err(format!("Failed to refresh token [{}]: {}", status, body));
    }
    let response: Response = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse response (refresh): {}", e))?;

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};

    /// Build a fake JWT (header.payload.signature) encoding the payload with the given engine
    fn fake_jwt(payload: &serde_json::Value, engine: &impl base64::Engine) -> String {
        let header = engine.encode(b"{}");
        let body = engine.encode(payload.to_string().as_bytes());
        let sig = engine.encode(b"sig");
        format!("{}.{}.{}", header, body, sig)
    }

    #[test]
    fn test_get_claim_url_safe_no_pad() {
        // Payload crafted so that base64 output contains '-' and '_' (URL-safe chars)
        // which would be '+' and '/' in standard base64. STANDARD.decode would fail on these.
        let payload = serde_json::json!({
            "sub": "user>>>???<<<",
            "iss": "https://example.com/auth/realms/test"
        });
        let token = fake_jwt(&payload, &URL_SAFE_NO_PAD);

        // Verify the token payload actually contains URL-safe-only characters
        let raw_payload = token.split('.').nth(1).unwrap();
        assert!(
            !raw_payload.contains('+') && !raw_payload.contains('/'),
            "URL_SAFE_NO_PAD should not produce + or / characters"
        );

        let resp = Response {
            access_token: token,
            expires_in: 300,
            refresh_token: String::new(),
        };

        assert_eq!(resp.get_user_id().unwrap(), "user>>>???<<<");
        assert_eq!(
            resp.get_claim("iss").unwrap(),
            serde_json::json!("https://example.com/auth/realms/test")
        );
    }

    #[test]
    fn test_get_claim_url_safe_with_padding_fallback() {
        // Tokens with URL-safe chars AND padding — the fallback path
        let payload = serde_json::json!({
            "sub": "user>>>???<<<",
            "role": "admin"
        });
        let token = fake_jwt(&payload, &URL_SAFE);
        let resp = Response {
            access_token: token,
            expires_in: 300,
            refresh_token: String::new(),
        };

        assert_eq!(resp.get_user_id().unwrap(), "user>>>???<<<");
        assert_eq!(resp.get_claim("role").unwrap(), serde_json::json!("admin"));
    }

    #[test]
    fn test_get_claim_missing_claim() {
        let payload = serde_json::json!({"sub": "user-1"});
        let token = fake_jwt(&payload, &URL_SAFE_NO_PAD);
        let resp = Response {
            access_token: token,
            expires_in: 0,
            refresh_token: String::new(),
        };
        assert!(resp.get_claim("nonexistent").is_err());
    }

    #[test]
    fn test_get_claim_invalid_jwt() {
        let resp = Response {
            access_token: "not-a-jwt".to_string(),
            expires_in: 0,
            refresh_token: String::new(),
        };
        assert!(resp.get_claim("sub").is_err());
    }

    #[tokio::test]
    async fn test_password_login() {
        let url = std::env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set");
        let realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
        let client_id =
            std::env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set");
        let username = std::env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set");
        let user_password =
            std::env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set");

        let token_url = password_url(&url, &realm);
        let response = password(PasswordParams {
            url: token_url,
            client_id,
            username,
            password: user_password,
        })
        .await
        .expect("Password login should succeed");

        assert!(
            !response.access_token.is_empty(),
            "Access token should not be empty"
        );
        assert!(response.expires_in > 0, "expires_in should be positive");

        // Verify URL_SAFE_NO_PAD decoding works on a real token
        let user_id = response
            .get_user_id()
            .expect("Should be able to extract user_id (sub) from real token");
        assert!(
            !user_id.is_empty(),
            "User ID from real token should not be empty"
        );
        println!("Decoded user_id (sub): {}", user_id);

        // Also verify we can extract another standard claim
        let iss = response
            .get_claim("iss")
            .expect("Should be able to extract 'iss' claim from real token");
        println!("Decoded issuer (iss): {}", iss);
    }
}
