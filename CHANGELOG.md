# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `keycloak::login::token_url(host, realm)` and
  `keycloak::login::master_token_url(host)` — build OIDC token endpoint URLs
  using the Keycloak 17+ (Quarkus distribution) path layout, which omits the
  `/auth` context root. Use these for default Keycloak 17+ deployments where
  realm endpoints are served at `/realms/{realm}/...`. The existing
  `client_credentials_url`, `password_url`, and `password_master_url`
  helpers are unchanged and continue to emit the legacy `/auth/realms/...`
  paths for backwards compatibility.

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
