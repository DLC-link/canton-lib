use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtendedString {
    String(String),
    Object(serde_json::Value),
}

impl ExtendedString {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ExtendedString::String(s) => Some(s),
            ExtendedString::Object(_) => None,
        }
    }
}

/// Domain types whose concrete representation wasn't provided
pub type Microseconds = ExtendedString;
pub type Rate = ExtendedString;
pub type Fee = ExtendedString;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Number {
    pub number: String,
}

/// `Step` wasn't defined in the Go snippet; this is a passthrough.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundsWrapper {
    #[serde(rename = "open_mining_rounds")]
    pub open_mining_rounds: Vec<OpenMiningRound>,

    #[serde(rename = "issuing_mining_rounds")]
    pub issuing_mining_rounds: Vec<IssuingMiningRound>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRound {
    #[serde(rename = "contract")]
    pub contract: OpenMiningRoundContract,

    #[serde(rename = "domain_id")]
    pub domain_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundContract {
    #[serde(rename = "template_id")]
    pub template_id: String,

    #[serde(rename = "contract_id")]
    pub contract_id: String,

    #[serde(rename = "payload")]
    pub payload: OpenMiningRoundPayload,

    #[serde(rename = "created_event_blob")]
    pub created_event_blob: String,

    #[serde(rename = "created_at")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundPayload {
    #[serde(rename = "dso")]
    pub dso: String,

    #[serde(rename = "tickDuration")]
    pub tick_duration: Microseconds,

    #[serde(rename = "issuingFor")]
    pub issuing_for: Microseconds,

    #[serde(rename = "amuletPrice")]
    pub amulet_price: String,

    #[serde(rename = "issuanceConfig")]
    pub issuance_config: OpenMiningRoundIssuanceConfig,

    #[serde(rename = "opensAt")]
    pub opens_at: DateTime<Utc>,

    #[serde(rename = "transferConfigUsd")]
    pub transfer_config_usd: OpenMiningRoundTransferConfigUsd,

    #[serde(rename = "targetClosesAt")]
    pub target_closes_at: DateTime<Utc>,

    #[serde(rename = "round")]
    pub round: Number,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundIssuanceConfig {
    #[serde(rename = "validatorRewardPercentage")]
    pub validator_reward_percentage: String,

    #[serde(rename = "unfeaturedAppRewardCap")]
    pub unfeatured_app_reward_cap: String,

    #[serde(rename = "appRewardPercentage")]
    pub app_reward_percentage: String,

    #[serde(rename = "featuredAppRewardCap")]
    pub featured_app_reward_cap: String,

    #[serde(rename = "amuletToIssuePerYear")]
    pub amulet_to_issue_per_year: String,

    #[serde(rename = "validatorRewardCap")]
    pub validator_reward_cap: String,

    #[serde(rename = "optValidatorFaucetCap")]
    pub opt_validator_faucet_cap: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundTransferConfigUsd {
    #[serde(rename = "holdingFee")]
    pub holding_fee: Rate,

    #[serde(rename = "extraFeaturedAppRewardAmount")]
    pub extra_featured_app_reward_amount: String,

    #[serde(rename = "maxNumInputs")]
    pub max_num_inputs: String,

    #[serde(rename = "lockHolderFee")]
    pub lock_holder_fee: Fee,

    #[serde(rename = "createFee")]
    pub create_fee: Fee,

    #[serde(rename = "maxNumLockHolders")]
    pub max_num_lock_holders: String,

    #[serde(rename = "transferFee")]
    pub transfer_fee: OpenMiningRoundTransferFee,

    #[serde(rename = "maxNumOutputs")]
    pub max_num_outputs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenMiningRoundTransferFee {
    #[serde(rename = "initialRate")]
    pub initial_rate: String,

    #[serde(rename = "steps")]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuingMiningRound {
    #[serde(rename = "contract")]
    pub contract: IssuingMiningRoundContract,

    #[serde(rename = "domain_id")]
    pub domain_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuingMiningRoundContract {
    #[serde(rename = "template_id")]
    pub template_id: String,

    #[serde(rename = "contract_id")]
    pub contract_id: String,

    #[serde(rename = "payload")]
    pub payload: IssuingMiningRoundPayload,

    #[serde(rename = "created_event_blob")]
    pub created_event_blob: String,

    #[serde(rename = "created_at")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuingMiningRoundPayload {
    #[serde(rename = "dso")]
    pub dso: String,

    #[serde(rename = "optIssuancePerValidatorFaucetCoupon")]
    pub opt_issuance_per_validator_faucet_coupon: String,

    #[serde(rename = "issuancePerFeaturedAppRewardCoupon")]
    pub issuance_per_featured_app_reward_coupon: String,

    #[serde(rename = "opensAt")]
    pub opens_at: DateTime<Utc>,

    #[serde(rename = "issuancePerSvRewardCoupon")]
    pub issuance_per_sv_reward_coupon: String,

    #[serde(rename = "targetClosesAt")]
    pub target_closes_at: DateTime<Utc>,

    #[serde(rename = "issuancePerUnfeaturedAppRewardCoupon")]
    pub issuance_per_unfeatured_app_reward_coupon: String,

    #[serde(rename = "round")]
    pub round: Number,

    #[serde(rename = "issuancePerValidatorRewardCoupon")]
    pub issuance_per_validator_reward_coupon: String,
}

/// GET /api/validator/v0/scan-proxy/open-and-issuing-mining-rounds
///
/// `base_url` corresponds to `env.GetWalletAPI()` in the Go code.
pub async fn get_open_mining_rounds(
    base_url: &str,
    token: &str,
) -> Result<OpenMiningRoundsWrapper, String> {
    let url = format!(
        "{}/api/validator/v0/scan-proxy/open-and-issuing-mining-rounds",
        base_url.trim_end_matches('/')
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| format!("Failed to get reqwest builder: {}", err))?;

    let response = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to {}: {}", url, e))?
        .error_for_status()
        .map_err(|e| format!("Error for status: {}", e))?;

    let raw_response = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response text from {}: {}", url, e))?;

    let result = serde_json::from_str::<OpenMiningRoundsWrapper>(&raw_response).map_err(|e| {
        format!(
            "Failed to parse response JSON from {}: {}. Response text: {}",
            url, e, raw_response
        )
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycloak::login::{ClientCredentialsParams, client_credentials, client_credentials_url};
    use std::env;

    #[tokio::test]
    async fn test_get_open_mining_rounds() {
        let params = ClientCredentialsParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            client_secret: env::var("LIB_TEST_AMULET_CLIENT_SECRET")
                .expect("LIB_TEST_AMULET_CLIENT_SECRET must be set"),
            url: client_credentials_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let result = client_credentials(params).await.unwrap();
        let wallet_api_host = env::var("WALLET_API_HOST").expect("WALLET_API_HOST must be set");

        let mining_rounds = get_open_mining_rounds(&wallet_api_host, &result.access_token)
            .await
            .expect("Failed to get open mining rounds");

        println!("Open Mining Rounds: {:?}", mining_rounds.open_mining_rounds);
        println!(
            "Issuing Mining Rounds: {:?}",
            mining_rounds.issuing_mining_rounds
        );
    }

    #[tokio::test]
    async fn test_get_open_mining_rounds_invalid_token() {
        let raw_data = r#"{"open_mining_rounds":[{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound","contract_id":"00ee4d0e493626b3b87b2c353eb78958344279e5719f251f249ba86588d82a63ddca121220c9949cfed1aef643a9942df76edfd670114af10543cd4aab4aec0ad285e19e90","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","tickDuration":{"microseconds":"600000000"},"issuingFor":{"microseconds":"11354400000000"},"amuletPrice":"0.17","issuanceConfig":{"validatorRewardPercentage":"0.05","unfeaturedAppRewardCap":"0.6","appRewardPercentage":"0.15","featuredAppRewardCap":"20000.0","amuletToIssuePerYear":"40000000000.0","validatorRewardCap":"0.2","optValidatorFaucetCap":"570.0"},"opensAt":"2025-11-14T11:59:03.800962Z","transferConfigUsd":{"holdingFee":{"rate":"0.0000190259"},"extraFeaturedAppRewardAmount":"1.0","maxNumInputs":"100","lockHolderFee":{"fee":"0.0"},"createFee":{"fee":"0.0"},"maxNumLockHolders":"50","transferFee":{"initialRate":"0.0","steps":[{"_1":"100.0","_2":"0.0"},{"_1":"1000.0","_2":"0.0"},{"_1":"1000000.0","_2":"0.0"}]},"maxNumOutputs":"100"},"targetClosesAt":"2025-11-14T12:19:03.800962Z","round":{"number":"18924"}},"created_event_blob":"CgMyLjESpAcKRQDuTQ5JNiazuHssNT63iVg0QnnlcZ8lHySbqGWI2Cpj3coSEiDJlJz+0a72Q6mULfdu39ZwEUrxBUPNSqtK7ArSheGekBINc3BsaWNlLWFtdWxldBpiCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGg9PcGVuTWluaW5nUm91bmQi5wRq5AQKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjYpwIKEAoOMgwwLjE3MDAwMDAwMDAKCwoJKYJoULmMQwYACgsKCSmC9NYAjUMGAAoQCg5qDAoKCggYgKDU7/SUBQqbAgqYAmqVAgoWChRqEgoQCg4yDDAuMDAwMDAwMDAwMAoWChRqEgoQCg4yDDAuMDAwMDE5MDI1OQqkAQqhAWqeAQoQCg4yDDAuMDAwMDAwMDAwMAqJAQqGAVqDAQooaiYKEgoQMg4xMDAuMDAwMDAwMDAwMAoQCg4yDDAuMDAwMDAwMDAwMAopaicKEwoRMg8xMDAwLjAwMDAwMDAwMDAKEAoOMgwwLjAwMDAwMDAwMDAKLGoqChYKFDISMTAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wMDAwMDAwMDAwChYKFGoSChAKDjIMMC4wMDAwMDAwMDAwChAKDjIMMS4wMDAwMDAwMDAwCgUKAxjIAQoFCgMYyAEKBAoCGGQKmAEKlQFqkgEKGgoYMhY0MDAwMDAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wNTAwMDAwMDAwChAKDjIMMC4xNTAwMDAwMDAwChAKDjIMMC4yMDAwMDAwMDAwChQKEjIQMjAwMDAuMDAwMDAwMDAwMAoQCg4yDDAuNjAwMDAwMDAwMAoWChRSEgoQMg41NzAuMDAwMDAwMDAwMAoOCgxqCgoICgYYgJiavAQqSURTTzo6MTIyMGJlNThjMjllNjVkZTQwYmYyNzNiZTFkYzJiMjY2ZDQzYTlhMDAyZWE1YjE4OTU1YWVlZjdhYWM4ODFiYjQ3MWE5giKNlYxDBgBCKgomCiQIARIgrGBWa9jHkdIoDqQoXpzT7ozrA+6vm6XqJ+ZjvJVoik0QHg==","created_at":"2025-11-14T11:49:03.800962Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"},{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound","contract_id":"008e2cfec8d53a4665623b44057825aaaeffa3dc56bf7b0b69887d9cf2b5479368ca121220cde815859b7f3315aee5d99f75228864f6130f926e0078ff1afa78bf4478c945","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","tickDuration":{"microseconds":"600000000"},"issuingFor":{"microseconds":"11355000000000"},"amuletPrice":"0.17","issuanceConfig":{"validatorRewardPercentage":"0.05","unfeaturedAppRewardCap":"0.6","appRewardPercentage":"0.15","featuredAppRewardCap":"20000.0","amuletToIssuePerYear":"40000000000.0","validatorRewardCap":"0.2","optValidatorFaucetCap":"570.0"},"opensAt":"2025-11-14T12:09:16.169761Z","transferConfigUsd":{"holdingFee":{"rate":"0.0000190259"},"extraFeaturedAppRewardAmount":"1.0","maxNumInputs":"100","lockHolderFee":{"fee":"0.0"},"createFee":{"fee":"0.0"},"maxNumLockHolders":"50","transferFee":{"initialRate":"0.0","steps":[{"_1":"100.0","_2":"0.0"},{"_1":"1000.0","_2":"0.0"},{"_1":"1000000.0","_2":"0.0"}]},"maxNumOutputs":"100"},"targetClosesAt":"2025-11-14T12:29:16.169761Z","round":{"number":"18925"}},"created_event_blob":"CgMyLjESpAcKRQCOLP7I1TpGZWI7RAV4Jaqu/6PcVr97C2mIfZzytUeTaMoSEiDN6BWFm38zFa7l2Z91Iohk9hMPkm4AeP8a+ni/RHjJRRINc3BsaWNlLWFtdWxldBpiCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGg9PcGVuTWluaW5nUm91bmQi5wRq5AQKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjapwIKEAoOMgwwLjE3MDAwMDAwMDAKCwoJKSFq0N2MQwYACgsKCSkh9lYljUMGAAoQCg5qDAoKCggYgLjuq/mUBQqbAgqYAmqVAgoWChRqEgoQCg4yDDAuMDAwMDAwMDAwMAoWChRqEgoQCg4yDDAuMDAwMDE5MDI1OQqkAQqhAWqeAQoQCg4yDDAuMDAwMDAwMDAwMAqJAQqGAVqDAQooaiYKEgoQMg4xMDAuMDAwMDAwMDAwMAoQCg4yDDAuMDAwMDAwMDAwMAopaicKEwoRMg8xMDAwLjAwMDAwMDAwMDAKEAoOMgwwLjAwMDAwMDAwMDAKLGoqChYKFDISMTAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wMDAwMDAwMDAwChYKFGoSChAKDjIMMC4wMDAwMDAwMDAwChAKDjIMMS4wMDAwMDAwMDAwCgUKAxjIAQoFCgMYyAEKBAoCGGQKmAEKlQFqkgEKGgoYMhY0MDAwMDAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wNTAwMDAwMDAwChAKDjIMMC4xNTAwMDAwMDAwChAKDjIMMC4yMDAwMDAwMDAwChQKEjIQMjAwMDAuMDAwMDAwMDAwMAoQCg4yDDAuNjAwMDAwMDAwMAoWChRSEgoQMg41NzAuMDAwMDAwMDAwMAoOCgxqCgoICgYYgJiavAQqSURTTzo6MTIyMGJlNThjMjllNjVkZTQwYmYyNzNiZTFkYzJiMjY2ZDQzYTlhMDAyZWE1YjE4OTU1YWVlZjdhYWM4ODFiYjQ3MWE5ISQNuoxDBgBCKgomCiQIARIgTuqeZSs96Te+yq+9iJxvT9wSDDnJCQbG7v/w2OLHDHwQHg==","created_at":"2025-11-14T11:59:16.169761Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"},{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:OpenMiningRound","contract_id":"0083e36afe571597d901f465052f8eeb92a59bb7cbfbc86fdd32864c5c8c8f860eca1212202f6c3c987668937b357053a08adb2b24737f167b8faf20cc9e455d801b403ccf","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","tickDuration":{"microseconds":"600000000"},"issuingFor":{"microseconds":"11355600000000"},"amuletPrice":"0.17","issuanceConfig":{"validatorRewardPercentage":"0.05","unfeaturedAppRewardCap":"0.6","appRewardPercentage":"0.15","featuredAppRewardCap":"20000.0","amuletToIssuePerYear":"40000000000.0","validatorRewardCap":"0.2","optValidatorFaucetCap":"570.0"},"opensAt":"2025-11-14T12:19:28.457485Z","transferConfigUsd":{"holdingFee":{"rate":"0.0000190259"},"extraFeaturedAppRewardAmount":"1.0","maxNumInputs":"100","lockHolderFee":{"fee":"0.0"},"createFee":{"fee":"0.0"},"maxNumLockHolders":"50","transferFee":{"initialRate":"0.0","steps":[{"_1":"100.0","_2":"0.0"},{"_1":"1000.0","_2":"0.0"},{"_1":"1000000.0","_2":"0.0"}]},"maxNumOutputs":"100"},"targetClosesAt":"2025-11-14T12:39:28.457485Z","round":{"number":"18926"}},"created_event_blob":"CgMyLjESpAcKRQCD42r+VxWX2QH0ZQUvjuuSpZu3y/vIb90yhkxcjI+GDsoSEiAvbDyYdmiTezVwU6CK2yskc38We4+vIMyeRV2AG0A8zxINc3BsaWNlLWFtdWxldBpiCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGg9PcGVuTWluaW5nUm91bmQi5wRq5AQKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjcpwIKEAoOMgwwLjE3MDAwMDAwMDAKCwoJKQ0vTwKNQwYACgsKCSkNu9VJjUMGAAoQCg5qDAoKCggYgNCI6P2UBQqbAgqYAmqVAgoWChRqEgoQCg4yDDAuMDAwMDAwMDAwMAoWChRqEgoQCg4yDDAuMDAwMDE5MDI1OQqkAQqhAWqeAQoQCg4yDDAuMDAwMDAwMDAwMAqJAQqGAVqDAQooaiYKEgoQMg4xMDAuMDAwMDAwMDAwMAoQCg4yDDAuMDAwMDAwMDAwMAopaicKEwoRMg8xMDAwLjAwMDAwMDAwMDAKEAoOMgwwLjAwMDAwMDAwMDAKLGoqChYKFDISMTAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wMDAwMDAwMDAwChYKFGoSChAKDjIMMC4wMDAwMDAwMDAwChAKDjIMMS4wMDAwMDAwMDAwCgUKAxjIAQoFCgMYyAEKBAoCGGQKmAEKlQFqkgEKGgoYMhY0MDAwMDAwMDAwMC4wMDAwMDAwMDAwChAKDjIMMC4wNTAwMDAwMDAwChAKDjIMMC4xNTAwMDAwMDAwChAKDjIMMC4yMDAwMDAwMDAwChQKEjIQMjAwMDAuMDAwMDAwMDAwMAoQCg4yDDAuNjAwMDAwMDAwMAoWChRSEgoQMg41NzAuMDAwMDAwMDAwMAoOCgxqCgoICgYYgJiavAQqSURTTzo6MTIyMGJlNThjMjllNjVkZTQwYmYyNzNiZTFkYzJiMjY2ZDQzYTlhMDAyZWE1YjE4OTU1YWVlZjdhYWM4ODFiYjQ3MWE5DemL3oxDBgBCKgomCiQIARIgcSPw4GiqNJn9xbwEBHUwblPQVlCg2/V9rBaafJTxUqUQHg==","created_at":"2025-11-14T12:09:28.457485Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"}],"issuing_mining_rounds":[{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:IssuingMiningRound","contract_id":"001afefd5b020fa285dc091238816b3b388c4b45e8e78d2cd5ce8ebb0181669ec3ca121220eff1ebcb0b8d803a8813b3e802f4c1610e6c5d3852c75611248db658eb5119c2","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","optIssuancePerValidatorFaucetCoupon":"138.8750013888","issuancePerFeaturedAppRewardCoupon":"4851.5981734966","opensAt":"2025-11-14T11:49:53.164873Z","issuancePerSvRewardCoupon":"0.2409291674","targetClosesAt":"2025-11-14T12:09:53.164873Z","issuancePerUnfeaturedAppRewardCoupon":"0.6","round":{"number":"18920"},"issuancePerValidatorRewardCoupon":"0.2"},"created_event_blob":"CgMyLjESnQQKRQAa/v1bAg+ihdwJEjiBazs4jEtF6OeNLNXOjrsBgWaew8oSEiDv8evLC42AOogTs+gC9MFhDmxdOFLHVhEkjbZY61EZwhINc3BsaWNlLWFtdWxldBplCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGhJJc3N1aW5nTWluaW5nUm91bmQi3QFq2gEKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjQpwIKEAoOMgwwLjIwMDAwMDAwMDAKEwoRMg80ODUxLjU5ODE3MzQ5NjYKEAoOMgwwLjYwMDAwMDAwMDAKEAoOMgwwLjI0MDkyOTE2NzQKCwoJKUlefpiMQwYACgsKCSlJ6gTgjEMGAAoWChRSEgoQMg4xMzguODc1MDAxMzg4OCpJRFNPOjoxMjIwYmU1OGMyOWU2NWRlNDBiZjI3M2JlMWRjMmIyNjZkNDNhOWEwMDJlYTViMTg5NTVhZWVmN2FhYzg4MWJiNDcxYTlJGLt0jEMGAEIqCiYKJAgBEiD0hBLUcFUtiz0kQ6+vDQO01u3whc1wARJvtK30xkAMdRAe","created_at":"2025-11-14T11:39:53.164873Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"},{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:IssuingMiningRound","contract_id":"00327833a6edf43fc57e91ee50c7dd8b998dcc7964ee1a9137c5b40b517bdfce89ca12122057e93045037edaa502f7606c2821bba80c95672c633576741f5a47ae67f4105d","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","optIssuancePerValidatorFaucetCoupon":"138.5658558327","issuancePerFeaturedAppRewardCoupon":"4851.5981734966","opensAt":"2025-11-14T11:59:56.992827Z","issuancePerSvRewardCoupon":"0.2409291674","targetClosesAt":"2025-11-14T12:19:56.992827Z","issuancePerUnfeaturedAppRewardCoupon":"0.6","round":{"number":"18921"},"issuancePerValidatorRewardCoupon":"0.2"},"created_event_blob":"CgMyLjESnQQKRQAyeDOm7fQ/xX6R7lDH3YuZjcx5ZO4akTfFtAtRe9/OicoSEiBX6TBFA37apQL3YGwoIbuoDJVnLGM1dnQfWkeuZ/QQXRINc3BsaWNlLWFtdWxldBplCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGhJJc3N1aW5nTWluaW5nUm91bmQi3QFq2gEKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjSpwIKEAoOMgwwLjIwMDAwMDAwMDAKEwoRMg80ODUxLjU5ODE3MzQ5NjYKEAoOMgwwLjYwMDAwMDAwMDAKEAoOMgwwLjI0MDkyOTE2NzQKCwoJKTsNfLyMQwYACgsKCSk7mQIEjUMGAAoWChRSEgoQMg4xMzguNTY1ODU1ODMyNypJRFNPOjoxMjIwYmU1OGMyOWU2NWRlNDBiZjI3M2JlMWRjMmIyNjZkNDNhOWEwMDJlYTViMTg5NTVhZWVmN2FhYzg4MWJiNDcxYTk7x7iYjEMGAEIqCiYKJAgBEiAuEv3Man8aIcXCd5/vJ3eGm+MGU3mLpYIemYblfgHwYRAe","created_at":"2025-11-14T11:49:56.992827Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"},{"contract":{"template_id":"3ca1343ab26b453d38c8adb70dca5f1ead8440c42b59b68f070786955cbf9ec1:Splice.Round:IssuingMiningRound","contract_id":"001457a4c3f3adde0bb35b92bd2122954b57ee3f7011db13eeda62389252ad587dca121220fb88609e83eb96de810be5ce92a945aa35216edaffa9fdd8708da84a2aef2c9f","payload":{"dso":"DSO::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a","optIssuancePerValidatorFaucetCoupon":"139.3837010275","issuancePerFeaturedAppRewardCoupon":"4851.5981734966","opensAt":"2025-11-14T12:10:15.379449Z","issuancePerSvRewardCoupon":"0.2409291674","targetClosesAt":"2025-11-14T12:30:15.379449Z","issuancePerUnfeaturedAppRewardCoupon":"0.6","round":{"number":"18922"},"issuancePerValidatorRewardCoupon":"0.2"},"created_event_blob":"CgMyLjESnQQKRQAUV6TD863eC7Nbkr0hIpVLV+4/cBHbE+7aYjiSUq1YfcoSEiD7iGCeg+uW3oEL5c6SqUWqNSFu2v+p/dhwjahKKu8snxINc3BsaWNlLWFtdWxldBplCkAzY2ExMzQzYWIyNmI0NTNkMzhjOGFkYjcwZGNhNWYxZWFkODQ0MGM0MmI1OWI2OGYwNzA3ODY5NTVjYmY5ZWMxEgZTcGxpY2USBVJvdW5kGhJJc3N1aW5nTWluaW5nUm91bmQi3QFq2gEKTQpLOklEU086OjEyMjBiZTU4YzI5ZTY1ZGU0MGJmMjczYmUxZGMyYjI2NmQ0M2E5YTAwMmVhNWIxODk1NWFlZWY3YWFjODgxYmI0NzFhCgwKCmoICgYKBBjUpwIKEAoOMgwwLjIwMDAwMDAwMDAKEwoRMg80ODUxLjU5ODE3MzQ5NjYKEAoOMgwwLjYwMDAwMDAwMDAKEAoOMgwwLjI0MDkyOTE2NzQKCwoJKfnhV+GMQwYACgsKCSn5bd4ojUMGAAoWChRSEgoQMg4xMzkuMzgzNzAxMDI3NSpJRFNPOjoxMjIwYmU1OGMyOWU2NWRlNDBiZjI3M2JlMWRjMmIyNjZkNDNhOWEwMDJlYTViMTg5NTVhZWVmN2FhYzg4MWJiNDcxYTn5m5S9jEMGAEIqCiYKJAgBEiA0LiBZ/9ukH57fmEC5eQOxFRFAwmFcz15OEV5gYP5DPxAe","created_at":"2025-11-14T12:00:15.379449Z"},"domain_id":"global-domain::1220be58c29e65de40bf273be1dc2b266d43a9a002ea5b18955aeef7aac881bb471a"}]}"#;

        let response: OpenMiningRoundsWrapper = serde_json::from_str(raw_data).unwrap();
        assert_eq!(response.open_mining_rounds.len(), 3);
        assert_eq!(response.issuing_mining_rounds.len(), 3);
    }
}
