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

    /// Extract an arbitrary claim from the access token JWT
    ///
    /// Useful for extracting custom claims like party_id, roles, etc.
    pub fn get_claim(&self, claim_name: &str) -> Result<serde_json::Value, String> {
        let parts: Vec<&str> = self.access_token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT format".to_string());
        }

        let payload = parts[1];

        // Try URL-safe base64 first, fall back to standard with padding
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .or_else(|_| {
                let padding_needed = (4 - (payload.len() % 4)) % 4;
                let padded = format!("{}{}", payload, "=".repeat(padding_needed));
                base64::engine::general_purpose::STANDARD.decode(&padded)
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
}
