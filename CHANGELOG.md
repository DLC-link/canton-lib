# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-09-01

### Added

- Token Standard V2 on the transfer path. A caller now chooses the registry's
  V1 or V2 API per call, at feature parity. V1 items stay exactly where they
  are; every V2 item lives in a `v2` submodule beside its V1 twin, so no V1
  path changes shape.
  - `common::transfer::v2` — `Account` (`owner`/`provider`/`id`, both
    `Optional` fields serializing as explicit JSON nulls, as the Daml JSON
    encoding requires) and `Transfer`, whose `sender` and `receiver` are
    accounts rather than bare parties. `Account::basic(owner)` builds the
    unlabelled account a V1 party maps to.
  - `common::transfer_factory::v2::ChoiceArguments` — drops `expectedAdmin`,
    adds `actors`. `ExtraArgs`, `Context`, `ContextValue`, `Meta`, `MetaValue`,
    `Response` and `ChoiceContext` are version-neutral and reused unchanged.
  - `common::accept::v2::ChoiceArguments` — `actors` plus `extraArgs`.
  - `common::submission::ChoiceArgumentsVariations::TransferFactoryV2` and
    `::AcceptV2` variants, for building the V2 exercise commands.
  - `registry::transfer_factory::v2::get` — the
    `/transfer-instruction/v2/transfer-factory` route, plus `factory_url` and
    `v2::factory_url` for the V1 and V2 paths.
  - `registry::accept_context::v2::get` — the
    `/transfer-instruction/v2/{id}/choice-contexts/{accept,reject,withdraw}`
    routes, selected by `v2::InstructionChoice`. V2 fetches the choice's own
    context; the V1 path fetches the accept context for all three.
  - `token::{transfer, split, consolidate, accept, reject, cancel_offers,
    distribute, batch}::v2` — a V2 entry point for every V1 one:
    `transfer::v2::{submit, submit_sequential_chained}`, `split::v2::submit`,
    `consolidate::v2::{consolidate_utxos, check_and_consolidate}`,
    `accept::v2::{submit, accept_all}`, `reject::v2::submit`,
    `cancel_offers::v2::{submit, withdraw_batch, withdraw_all}`,
    `distribute::v2::submit` and `batch::v2::submit_from_csv`.
  - `common::TokenStandardVersion` — `V1` (the default) or `V2`.
- `TokenClientConfig.version` — which registry API this client's calls use.
  `TokenStandardVersion::V1` reproduces the behaviour of every release before
  0.7.0. **The struct has no `Default` and this field has none**: adding it
  means every `TokenClientConfig` literal must name it, so no existing caller
  silently changes version.
- `token::active_contracts::Params.account: Option<common::transfer::v2::Account>`
  — when `Some`, keep only the holdings whose account label matches; a V2
  holding carries its label in the metadata of its V1 interface view. `None`
  keeps every holding the party owns, which is what every V1 caller wants and
  exactly what this function did before.

### Notes

- **The one input a V2 entry point rejects that V1 had no way to express** is
  an `Account` with `owner: None` — the registry-managed account shape, used
  for an instrument admin's mint source. Every V2 entry point holding an
  account guards it through one `require_owner` helper and fails with an error
  naming the offending field. V1 carried bare party strings, which cannot be
  absent, so no V1 caller can reach this error.
- The choice names are version-neutral: V2 keeps `TransferFactory_Transfer`,
  `TransferInstruction_Accept`, `TransferInstruction_Reject` and
  `TransferInstruction_Withdraw`. No V2 choice-name constant was added.
- `actors` is derived inside each V2 entry point from the data it already
  holds, never taken from a public `Params`. The registry's `checkActors`
  accepts exactly one set per path and compares by set equality, so any other
  value fails the submission.

## [0.6.1] - 2026-06-24

### Added

- Token-standard allocation (Delivery-versus-Payment) client support, mirroring
  the existing transfer-instruction clients:
  - `common::allocation` — `AllocationSpecification`, `SettlementInfo`,
    `TransferLeg`, `Reference`, and `Metadata` domain types.
  - `common::allocation_factory` — `AllocationFactory_Allocate` choice arguments
    and the `getAllocationFactory` response (`factoryId` + `choiceContext`).
  - `registry::allocation_factory::get` — fetches the allocation factory and
    choice context (`/registry/allocation-instruction/v1/allocation-factory`).
  - `registry::allocation_context::get` — fetches the execute-transfer /
    withdraw / cancel choice contexts
    (`/registry/allocations/v1/{allocationId}/choice-contexts/...`) via the
    `AllocationChoice` selector.
  - `common::consts::TEMPLATE_ALLOCATION_FACTORY` and
    `common::consts::TEMPLATE_ALLOCATION` interface ids.
  - `common::submission::ChoiceArgumentsVariations::AllocationFactory` variant
    for building the `AllocationFactory_Allocate` exercise command.

