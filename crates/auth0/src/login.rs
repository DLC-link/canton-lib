use base64::Engine;
use serde::Deserialize;

/// Parameters for Auth0 client credentials authentication
pub struct ClientCredentialsParams {
    /// The Auth0 token endpoint URL (use `auth0_url()` to construct)
    pub url: String,
    /// Your Auth0 application's client ID
    pub client_id: String,
    /// Your Auth0 application's client secret
    pub client_secret: String,
    /// The API audience identifier
    pub audience: String,
}

/// Authentication response containing the access token
#[derive(Deserialize, Debug, Clone)]
pub struct Response {
    /// The JWT access token to use for API requests
    pub access_token: String,
    /// Token expiration time in seconds
    #[serde(default)]
    pub expires_in: u32,
    /// Token type (usually "Bearer")
    #[serde(default)]
    pub token_type: String,
}

impl Response {
    /// Extract the user ID (subject claim) from the access token JWT
    ///
    /// Returns the 'sub' claim which is typically the Auth0 user/client identifier.
    /// For machine-to-machine tokens, this is usually `client_id@clients`.
    pub fn get_user_id(&self) -> Result<String, String> {
        self.get_claim("sub")
            .and_then(|v| {
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
        let parts: Vec<&str> = self.access_token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT format".to_string());
        }

        let payload = parts[1];

        // Try URL-safe base64 first, fall back to URL-safe with padding
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .or_else(|_| {
                let padding_needed = (4 - (payload.len() % 4)) % 4;
                let padded = format!("{}{}", payload, "=".repeat(padding_needed));
                base64::engine::general_purpose::URL_SAFE.decode(&padded)
            })
            .map_err(|e| format!("Failed to decode JWT payload: {e}"))?;

        let json: serde_json::Value = serde_json::from_slice(&decoded)
            .map_err(|e| format!("Failed to parse JWT payload JSON: {e}"))?;

        json.get(claim_name)
            .cloned()
            .ok_or_else(|| format!("JWT does not contain '{claim_name}' claim"))
    }
}

/// Perform Auth0 client credentials authentication
pub async fn client_credentials(params: ClientCredentialsParams) -> Result<Response, String> {
    let client = reqwest::Client::new();
    client_credentials_with_client(params, &client).await
}

/// Perform Auth0 client credentials authentication with a pre-built HTTP client
pub async fn client_credentials_with_client(
    params: ClientCredentialsParams,
    client: &reqwest::Client,
) -> Result<Response, String> {
    let json_body = serde_json::json!({
        "grant_type": "client_credentials",
        "client_id": params.client_id,
        "client_secret": params.client_secret,
        "audience": params.audience,
    });

    let res = client
        .post(&params.url)
        .json(&json_body)
        .send()
        .await
        .map_err(|e| format!("Auth0 client_credentials request failed: {e}"))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| format!("Failed to read Auth0 response: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "Auth0 authentication failed [{status}]: {body}"
        ));
    }

    let response: Response = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse Auth0 response: {e}"))?;

    Ok(response)
}

/// Construct Auth0 OAuth token endpoint URL
///
/// # Arguments
/// * `domain` - Your Auth0 domain (e.g., "https://your-tenant.auth0.com")
///
/// # Returns
/// The full token endpoint URL (e.g., "https://your-tenant.auth0.com/oauth/token")
pub fn auth0_url(domain: &str) -> String {
    let domain = domain.trim_end_matches('/');
    format!("{domain}/oauth/token")
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
    fn test_auth0_url() {
        assert_eq!(
            auth0_url("https://example.auth0.com"),
            "https://example.auth0.com/oauth/token"
        );
        assert_eq!(
            auth0_url("https://example.auth0.com/"),
            "https://example.auth0.com/oauth/token"
        );
    }

    #[test]
    fn test_get_claim_url_safe_no_pad() {
        let payload = serde_json::json!({
            "sub": "user>>>???<<<",
            "iss": "https://example.auth0.com/"
        });
        let token = fake_jwt(&payload, &URL_SAFE_NO_PAD);

        // Verify the token payload contains no standard base64 chars
        let raw_payload = token.split('.').nth(1).unwrap();
        assert!(
            !raw_payload.contains('+') && !raw_payload.contains('/'),
            "URL_SAFE_NO_PAD should not produce + or / characters"
        );

        let resp = Response {
            access_token: token,
            expires_in: 300,
            token_type: String::new(),
        };

        assert_eq!(resp.get_user_id().unwrap(), "user>>>???<<<");
        assert_eq!(
            resp.get_claim("iss").unwrap(),
            serde_json::json!("https://example.auth0.com/")
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
            token_type: String::new(),
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
            token_type: String::new(),
        };
        assert!(resp.get_claim("nonexistent").is_err());
    }

    #[test]
    fn test_get_claim_invalid_jwt() {
        let resp = Response {
            access_token: "not-a-jwt".to_string(),
            expires_in: 0,
            token_type: String::new(),
        };
        assert!(resp.get_claim("sub").is_err());
    }
}
