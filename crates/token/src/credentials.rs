use common::submission;
use ledger::active_contracts;
use ledger::common::{TemplateFilter, TemplateFilterValue, TemplateIdentifierFilter};
use ledger::ledger_end;
use ledger::models::{JsActiveContract, JsSubmitAndWaitForTransactionResponse};
use ledger::submit;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Template IDs for credential-related contracts
const CREDENTIAL_OFFER_TEMPLATE_ID: &str =
    "#utility-credential-app-v0:Utility.Credential.App.V0.Model.Offer:CredentialOffer";
const CREDENTIAL_TEMPLATE_ID: &str =
    "#utility-credential-v0:Utility.Credential.V0.Credential:Credential";
const USER_SERVICE_TEMPLATE_ID: &str =
    "#utility-credential-app-v0:Utility.Credential.App.V0.Service.User:UserService";

/// A claim within a credential (matches Daml Claim type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub subject: String,
    pub property: String,
    pub value: String,
}

/// A credential offer pending acceptance by the holder
#[derive(Debug, Clone)]
pub struct CredentialOffer {
    pub contract_id: String,
    pub template_id: String,
    pub created_event_blob: String,
    pub issuer: String,
    pub holder: String,
    pub id: String,
    pub description: String,
    pub claims: Vec<Claim>,
}

impl CredentialOffer {
    /// Parse a CredentialOffer from a JsActiveContract
    pub fn from_active_contract(contract: &JsActiveContract) -> Result<Self, String> {
        let contract_id = contract.created_event.contract_id.clone();
        let template_id = contract.created_event.template_id.clone();
        let created_event_blob = contract
            .created_event
            .created_event_blob
            .clone()
            .unwrap_or_default();

        let args = contract
            .created_event
            .create_argument
            .as_ref()
            .and_then(|v| v.as_object())
            .ok_or("createArgument is not an object")?;

        let issuer = args
            .get("issuer")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'issuer' field")?
            .to_string();

        let holder = args
            .get("holder")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'holder' field")?
            .to_string();

        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id' field")?
            .to_string();

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'description' field")?
            .to_string();

        let claims = match args.get("claims") {
            None => Vec::new(),
            Some(v) => serde_json::from_value::<Vec<Claim>>(v.clone())
                .map_err(|e| format!("Failed to parse 'claims' field: {e}"))?,
        };

        Ok(Self {
            contract_id,
            template_id,
            created_event_blob,
            issuer,
            holder,
            id,
            description,
            claims,
        })
    }
}

/// An active credential held by the user
#[derive(Debug, Clone)]
pub struct UserCredential {
    pub contract_id: String,
    pub template_id: String,
    pub issuer: String,
    pub holder: String,
    pub id: String,
    pub description: String,
    pub claims: Vec<Claim>,
}

impl UserCredential {
    /// Parse a UserCredential from a JsActiveContract
    pub fn from_active_contract(contract: &JsActiveContract) -> Result<Self, String> {
        let contract_id = contract.created_event.contract_id.clone();
        let template_id = contract.created_event.template_id.clone();

        let args = contract
            .created_event
            .create_argument
            .as_ref()
            .and_then(|v| v.as_object())
            .ok_or("createArgument is not an object")?;

        let issuer = args
            .get("issuer")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'issuer' field")?
            .to_string();

        let holder = args
            .get("holder")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'holder' field")?
            .to_string();

        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'id' field")?
            .to_string();

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or("Missing 'description' field")?
            .to_string();

        let claims = match args.get("claims") {
            None => Vec::new(),
            Some(v) => serde_json::from_value::<Vec<Claim>>(v.clone())
                .map_err(|e| format!("Failed to parse 'claims' field: {e}"))?,
        };

        Ok(Self {
            contract_id,
            template_id,
            issuer,
            holder,
            id,
            description,
            claims,
        })
    }
}

/// Information about a UserService contract
#[derive(Debug, Clone)]
pub struct UserServiceInfo {
    pub contract_id: String,
    pub template_id: String,
    pub operator: String,
    pub user: String,
    pub dso: String,
}

/// Parameters for listing credential offers
pub struct ListCredentialOffersParams {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
}