## [0.6.0] - 2026-05-19

### Added

- `keycloak::login::token_url(host, realm)` and
  `keycloak::login::master_token_url(host)` — build OIDC token endpoint URLs
  using the Keycloak 17+ (Quarkus distribution) path layout, which omits the
  `/auth` context root. Use these for default Keycloak 17+ deployments where
  realm endpoints are served at `/realms/{realm}/...`. The existing
  `client_credentials_url`, `password_url`, and `password_master_url`
  helpers are unchanged and continue to emit the legacy `/auth/realms/...`
  paths for backwards compatibility.

### Changed

- Bumped `canton-api-client` from `3.3.0-0.1.0` to `3.6.0-0.1.0` (regenerated from the Canton 3.6.0 OpenAPI spec). Most spec-level changes are absorbed inside the conversion functions in `common::filters` and `ledger::common` with no caller-visible API changes; the only behavior change is the `ledger_end::get` note below.
- `ledger::ledger_end::get` and `ledger::ledger_end::get_with_client` now return `Err("Ledger end response missing offset")` when the upstream `offset` field is absent. In Canton 3.6 the field became optional on the wire; treating it as a hard error preserves the previous `Response.offset: i64` API surface and surfaces what is almost certainly a server-side bug rather than silently defaulting.

## [0.5.0] - 2026-05-11

### Added

- `ledger::submit::wait_for_transaction(Params)` — calls the flat `POST /v2/commands/submit-and-wait-for-transaction` JSON Ledger API endpoint and returns the raw `JsSubmitAndWaitForTransactionResponse` body (`{ "transaction": { "events": [...] } }`). When `Submission::transaction_format` is unset, builds a default `TransactionFormat { transactionShape: TRANSACTION_SHAPE_LEDGER_EFFECTS, eventFormat: { verbose: true, filtersByParty: <actAs ∪ readAs> → {} } }` to match the behavior the deprecated tree endpoint hardcoded server-side. When set, the caller's value is moved to the top-level request body so it is not double-nested inside `commands`.
- `common::consts::TRANSACTION_SHAPE_LEDGER_EFFECTS` — JSON Ledger API enum constant used by `wait_for_transaction`'s default `TransactionFormat`.

### Deprecated

- `ledger::submit::wait_for_transaction_tree` — still calls `POST /v2/commands/submit-and-wait-for-transaction-tree` and returns the tree-shaped response unchanged, but emits a deprecation warning. The endpoint is removed in Canton 3.5.0; migrate to `wait_for_transaction` before upgrading. Note the response shape changes from `{ "transactionTree": { "eventsById": {...} } }` (with `CreatedTreeEvent` / `ExercisedTreeEvent`) to `{ "transaction": { "events": [...] } }` (with `CreatedEvent` / `ExercisedEvent`), so downstream parsing must be updated as part of the migration.

### Changed

- README "Submit Commands" curl example updated to the flat endpoint and shows the required `transactionFormat` block.

### Migration: `wait_for_transaction_tree` → `wait_for_transaction`

The tree endpoint is removed in Canton 3.5.0. Both functions take the same
`Params { ledger_host, access_token, request: Submission }` and return
`Result<String, String>`, so the call site itself changes only by name — but
the **response body shape changes**, so downstream JSON parsing must be
updated.

**1. Update the call**

```rust
// Before (deprecated, removed in Canton 3.5.0):
let body = ledger::submit::wait_for_transaction_tree(Params {
    ledger_host,
    access_token,
    request: submission,
}).await?;

// After:
let body = ledger::submit::wait_for_transaction(Params {
    ledger_host,
    access_token,
    request: submission,
}).await?;
```

`wait_for_transaction` requires `Submission.act_as` to contain at least one
party when `Submission.transaction_format` is unset (otherwise the default
`filtersByParty` would be empty and the server rejects it with an opaque
error). The tree endpoint accepted an empty `act_as`; if you relied on that,
either populate `act_as` or set `transaction_format` explicitly.

**2. Update response parsing**

Response shape changes from a tree-keyed object map to a flat event array:

```jsonc
// Before — wait_for_transaction_tree response:
{
  "transactionTree": {
    "eventsById": {
      "0": { "CreatedTreeEvent":  { /* ... */ } },
      "1": { "ExercisedTreeEvent": { /* ... */ } }
    }
  }
}

// After — wait_for_transaction response:
{
  "transaction": {
    "events": [
      { "CreatedEvent":   { /* ... */ } },
      { "ExercisedEvent": { /* ... */ } }
    ]
  }
}
```

Concretely: replace `transactionTree.eventsById` lookups with iteration over
`transaction.events`, and rename `CreatedTreeEvent` → `CreatedEvent` and
`ExercisedTreeEvent` → `ExercisedEvent` in your deserialization types.

**3. (Optional) customize `transactionFormat`**

If `Submission::transaction_format` is left unset, `wait_for_transaction`
builds a default equivalent to what the tree endpoint applied server-side:
`transactionShape = TRANSACTION_SHAPE_LEDGER_EFFECTS`, `verbose = true`, and
one empty-filter entry in `filtersByParty` per party in `actAs ∪ readAs`. To
override (e.g. for `ACS_DELTA` shape or party-specific template filters),
set `Submission::transaction_format` before calling.

## [0.4.0] - 2026-04-07

### Added

- `common::decimal::DamlDecimal` newtype wrapper for Daml `Decimal` (Numeric 10) values
  - Validates ≤10 decimal places at construction time
  - Arithmetic (`+`, `-`, `*`, `/`, negation, `+=`, `-=`, `*=`, `/=`) with banker's rounding (HalfEven) on `*` and `/`, matching Daml's behavior
  - Custom serde: serializes as JSON string, deserializes with validation
  - `Display`, `FromStr`, `Sum`, `Copy`, `Eq`, `Ord`, `Hash`
  - `DamlDecimal::ZERO` constant
- `common::decimal::DamlDecimalError` error type for construction/parse failures
- `rust-version = "1.85"` in workspace manifest (required by `edition = "2024"`)

### Changed

- **Breaking:** `Transfer::amount` changed from `String` to `DamlDecimal`

  ```rust
  // Before
  amount: "0.02".to_string(),

  // After
  use common::decimal::DamlDecimal;
  amount: DamlDecimal::parse("0.02").unwrap(),
  ```

- **Breaking:** `ContextValue::Decimal` changed from `f64` to `DamlDecimal`

  ```rust
  // Before
  ContextValue::Decimal(3.14),

  // After
  ContextValue::Decimal(DamlDecimal::parse("3.14").unwrap()),
  ```

- **Breaking:** Wallet `mining_rounds` fields changed from `String` to `DamlDecimal`:
  `amulet_price`, `validator_reward_percentage`, `unfeatured_app_reward_cap`,
  `app_reward_percentage`, `featured_app_reward_cap`, `amulet_to_issue_per_year`,
  `validator_reward_cap`, `extra_featured_app_reward_amount`, `initial_rate`,
  `issuance_per_featured_app_reward_coupon`, `issuance_per_sv_reward_coupon`,
  `issuance_per_unfeatured_app_reward_coupon`, `issuance_per_validator_reward_coupon`

- **Breaking:** Wallet `mining_rounds` Optional Decimal fields changed from `String` to `Option<DamlDecimal>`:
  `opt_validator_faucet_cap`, `opt_issuance_per_validator_faucet_coupon`

- **Breaking:** `wallet` crate now depends on `common` instead of `rust_decimal` directly.
  If you were using `rust_decimal::Decimal` for wallet field types, switch to `common::decimal::DamlDecimal`.

- `rust_decimal` workspace dependency: disabled default features, keeping only `std` and `serde-with-str`

### Removed

- `common::decimal::validate_daml_decimal()` — replaced by `DamlDecimal::new()`
- `common::decimal::validate_context_value()` — no longer needed (type system enforces validity)
- `common::decimal::validate_submission()` — no longer needed (type system enforces validity)
- `common::decimal::parse_daml_decimal()` — replaced by `DamlDecimal::parse()`
- `log` dependency from `common` crate and workspace

## [0.3.1]

### Fixed

- Minor fixes

## [0.3.0]

### Changed

- **Breaking:** `common::transfer::DisclosedContract::template_id` changed from `String` to `Option<String>`

## [0.2.0]

### Changed

- **Breaking:** `common::submission::Submission` gained extra fields (use `..Default::default()`)

## [0.1.0]

### Added

- Initial release
