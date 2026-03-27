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
        // JWT format: header.payload.signature
        let parts: Vec<&str> = self.access_token.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid JWT format".to_string());
        }

        // Decode the payload (second part)
        let payload = parts[1];

        // URL-safe base64 without padding - try URL_SAFE first, fall back to STANDARD with padding
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .or_else(|_| {
                let padding_needed = (4 - (payload.len() % 4)) % 4;
                let padded = format!("{}{}", payload, "=".repeat(padding_needed));
                base64::engine::general_purpose::STANDARD.decode(&padded)
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

pub fn client_credentials_url(host: &str, realm: &str) -> String {
    format!(
        "{}/auth/realms/{}/protocol/openid-connect/token",
        host, realm
    )
}

pub fn password_url(host: &str, realm: &str) -> String {
    format!(
        "{}/auth/realms/{}/protocol/openid-connect/token",
        host, realm
    )
}

pub fn password_master_url(host: &str) -> String {
    format!("{}/auth/realms/master/protocol/openid-connect/token", host)
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