/// Parameters for listing credentials
pub struct ListCredentialsParams {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
}

/// Parameters for accepting a credential offer
pub struct AcceptCredentialOfferParams {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
    pub user_service_contract_id: String,
    pub user_service_template_id: String,
    pub credential_offer_cid: String,
}

/// Parameters for finding a user's UserService contract
pub struct FindUserServiceParams {
    pub ledger_host: String,
    pub party: String,
    pub access_token: String,
}

/// List all credential offers for a party
pub async fn list_credential_offers(
    params: ListCredentialOffersParams,
) -> Result<Vec<CredentialOffer>, String> {
    let ledger_end_response = ledger_end::get(ledger_end::Params {
        access_token: params.access_token.clone(),
        ledger_host: params.ledger_host.clone(),
    })
    .await?;

    let filter =
        ledger::common::IdentifierFilter::TemplateIdentifierFilter(TemplateIdentifierFilter {
            template_filter: TemplateFilter {
                value: TemplateFilterValue {
                    template_id: Some(CREDENTIAL_OFFER_TEMPLATE_ID.to_string()),
                    include_created_event_blob: true,
                },
            },
        });

    let contracts = active_contracts::get_by_party(active_contracts::Params {
        ledger_host: params.ledger_host,
        party: params.party.clone(),
        filter,
        access_token: params.access_token,
        ledger_end: ledger_end_response.offset,
        unknown_contract_entry_handler: None,
    })
    .await?;

    let offers: Result<Vec<CredentialOffer>, String> = contracts
        .iter()
        .filter(|contract| {
            contract
                .created_event
                .create_argument
                .as_ref()
                .and_then(|v| v.as_object())
                .and_then(|args| args.get("holder"))
                .and_then(|v| v.as_str())
                .map(|holder| holder == params.party)
                .unwrap_or(false)
        })
        .map(CredentialOffer::from_active_contract)
        .collect();

    offers
}

/// List all credentials for a party
pub async fn list_credentials(
    params: ListCredentialsParams,
) -> Result<Vec<UserCredential>, String> {
    let ledger_end_response = ledger_end::get(ledger_end::Params {
        access_token: params.access_token.clone(),
        ledger_host: params.ledger_host.clone(),
    })
    .await?;

    let filter =
        ledger::common::IdentifierFilter::TemplateIdentifierFilter(TemplateIdentifierFilter {
            template_filter: TemplateFilter {
                value: TemplateFilterValue {
                    template_id: Some(CREDENTIAL_TEMPLATE_ID.to_string()),
                    include_created_event_blob: true,
                },
            },
        });

    let contracts = active_contracts::get_by_party(active_contracts::Params {
        ledger_host: params.ledger_host,
        party: params.party.clone(),
        filter,
        access_token: params.access_token,
        ledger_end: ledger_end_response.offset,
        unknown_contract_entry_handler: None,
    })
    .await?;

    let credentials: Result<Vec<UserCredential>, String> = contracts
        .iter()
        .filter(|contract| {
            contract
                .created_event
                .create_argument
                .as_ref()
                .and_then(|v| v.as_object())
                .and_then(|args| args.get("holder"))
                .and_then(|v| v.as_str())
                .map(|holder| holder == params.party)
                .unwrap_or(false)
        })
        .map(UserCredential::from_active_contract)
        .collect();

    credentials
}

