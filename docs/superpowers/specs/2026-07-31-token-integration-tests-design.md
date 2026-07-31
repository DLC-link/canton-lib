# Token crate integration tests — design

Date: 2026-07-31
Branch: `feat/token/init-token-crate`

## Goal

The workspace gets two test suites with two separate commands:

- Unit suite: `cargo test --workspace`. No network, no env vars, always green.
- Integration suite: `cargo test --workspace -- --ignored --test-threads=1 integration_`.
  This runs only the live integration tests against devnet.

## Existing live tests

Seventeen existing tests need env vars and a live ledger. They fail in a
plain checkout. Each one gets `#[ignore = "live test: requires env vars and
network"]`. They keep their code and env var names. They run in neither
suite, because the integration command filters on the `integration_` name
prefix. You can still run one by naming it.

The tests are in: `token` (active_contracts, batch, consolidate x2,
credentials x3, distribute, split, transfer), `ledger` (active_contracts,
ledger_end, websocket x2), `registry` (transfer_factory), and `wallet`
(amulet_rules, mining_rounds).

## New integration tests

The tests live in `crates/token/src/client.rs`, in a
`#[cfg(test)] mod integration_tests` block. They test only `TokenClient`
methods, the outer package boundary. Each test:

- has a name that starts with `integration_`,
- carries `#[ignore = "integration test: requires live devnet and env vars"]`,
- creates a random `test_uuid` and adds it as the reference on every
  transfer it submits,
- restores both wallet balances when it succeeds.

### Sequential execution

The command passes `--test-threads=1`. Each test also locks a static
`std::sync::Mutex<()>` as its first action, with poison recovery. The tests
never run in parallel, even without the flag.

### Environment

`IntegrationTestState` holds all configuration. `from_env()` loads a `.env`
file when present and panics with a clear message on a missing variable.

| Field | Source |
|---|---|
| `party_1` | `PARTY_ID_1` |
| `party_2` | `PARTY_ID_2` |
| `instrument.admin` | `DECENTRALIZED_PARTY_ID` |
| `instrument.id` | `INSTRUMENT_ID` (symbol only) |
| `ledger_host` | `LEDGER_HOST` |
| `registry_url` | hardcoded `registry::consts::DEVNET_REGISTRY_URL` |
| `keycloak.client_id` | `KEYCLOAK_CLIENT_ID` |
| `keycloak.username` | `KEYCLOAK_USER` |
| `keycloak.password` | `KEYCLOAK_PASSWORD` |
| `keycloak.url` | `KEYCLOAK_URL` (full token endpoint URL) |

Both parties share the Keycloak credentials and the ledger host.
`client_for(party)` builds a connected `TokenClient` for either party.

### Helpers

- `offers_with_reference(offers, reference)` filters offers by
  `create_argument.transfer.meta.values["splice.lfdecentralizedtrust.org/reference"]`.
- `offer_amount(offer)` reads `create_argument.transfer.amount`.
- `distribute_reference(base, sender, receiver)` computes
  `base64("{base}-{sender}-{receiver}")`. `distribute()` writes this encoded
  value on-ledger, so the raw uuid never appears there.
- `dec(s)` parses a `DamlDecimal`.

### Test flows

**integration_transfer_offer_accept** (amount 1, full round trip):

1. Party 1 saves its balance, sends an offer of 1 to party 2 with the
   test_uuid reference, and verifies one filtered outgoing offer.
2. Party 2 verifies one filtered incoming offer, saves its balance, accepts
   by contract id, and asserts its balance grew by 1.
3. Party 2 sends an offer of 1 back to party 1 with the same reference and
   verifies one filtered outgoing offer.
4. Party 1 asserts its balance is down 1, verifies one filtered incoming
   offer, accepts, asserts its balance equals the start, and verifies the
   filtered incoming offers are empty.

**integration_transfer_offer_cancel_reject**:

1. Party 1 saves its balance, sends an offer of 1.24, verifies one filtered
   outgoing offer, cancels it, verifies the filtered outgoing offers are
   empty, and asserts its balance equals the start.
2. Party 1 sends an offer of 1.25 and verifies one filtered outgoing offer.
3. Party 2 saves its balance, verifies one filtered incoming offer, rejects
   it, verifies the filtered incoming offers are empty, and asserts its
   balance equals the start.
4. Party 1 verifies the filtered outgoing offers are empty.

**integration_distribute**:

1. Party 1 saves its balance and calls `distribute` with
   `reference_base = test_uuid` and recipients `[party_2: 1, party_2: 2,
   party_2: 3]`.
2. Party 1 filters its outgoing offers by the encoded reference. All three
   offers share one encoded reference, because the sender and receiver are
   the same. The test asserts the amounts are exactly {1, 2, 3}.
3. Party 1 cancels each offer by contract id, verifies the filtered outgoing
   offers are empty, and asserts its balance equals the start.

## Accepted assumptions

- Cleanup runs only on success. A mid-flow failure can leave offers behind.
  The test_uuid reference makes manual cleanup easy.
- Transfers carry no token-denominated fee, so exact balance equality holds.
- Submits are submit-and-wait and both parties read the same participant,
  so the tests do not poll or retry reads.
- An outgoing offer consumes the sender's holdings at creation time, so the
  sender's balance drops before the receiver accepts.
