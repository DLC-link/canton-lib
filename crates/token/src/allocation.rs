use crate::active_contracts;
use registry::allocation_context::AllocationChoice;
use std::collections::HashMap;

/// Parameters for allocating a transfer leg into a settlement.
///
/// The sender locks holdings for one leg of a Delivery-versus-Payment
/// settlement via the registry's `AllocationFactory_Allocate` choice. The leg is
/// later settled atomically (with the other legs) by the settlement executor.
pub struct Params {
    /// The allocation specification: the shared settlement plus this leg.
    pub allocation: common::allocation::AllocationSpecification,
    /// `requestedAt` timestamp for the allocate choice (RFC3339).
    pub requested_at: String,
    /// Holdings to fund the allocation. If empty, the sender's holdings are
    /// auto-selected from the ledger (the factory merges/splits as needed).
    pub input_holding_cids: Vec<String>,
    pub ledger_host: String,
    pub access_token: String,
    pub registry_url: String,
    pub decentralized_party_id: String,
}

/// Parameters for exercising a choice on an existing allocation contract
/// (`Allocation_ExecuteTransfer`, `Allocation_Withdraw`, or `Allocation_Cancel`).
pub struct ActionParams {
    /// Contract id of the allocation to act on.
    pub allocation_contract_id: String,
    /// The party submitting the action (`actAs`). For `execute_transfer` this is
    /// the settlement executor; for `withdraw`/`cancel`, typically the sender.
    pub actor_party: String,
    pub ledger_host: String,
    pub access_token: String,
    pub registry_url: String,
    pub decentralized_party_id: String,
}

/// Allocate a transfer leg: lock the sender's holdings into a settlement leg via
/// the registry's `AllocationFactory_Allocate` choice.
///
/// Mirrors [`crate::transfer::submit`]: it fetches the allocation factory and
/// choice context from the registry, threads the returned context and disclosed
/// contracts into the exercise command, and submits as the leg sender.
///
/// # Errors
///
/// Returns an error string if holding selection, the registry request, or the
/// ledger submission fails.
pub async fn allocate(params: Params) -> Result<(), String> {
    // Auto-select the sender's holdings when none were provided.
    let mut input_holding_cids = params.input_holding_cids;
    if input_holding_cids.is_empty() {
        let contracts = active_contracts::get(active_contracts::Params {
            ledger_host: params.ledger_host.clone(),
            party: params.allocation.transfer_leg.sender.clone(),
            access_token: params.access_token.clone(),
        })
        .await?;
        input_holding_cids = contracts
            .into_iter()
            .map(|contract| contract.created_event.contract_id)
            .collect();
    }

    let factory = registry::allocation_factory::get(registry::allocation_factory::Params {
        registry_url: params.registry_url,
        decentralized_party_id: params.decentralized_party_id.clone(),
        request: registry::allocation_factory::Request {
            choice_arguments: common::allocation_factory::ChoiceArguments {
                expected_admin: params.decentralized_party_id.clone(),
                allocation: params.allocation.clone(),
                requested_at: params.requested_at.clone(),
                input_holding_cids: input_holding_cids.clone(),
                extra_args: empty_extra_args(),
            },
            exclude_debug_fields: true,
        },
    })
    .await?;

    let sender = params.allocation.transfer_leg.sender.clone();
    let exercise_command = build_allocate_command(
        factory.factory_id,
        params.decentralized_party_id,
        params.allocation,
        params.requested_at,
        input_holding_cids,
        factory.choice_context.choice_context_data,
    );

    let submission_request = common::submission::Submission {
        act_as: vec![sender],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: factory.choice_context.disclosed_contracts,
        commands: vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
        ..Default::default()
    };

    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: params.ledger_host,
        access_token: params.access_token,
        request: submission_request,
    })
    .await?;

    Ok(())
}

/// Execute the transfer of an allocated leg (`Allocation_ExecuteTransfer`).
///
/// Submitted by the settlement executor. A coordinating app normally settles all
/// legs of a settlement together in one transaction; this exposes the single-leg
/// choice for that purpose.
///
/// # Errors
///
/// Returns an error string if the registry request or ledger submission fails.
pub async fn execute_transfer(params: ActionParams) -> Result<(), String> {
    exercise_allocation_choice(
        AllocationChoice::ExecuteTransfer,
        "Allocation_ExecuteTransfer",
        params,
    )
    .await
}