/// Find the UserService contract for a party
pub async fn find_user_service(params: FindUserServiceParams) -> Result<UserServiceInfo, String> {
    let ledger_end_response = ledger_end::get(ledger_end::Params {
        access_token: params.access_token.clone(),
        ledger_host: params.ledger_host.clone(),
    })
    .await?;

    let filter =
        ledger::common::IdentifierFilter::TemplateIdentifierFilter(TemplateIdentifierFilter {
            template_filter: TemplateFilter {
                value: TemplateFilterValue {
                    template_id: Some(USER_SERVICE_TEMPLATE_ID.to_string()),
                    include_created_event_blob: true,
                },
            },
        });

    let contracts = active_contracts::get_by_party(active_contracts::Params {
        ledger_host: params.ledger_host,
        party: params.party.clone(),
        filter,
        access_token: params.access_token,
        ledger_end: ledger_end_response.offset,
        unknown_contract_entry_handler: None,
    })
    .await?;

    for contract in &contracts {
        let contract_id = contract.created_event.contract_id.clone();
        let template_id = contract.created_event.template_id.clone();

        let args = match contract
            .created_event
            .create_argument
            .as_ref()
            .and_then(|v| v.as_object())
        {
            Some(a) => a,
            None => continue,
        };

        let user = match args.get("user").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => continue,
        };

        if user == params.party {
            let operator = args
                .get("operator")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let dso = args
                .get("dso")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            return Ok(UserServiceInfo {
                contract_id,
                template_id,
                operator,
                user,
                dso,
            });
        }
    }

    Err(format!(
        "No UserService contract found for party {}. The user must be onboarded to the Canton Network utility first.",
        params.party
    ))
}

/// Accept a credential offer by exercising the UserService_AcceptFreeCredentialOffer choice
pub async fn accept_credential_offer(
    params: AcceptCredentialOfferParams,
) -> Result<UserCredential, String> {
    let command_id = format!("cmd-{}", uuid::Uuid::new_v4());

    let choice_argument = json!({
        "credentialOfferCid": params.credential_offer_cid
    });

    let exercise_command = submission::ExerciseCommand {
        exercise_command: submission::ExerciseCommandData {
            template_id: params.user_service_template_id.clone(),
            contract_id: params.user_service_contract_id.clone(),
            choice: "UserService_AcceptFreeCredentialOffer".to_string(),
            choice_argument: submission::ChoiceArgumentsVariations::Generic(choice_argument),
        },
    };

    let submission_request = submission::Submission {
        act_as: vec![params.party.clone()],
        read_as: None,
        command_id,
        disclosed_contracts: vec![],
        commands: vec![submission::Command::ExerciseCommand(exercise_command)],
        ..Default::default()
    };

    let response_raw = submit::wait_for_transaction(submit::Params {
        ledger_host: params.ledger_host.clone(),
        access_token: params.access_token.clone(),
        request: submission_request,
    })
    .await?;

    let response: JsSubmitAndWaitForTransactionResponse = serde_json::from_str(&response_raw)
        .map_err(|e| format!("Failed to parse submit response: {}", e))?;

    parse_accept_credential_offer_response(&response)
}

/// Extract the newly created `UserCredential` from a flat-shaped submit
/// response for `UserService_AcceptFreeCredentialOffer`.
///
/// Walks `transaction.events`, finds the first `CreatedEvent` whose
/// `templateId` ends with `:Utility.Credential.V0.Credential:Credential`,
/// wraps the typed `CreatedEvent` in a `JsActiveContract` (via
/// `Box::new(c.clone())`) and delegates to `UserCredential::from_active_contract`.
fn parse_accept_credential_offer_response(
    response: &JsSubmitAndWaitForTransactionResponse,
) -> Result<UserCredential, String> {
    let events = &response.transaction.events;

    for event in events {
        if let Some(created) = crate::event_helpers::as_created_event(event) {
            if created
                .template_id
                .ends_with(":Utility.Credential.V0.Credential:Credential")
            {
                let active_contract = JsActiveContract {
                    created_event: Box::new(created.clone()),
                    reassignment_counter: 0,
                    synchronizer_id: String::new(),
                };
                return UserCredential::from_active_contract(&active_contract);
            }
        }
    }

    Err("No Credential contract was created in the transaction".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keycloak::login::{PasswordParams, password, password_url};
    use std::env;

    #[tokio::test]
    async fn test_list_credentials() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = PasswordParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
            password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
            url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = password(params).await.unwrap();

        let credentials = list_credentials(ListCredentialsParams {
            ledger_host,
            party: party_id,
            access_token: login_response.access_token,
        })
        .await
        .expect("Failed to list credentials");

        log::debug!("Found {} credentials", credentials.len());
    }

    #[tokio::test]
    async fn test_list_credential_offers() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = PasswordParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
            password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
            url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = password(params).await.unwrap();

        let offers = list_credential_offers(ListCredentialOffersParams {
            ledger_host,
            party: party_id,
            access_token: login_response.access_token,
        })
        .await
        .expect("Failed to list credential offers");

        log::debug!("Found {} credential offers", offers.len());
    }

    #[tokio::test]
    async fn test_find_user_service() {
        dotenvy::dotenv().ok();

        let ledger_host = env::var("LEDGER_HOST").expect("LEDGER_HOST must be set");
        let party_id = env::var("PARTY_ID").expect("PARTY_ID must be set");

        let params = PasswordParams {
            client_id: env::var("KEYCLOAK_CLIENT_ID").expect("KEYCLOAK_CLIENT_ID must be set"),
            username: env::var("KEYCLOAK_USERNAME").expect("KEYCLOAK_USERNAME must be set"),
            password: env::var("KEYCLOAK_PASSWORD").expect("KEYCLOAK_PASSWORD must be set"),
            url: password_url(
                &env::var("KEYCLOAK_HOST").expect("KEYCLOAK_HOST must be set"),
                &env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set"),
            ),
        };
        let login_response = password(params).await.unwrap();

        let user_service = find_user_service(FindUserServiceParams {
            ledger_host,
            party: party_id.clone(),
            access_token: login_response.access_token,
        })
        .await
        .expect("Failed to find UserService");

        assert_eq!(user_service.user, party_id);
        assert!(!user_service.contract_id.is_empty());
        assert!(!user_service.operator.is_empty());
    }
}

