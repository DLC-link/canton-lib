use common::decimal::DamlDecimal;

/// Extract amount from a contract's interface views
pub fn extract_amount(contract: &ledger::models::JsActiveContract) -> Option<DamlDecimal> {
    if let Some(views) = &contract.created_event.interface_views {
        for view in views {
            if let Some(Some(value)) = &view.view_value
                && let Some(amount_value) = value.get("amount")
                && let Some(amount_str) = amount_value.as_str()
            {
                return DamlDecimal::parse(amount_str).ok();
            }
        }
    }
    None
}

/// Fetch all pending TransferInstruction contracts of an instrument for a party where the party is the receiver
pub async fn fetch_incoming_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
    instrument_id: common::transfer::InstrumentId,
) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    fetch_transfers(
        ledger_host,
        party,
        access_token,
        instrument_id,
        TransferDirection::Incoming,
    )
    .await
}

/// Fetch all pending TransferInstruction contracts of an instrument for a party where the party is the sender
pub async fn fetch_outgoing_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
    instrument_id: common::transfer::InstrumentId,
) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    fetch_transfers(
        ledger_host,
        party,
        access_token,
        instrument_id,
        TransferDirection::Outgoing,
    )
    .await
}

enum TransferDirection {
    Incoming,
    Outgoing,
}

