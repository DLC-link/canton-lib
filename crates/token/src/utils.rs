use common::decimal::DamlDecimal;

/// Extract amount from a contract's interface views
pub fn extract_amount(contract: &ledger::models::JsActiveContract) -> Option<DamlDecimal> {
    if let Some(views) = &contract.created_event.interface_views {
        for view in views {
            if let Some(Some(value)) = &view.view_value {
                if let Some(amount_value) = value.get("amount") {
                    if let Some(amount_str) = amount_value.as_str() {
                        return DamlDecimal::parse(amount_str).ok();
                    }
                }
            }
        }
    }
    None
}

/// Fetch all pending CBTC TransferInstruction contracts for a party where the party is the receiver
pub async fn fetch_incoming_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    fetch_transfers(
        ledger_host,
        party,
        access_token,
        TransferDirection::Incoming,
    )
    .await
}

/// Fetch all pending CBTC TransferInstruction contracts for a party where the party is the sender
pub async fn fetch_outgoing_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
) -> Result<Vec<ledger::models::JsActiveContract>, String> {
    fetch_transfers(
        ledger_host,
        party,
        access_token,
        TransferDirection::Outgoing,
    )
    .await
}

enum TransferDirection {
    Incoming,
    Outgoing,
}

/// Fetch all pending CBTC TransferInstruction contracts for a party
async fn fetch_transfers(
    ledger_host: String,
    party: String,
    access_token: String,
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

    // Filter for CBTC transfers based on direction
    let filtered: Vec<ledger::models::JsActiveContract> = result
        .into_iter()
        .filter(|ac| {
            if let Some(create_arg) = &ac.created_event.create_argument {
                if let Some(transfer) = create_arg.get("transfer") {
                    // Check if instrumentId is CBTC
                    let is_cbtc = if let Some(instrument_id) = transfer.get("instrumentId") {
                        if let Some(id) = instrument_id.get("id") {
                            if let Some(id_str) = id.as_str() {
                                id_str.to_lowercase() == "cbtc"
                            } else {
                                false
                            }
                        } else {
                            false
                        }
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

                    return is_cbtc && matches_direction;
                }
            }
            false
        })
        .collect();

    Ok(filtered)
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
    pub fn exercised_event_value(
        template_id: &str,
        choice: &str,
        exercise_result: Value,
    ) -> Value {
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