#[cfg(test)]
mod parser_tests {
    //! Pure-data fixture tests for the flat-event parser used by
    //! `accept_credential_offer` (`parse_accept_credential_offer_response`).

    use super::*;
    use crate::utils::test_fixtures::{
        created_event_value, created_event_value_with_blob, exercised_event_value,
        transaction_response,
    };
    use serde_json::json;

    const CRED_TID: &str =
        "pkg-hash:Utility.Credential.V0.Credential:Credential";

    fn credential_create_argument() -> serde_json::Value {
        json!({
            "issuer": "issuer::1220aaaa",
            "holder": "holder::1220bbbb",
            "id": "cred-id-42",
            "description": "Minter role credential",
            "claims": [
                {
                    "subject": "holder::1220bbbb",
                    "property": "hasCBTCRole",
                    "value": "Minter"
                }
            ]
        })
    }

    #[test]
    fn happy_path_builds_user_credential() {
        let response = transaction_response(
            "tx-1",
            json!([
                exercised_event_value(
                    "pkg:Utility.Credential.App.V0.Service.User:UserService",
                    "UserService_AcceptFreeCredentialOffer",
                    json!(null),
                ),
                created_event_value_with_blob(
                    CRED_TID,
                    "00cred-cid",
                    credential_create_argument(),
                    "blob-base64",
                ),
            ]),
        );

        let cred = parse_accept_credential_offer_response(&response).unwrap();
        assert_eq!(cred.contract_id, "00cred-cid");
        assert_eq!(cred.template_id, CRED_TID);
        assert_eq!(cred.issuer, "issuer::1220aaaa");
        assert_eq!(cred.holder, "holder::1220bbbb");
        assert_eq!(cred.id, "cred-id-42");
        assert_eq!(cred.description, "Minter role credential");
        assert_eq!(cred.claims.len(), 1);
        assert_eq!(cred.claims[0].property, "hasCBTCRole");
        assert_eq!(cred.claims[0].value, "Minter");
    }

    #[test]
    fn missing_match_returns_err() {
        // CreatedEvent present but for a different template.
        let response = transaction_response(
            "tx-x",
            json!([created_event_value(
                "pkg:Some.Other:Template",
                "00other",
                json!({}),
            )]),
        );

        let err = parse_accept_credential_offer_response(&response).unwrap_err();
        assert!(
            err.contains("No Credential contract was created"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_events_returns_err() {
        // `events` is required on the wire now, so an empty list stands in for
        // "missing events"; the parser falls through to its post-loop check.
        let response = transaction_response("tx-x", json!(null));
        let err = parse_accept_credential_offer_response(&response).unwrap_err();
        assert!(
            err.contains("No Credential contract was created"),
            "unexpected error: {err}"
        );
    }
}