/// Withdraw a pending allocation (`Allocation_Withdraw`), reclaiming the locked
/// holdings. Submitted unilaterally by the leg sender before settlement.
///
/// # Errors
///
/// Returns an error string if the registry request or ledger submission fails.
pub async fn withdraw(params: ActionParams) -> Result<(), String> {
    exercise_allocation_choice(AllocationChoice::Withdraw, "Allocation_Withdraw", params).await
}

/// Cancel an allocation (`Allocation_Cancel`), releasing the locked holdings
/// back to the sender.
///
/// # Errors
///
/// Returns an error string if the registry request or ledger submission fails.
pub async fn cancel(params: ActionParams) -> Result<(), String> {
    exercise_allocation_choice(AllocationChoice::Cancel, "Allocation_Cancel", params).await
}

/// Shared implementation for the three choices exercised on an existing
/// allocation. Fetches the choice context for `choice` from the registry, builds
/// the `daml_choice` exercise command, and submits as `actor_party`.
async fn exercise_allocation_choice(
    choice: AllocationChoice,
    daml_choice: &str,
    params: ActionParams,
) -> Result<(), String> {
    let context = registry::allocation_context::get(registry::allocation_context::Params {
        registry_url: params.registry_url,
        decentralized_party_id: params.decentralized_party_id.clone(),
        allocation_contract_id: params.allocation_contract_id.clone(),
        choice,
        request: registry::allocation_context::Request {
            meta: registry::allocation_context::Meta {
                values: String::new(),
            },
        },
    })
    .await?;

    let exercise_command = build_action_command(
        params.allocation_contract_id,
        daml_choice,
        context.choice_context_data.values,
    );

    let submission_request = common::submission::Submission {
        act_as: vec![params.actor_party],
        read_as: None,
        command_id: uuid::Uuid::new_v4().to_string(),
        disclosed_contracts: context.disclosed_contracts,
        commands: vec![common::submission::Command::ExerciseCommand(
            exercise_command,
        )],
        ..Default::default()
    };

    ledger::submit::wait_for_transaction(ledger::submit::Params {
        ledger_host: params.ledger_host,
        access_token: params.access_token,
        request: submission_request,
    })
    .await?;

    Ok(())
}

/// An empty `extraArgs` (empty context and meta), as required on the allocate
/// request before the registry fills in the choice context.
fn empty_extra_args() -> common::transfer_factory::ExtraArgs {
    common::transfer_factory::ExtraArgs {
        context: common::transfer_factory::Context {
            values: HashMap::new(),
        },
        meta: common::transfer_factory::Meta {
            values: common::transfer_factory::MetaValue {},
        },
    }
}

/// Build the `AllocationFactory_Allocate` exercise command, threading the
/// registry-provided `context` into the choice's `extraArgs`.
fn build_allocate_command(
    factory_id: String,
    expected_admin: String,
    allocation: common::allocation::AllocationSpecification,
    requested_at: String,
    input_holding_cids: Vec<String>,
    context: common::transfer_factory::Context,
) -> common::submission::ExerciseCommand {
    common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_ALLOCATION_FACTORY.to_string(),
            contract_id: factory_id,
            choice: "AllocationFactory_Allocate".to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::AllocationFactory(
                common::allocation_factory::ChoiceArguments {
                    expected_admin,
                    allocation,
                    requested_at,
                    input_holding_cids,
                    extra_args: common::transfer_factory::ExtraArgs {
                        context,
                        meta: common::transfer_factory::Meta {
                            values: common::transfer_factory::MetaValue {},
                        },
                    },
                },
            ),
        },
    }
}