/// Fetch all pending TransferInstruction contracts of an instrument for a party
async fn fetch_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
    instrument_id: common::transfer::InstrumentId,
    direction: TransferDirection,
) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    use ledger::ledger_end;
    use ledger::websocket::active_contracts;

    // Get current ledger end
    let ledger_end_result = ledger_end::get(ledger_end::Params {
        access_token: access_token.clone(),
        ledger_host: ledger_host.clone(),
    })
    .await?;

    // Fetch all active contracts with TransferInstruction template filter
    let result = active_contracts::get(active_contracts::Params {
        ledger_host,
        party: party.clone(),
        filter: ledger::common::IdentifierFilter::TemplateIdentifierFilter(
            ledger::common::TemplateIdentifierFilter {
                template_filter: ledger::common::TemplateFilter {
                    value: ledger::common::TemplateFilterValue {
                        template_id: Some(common::consts::TEMPLATE_TRANSFER_OFFER.to_string()),
                        include_created_event_blob: true,
                    },
                },
            },
        ),
        access_token,
        ledger_end: ledger_end_result.offset,
    })
    .await?;

    log::debug!(
        "Total active TransferInstruction contracts fetched: {}",
        result.len()
    );

    // Filter for the requested instrument's transfers based on direction
    let filtered: Vec<ledger::models::JsActiveContract> = result
        .into_iter()
        .filter(|ac| {
            if let Some(create_arg) = &ac.created_event.create_argument
                && let Some(transfer) = create_arg.get("transfer")
            {
                // Check if instrumentId matches the requested instrument
                let is_instrument = if let Some(instrument) = transfer.get("instrumentId") {
                    let id_ok = if let Some(id) = instrument.get("id") {
                        if let Some(id_str) = id.as_str() {
                            id_str == instrument_id.id
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    let admin_ok = if let Some(admin) = instrument.get("admin") {
                        if let Some(admin_str) = admin.as_str() {
                            admin_str == instrument_id.admin
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    id_ok && admin_ok
                } else {
                    false
                };

                // Check role based on direction
                let matches_direction = match direction {
                    TransferDirection::Incoming => {
                        // Check if we are the receiver
                        if let Some(receiver) = transfer.get("receiver") {
                            if let Some(receiver_str) = receiver.as_str() {
                                receiver_str == party
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    TransferDirection::Outgoing => {
                        // Check if we are the sender
                        if let Some(sender) = transfer.get("sender") {
                            if let Some(sender_str) = sender.as_str() {
                                sender_str == party
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                };

                return is_instrument && matches_direction;
            }
            false
        })
        .collect();

    Ok(filtered)
}

pub(crate) const REASON_META_KEY: &str = "splice.lfdecentralizedtrust.org/reason";
pub(crate) const TX_KIND_META_KEY: &str = "splice.lfdecentralizedtrust.org/tx-kind";
pub(crate) const REFERENCE_META_KEY: &str = "splice.lfdecentralizedtrust.org/reference";

/// The owner of a regular account.
///
/// `Account.owner` is `None` only for the special accounts an instrument admin
/// manages, such as the source account for a mint. This library never operates
/// one, and every V2 entry point needs the owner: it selects holdings by party
/// and it supplies the `actors` the registry checks. So a `None` owner is a
/// caller error, reported before any network call.
///
/// No V1 path holds an `Account`, so no V1 caller can reach this error.
pub(crate) fn require_owner(
    account: &common::transfer::v2::Account,
    field: &str,
) -> Result<String, String> {
    account.owner.clone().ok_or_else(|| {
        format!("{field}.owner is None: this library cannot operate a registry-managed account")
    })
}

/// Default the `reason` key into a transfer's metadata when the caller supplied
/// none. A supplied metadata passes through untouched.
pub(crate) fn ensure_reason_meta(
    meta: Option<common::transfer::Meta>,
) -> Option<common::transfer::Meta> {
    match meta {
        Some(meta) => Some(meta),
        None => {
            let mut values = std::collections::HashMap::new();
            values.insert(REASON_META_KEY.to_string(), String::new());
            Some(common::transfer::Meta {
                values: Some(values),
            })
        }
    }
}

/// Metadata for a self-transfer, which the registry reads as a merge-split.
pub(crate) fn merge_split_meta(reason: &str) -> common::transfer::Meta {
    let mut values = std::collections::HashMap::new();
    values.insert(REASON_META_KEY.to_string(), reason.to_string());
    values.insert(TX_KIND_META_KEY.to_string(), "merge-split".to_string());
    common::transfer::Meta {
        values: Some(values),
    }
}

/// Metadata for one leg of a sequential chained transfer.
pub(crate) fn chained_transfer_meta(reference: Option<&str>) -> common::transfer::Meta {
    let mut values = std::collections::HashMap::new();
    values.insert(REASON_META_KEY.to_string(), String::new());
    if let Some(reference) = reference {
        values.insert(REFERENCE_META_KEY.to_string(), reference.to_string());
    }
    common::transfer::Meta {
        values: Some(values),
    }
}

/// Build the submission envelope. Version-independent: only the commands and
/// the acting parties differ between V1 and V2.
pub(crate) fn build_submission(
    act_as: Vec<String>,
    disclosed_contracts: Vec<common::transfer::DisclosedContract>,
    commands: Vec<common::submission::Command>,
) -> common::submission::Submission {
    common::submission::Submission {
        act_as,
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts,
        commands,
        ..Default::default()
    }
}

/// Submit and wait for the transaction. Returns the raw response body, which
/// the operation modules parse for holding and instruction contract ids.
pub(crate) async fn submit_and_wait(
    ledger_host: &str,
    access_token: &str,
    submission: common::submission::Submission,
) -> Result<String, String> {
    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: ledger_host.to_string(),
        access_token: access_token.to_string(),
        request: submission,
    })
    .await
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    use serde_json::json;

    /// The exact `Submission` JSON every operation module builds today.
    /// `build_submission` must reproduce it field for field.
    fn expected_submission_json(command_id: &str) -> serde_json::Value {
        json!({
            "actAs": ["alice::1220ab"],
            "commandId": command_id,
            "disclosedContracts": [{
                "templateId": "pkg:Mod:T",
                "contractId": "00disclosed",
                "createdEventBlob": "blob",
                "synchronizerId": "sync::1220"
            }],
            "commands": [{
                "ExerciseCommand": {
                    "templateId": "pkg:Mod:Factory",
                    "contractId": "00factory",
                    "choice": "Some_Choice",
                    "choiceArgument": { "extraArgs": {
                        "context": { "values": {} },
                        "meta": { "values": {} }
                    }}
                }
            }]
        })
    }

    fn fixture_parts() -> (
        Vec<common::transfer::DisclosedContract>,
        Vec<common::submission::Command>,
    ) {
        let disclosed = vec![common::transfer::DisclosedContract {
            template_id: Some("pkg:Mod:T".to_string()),
            contract_id: "00disclosed".to_string(),
            created_event_blob: "blob".to_string(),
            synchronizer_id: "sync::1220".to_string(),
        }];
        let commands = vec![common::submission::Command::ExerciseCommand(
            common::submission::ExerciseCommand {
                exercise_command: common::submission::ExerciseCommandData {
                    template_id: "pkg:Mod:Factory".to_string(),
                    contract_id: "00factory".to_string(),
                    choice: "Some_Choice".to_string(),
                    choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                        common::accept::ChoiceArguments {
                            extra_args: common::accept::ExtraArgs {
                                context: common::accept::Context {
                                    values: serde_json::json!({}),
                                },
                                meta: common::accept::Meta {
                                    values: common::accept::MetaValue {},
                                },
                            },
                        },
                    ),
                },
            },
        )];
        (disclosed, commands)
    }

    #[test]
    fn inline_submission_shape_is_pinned() {
        let (disclosed_contracts, commands) = fixture_parts();
        let submission = common::submission::Submission {
            act_as: vec!["alice::1220ab".to_string()],
            read_as: None,
            command_id: "fixed-command-id".to_string(),
            disclosed_contracts,
            commands,
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_value(&submission).unwrap(),
            expected_submission_json("fixed-command-id")
        );
    }

    #[test]
    fn build_submission_reproduces_the_pinned_shape() {
        let (disclosed_contracts, commands) = fixture_parts();
        let submission = build_submission(
            vec!["alice::1220ab".to_string()],
            disclosed_contracts,
            commands,
        );

        let command_id = submission.command_id.clone();
        assert!(
            uuid::Uuid::parse_str(&command_id).is_ok(),
            "commandId must stay a fresh v4 uuid, got {command_id}"
        );
        assert_eq!(
            serde_json::to_value(&submission).unwrap(),
            expected_submission_json(&command_id)
        );
    }

    #[test]
    fn require_owner_returns_the_owner_of_a_regular_account() {
        let account = common::transfer::v2::Account::basic("alice::1220ab");
        assert_eq!(
            require_owner(&account, "transfer.sender").unwrap(),
            "alice::1220ab"
        );
    }

    #[test]
    fn require_owner_names_the_field_when_the_owner_is_none() {
        let account = common::transfer::v2::Account {
            owner: None,
            provider: None,
            id: String::new(),
        };
        let err = require_owner(&account, "transfer.sender").unwrap_err();
        assert!(
            err.contains("transfer.sender"),
            "the error must name the parameter, got {err}"
        );
    }

    #[test]
    fn ensure_reason_meta_defaults_only_when_absent() {
        let defaulted = ensure_reason_meta(None).expect("must produce meta");
        assert_eq!(
            defaulted.values.unwrap().get(REASON_META_KEY),
            Some(&String::new())
        );

        let mut supplied = std::collections::HashMap::new();
        supplied.insert(REASON_META_KEY.to_string(), "kept".to_string());
        let untouched = ensure_reason_meta(Some(common::transfer::Meta {
            values: Some(supplied),
        }))
        .expect("must pass through");
        assert_eq!(
            untouched.values.unwrap().get(REASON_META_KEY),
            Some(&"kept".to_string()),
            "a caller-supplied meta must not be overwritten"
        );
    }

    #[test]
    fn merge_split_meta_carries_reason_and_tx_kind() {
        let meta = merge_split_meta("merge-split");
        let values = meta.values.expect("must carry values");
        assert_eq!(
            values.get(REASON_META_KEY),
            Some(&"merge-split".to_string())
        );
        assert_eq!(
            values.get(TX_KIND_META_KEY),
            Some(&"merge-split".to_string())
        );

        // The second caller is the reason `reason` is a parameter. Without
        // this case an implementation that ignored the argument and hardcoded
        // "merge-split" into both keys would pass.
        let consolidation = merge_split_meta("UTXO consolidation");
        let values = consolidation.values.expect("must carry values");
        assert_eq!(
            values.get(REASON_META_KEY),
            Some(&"UTXO consolidation".to_string())
        );
        assert_eq!(
            values.get(TX_KIND_META_KEY),
            Some(&"merge-split".to_string()),
            "tx-kind is fixed; only reason varies between the two callers"
        );
    }

    #[test]
    fn chained_transfer_meta_adds_the_reference_only_when_given() {
        let without = chained_transfer_meta(None).values.expect("values");
        assert_eq!(without.get(REASON_META_KEY), Some(&String::new()));
        assert!(!without.contains_key(REFERENCE_META_KEY));

        let with = chained_transfer_meta(Some("ref-1")).values.expect("values");
        assert_eq!(with.get(REFERENCE_META_KEY), Some(&"ref-1".to_string()));
    }
}

#[cfg(test)]
pub(crate) mod test_fixtures {
    //! Helpers for building typed `JsSubmitAndWaitForTransactionResponse`
    //! values for parser unit tests. The canton-api-client model structs
    //! reject responses that lack required fields like `nodeId`,
    //! `createdAt`, `packageName`, `offset`, `synchronizerId`, etc. — these
    //! helpers stamp in dummy values for those so each test fixture only
    //! has to specify the fields it actually exercises.
    use ledger::models::JsSubmitAndWaitForTransactionResponse;
    use serde_json::{Value, json};

    /// Build a flat-event `CreatedEvent` as a JSON value with required
    /// structural fields filled in with placeholders. Pass `create_argument`
    /// as `json!(null)` if the test doesn't care about it.
    pub fn created_event_value(
        template_id: &str,
        contract_id: &str,
        create_argument: Value,
    ) -> Value {
        json!({
            "CreatedEvent": {
                "offset": 1_i64,
                "nodeId": 0_i32,
                "contractId": contract_id,
                "templateId": template_id,
                "createArgument": create_argument,
                "createdEventBlob": "",
                "witnessParties": [],
                "signatories": [],
                "observers": [],
                "createdAt": "1970-01-01T00:00:00Z",
                "packageName": "test-pkg",
                "representativePackageId": "test-pkg",
                "acsDelta": true,
            }
        })
    }

    /// Same as `created_event_value`, but lets the caller override
    /// `createdEventBlob` — used by tests that assert on the blob being
    /// propagated into the resulting domain object.
    pub fn created_event_value_with_blob(
        template_id: &str,
        contract_id: &str,
        create_argument: Value,
        created_event_blob: &str,
    ) -> Value {
        let mut event = created_event_value(template_id, contract_id, create_argument);
        event["CreatedEvent"]["createdEventBlob"] = json!(created_event_blob);
        event
    }

    /// Build a flat-event `ExercisedEvent` as a JSON value with required
    /// structural fields filled in with placeholders. Pass `exercise_result`
    /// as `json!(null)` if the test doesn't care about it.
    pub fn exercised_event_value(template_id: &str, choice: &str, exercise_result: Value) -> Value {
        json!({
            "ExercisedEvent": {
                "offset": 1_i64,
                "nodeId": 0_i32,
                "contractId": "00exercise-target",
                "templateId": template_id,
                "choice": choice,
                "choiceArgument": null,
                "actingParties": [],
                "consuming": true,
                "witnessParties": [],
                "lastDescendantNodeId": 0_i32,
                "exerciseResult": exercise_result,
                "packageName": "test-pkg",
                "acsDelta": true,
            }
        })
    }

    /// Build a `JsSubmitAndWaitForTransactionResponse` from an updateId and
    /// an `events` value. Pass `json!(null)` to construct a response with an
    /// empty events list (the typed model now treats `events` as required, so
    /// "no events" is represented as `[]` rather than an absent field).
    /// Deserializes through the typed model so fixtures fail loudly when the
    /// shape diverges from canton-api-client's schema.
    pub fn transaction_response(
        update_id: &str,
        events: Value,
    ) -> JsSubmitAndWaitForTransactionResponse {
        let events = if events.is_null() { json!([]) } else { events };
        let transaction = json!({
            "updateId": update_id,
            "commandId": "",
            "workflowId": "",
            "effectiveAt": "1970-01-01T00:00:00Z",
            "events": events,
            "offset": 1_i64,
            "synchronizerId": "test-synchronizer",
            "recordTime": "1970-01-01T00:00:00Z",
        });
        let envelope = json!({ "transaction": transaction });
        serde_json::from_value(envelope).expect("test fixture is not a valid response")
    }

    /// Variant of `transaction_response` whose `transaction.update_id` is
    /// set to the empty string, for tests that exercise the "missing
    /// updateId" parser branch.
    ///
    /// `JsTransaction.update_id` is a required `String` in the typed model,
    /// so we can't literally omit it on the wire and still deserialize.
    /// Empty-string is the closest in-band equivalent and is what the
    /// parser's emptiness check is meant to catch.
    pub fn transaction_response_without_update_id(
        events: Value,
    ) -> JsSubmitAndWaitForTransactionResponse {
        let mut response = transaction_response("placeholder", events);
        response.transaction.update_id = String::new();
        response
    }
}