/// Build an `Allocation_*` exercise command (`daml_choice`) on an existing
/// allocation. These choices take only `extraArgs`, so they reuse the generic
/// accept-style choice-argument shape with the registry-provided context.
fn build_action_command(
    allocation_contract_id: String,
    daml_choice: &str,
    context_values: serde_json::Value,
) -> common::submission::ExerciseCommand {
    common::submission::ExerciseCommand {
        exercise_command: common::submission::ExerciseCommandData {
            template_id: common::consts::TEMPLATE_ALLOCATION.to_string(),
            contract_id: allocation_contract_id,
            choice: daml_choice.to_string(),
            choice_argument: common::submission::ChoiceArgumentsVariations::Accept(
                common::accept::ChoiceArguments {
                    extra_args: common::accept::ExtraArgs {
                        context: common::accept::Context {
                            values: context_values,
                        },
                        meta: common::accept::Meta {
                            values: common::accept::MetaValue {},
                        },
                    },
                },
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::submission::ChoiceArgumentsVariations;

    fn sample_allocation() -> common::allocation::AllocationSpecification {
        common::allocation::AllocationSpecification {
            settlement: common::allocation::SettlementInfo {
                executor: "venue".to_string(),
                settlement_ref: common::allocation::Reference {
                    id: "ref".to_string(),
                    cid: None,
                },
                requested_at: "2024-01-01T00:00:00Z".to_string(),
                allocate_before: "2024-01-02T00:00:00Z".to_string(),
                settle_before: "2024-01-03T00:00:00Z".to_string(),
                meta: common::allocation::Metadata::default(),
            },
            transfer_leg_id: "leg0".to_string(),
            transfer_leg: common::allocation::TransferLeg {
                sender: "sender".to_string(),
                receiver: "receiver".to_string(),
                amount: common::decimal::DamlDecimal::parse("0.1").unwrap(),
                instrument_id: common::transfer::InstrumentId {
                    admin: "admin".to_string(),
                    id: "CBTC".to_string(),
                },
                meta: common::allocation::Metadata::default(),
            },
        }
    }

    #[test]
    fn allocate_command_wires_factory_choice_and_fields() {
        let mut ctx_values = HashMap::new();
        ctx_values.insert(
            "instrument-configuration".to_string(),
            common::transfer_factory::ContextValue::ContractId("00cfg".to_string()),
        );
        let command = build_allocate_command(
            "00factory".to_string(),
            "admin".to_string(),
            sample_allocation(),
            "2024-01-01T00:00:00Z".to_string(),
            vec!["cid1".to_string(), "cid2".to_string()],
            common::transfer_factory::Context { values: ctx_values },
        );

        assert_eq!(
            command.exercise_command.template_id,
            common::consts::TEMPLATE_ALLOCATION_FACTORY
        );
        assert_eq!(command.exercise_command.contract_id, "00factory");
        assert_eq!(
            command.exercise_command.choice,
            "AllocationFactory_Allocate"
        );

        match command.exercise_command.choice_argument {
            ChoiceArgumentsVariations::AllocationFactory(args) => {
                assert_eq!(args.expected_admin, "admin");
                assert_eq!(args.requested_at, "2024-01-01T00:00:00Z");
                assert_eq!(
                    args.input_holding_cids,
                    vec!["cid1".to_string(), "cid2".to_string()]
                );
                assert_eq!(args.allocation.transfer_leg_id, "leg0");
                // The registry-provided factory context must be threaded through.
                assert!(
                    args.extra_args
                        .context
                        .values
                        .contains_key("instrument-configuration")
                );
            }
            _ => panic!("expected AllocationFactory choice argument"),
        }
    }

    #[test]
    fn action_commands_wire_choice_and_context() {
        for daml_choice in [
            "Allocation_ExecuteTransfer",
            "Allocation_Withdraw",
            "Allocation_Cancel",
        ] {
            let command = build_action_command(
                "00alloc".to_string(),
                daml_choice,
                serde_json::json!({ "context-key": "context-value" }),
            );

            assert_eq!(
                command.exercise_command.template_id,
                common::consts::TEMPLATE_ALLOCATION
            );
            assert_eq!(command.exercise_command.contract_id, "00alloc");
            assert_eq!(command.exercise_command.choice, daml_choice);

            match command.exercise_command.choice_argument {
                ChoiceArgumentsVariations::Accept(args) => {
                    // The registry-provided choice context must pass through untouched.
                    assert_eq!(
                        args.extra_args.context.values,
                        serde_json::json!({ "context-key": "context-value" })
                    );
                }
                _ => panic!("expected Accept choice argument"),
            }
        }
    }
}
