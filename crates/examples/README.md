# Examples

This directory contains example programs demonstrating how to use the Canton CBTC library.

## Setup

1. Copy `.env.example` to `.env` in the project root:

   ```bash
   cp ../../.env.example ../../.env
   ```

2. Fill in your configuration values in `.env`

## Running Examples

All examples should be run from the workspace root directory.

### Check Balance

Check your CBTC balance and UTXO count:

```bash
cargo run -p examples --bin check_balance
```

### Send CBTC

Send CBTC to another party:

```bash
# Set the amount and receiver in .env or environment
export TRANSFER_AMOUNT=0.1
export LIB_TEST_RECEIVER_PARTY_ID="receiver-party::1220..."
cargo run -p examples --bin send_cbtc
```

### List Incoming Offers

List all pending CBTC transfer offers where you are the receiver:

```bash
cargo run -p examples --bin list_incoming_offers
```

This example lists all pending transfers waiting for you to accept.

### List Outgoing Offers

List all pending CBTC transfer offers where you are the sender:

```bash
cargo run -p examples --bin list_outgoing_offers
```

This example shows all transfers you've sent that haven't been accepted yet.

### Accept Pending Transfers

Accept all pending CBTC transfers for your party:

```bash
cargo run -p examples --bin accept_transfers
```

This example automatically fetches all pending TransferInstruction contracts and accepts them in a loop. Useful for automated acceptance of incoming transfers.

### Cancel Pending Transfers

Cancel all pending outgoing transfers that haven't been accepted:

```bash
cargo run -p examples --bin cancel_offers
```

This example withdraws all transfer offers you've sent that are still pending, returning the CBTC to your account.

### Stream CBTC

Stream CBTC to a single receiver multiple times:

```bash
# Set the streaming parameters
export RECEIVER_PARTY="receiver-party::1220..."
export TRANSFER_COUNT=10
export TRANSFER_AMOUNT=0.001
cargo run -p examples --bin stream_cbtc
```

This example sends multiple transfers to the same receiver, useful for streaming payments or testing repeated transfers.

### Consolidate UTXOs

Check and consolidate UTXOs if needed:

```bash
# Optional: set custom threshold (default is 10)
export CONSOLIDATION_THRESHOLD=8
cargo run -p examples --bin consolidate_utxos
```

### Batch Distribute

Distribute CBTC to multiple recipients from a CSV file:

```bash
# Create recipients.csv with format:
# receiver,amount
# party1::1220...,5.0
# party2::1220...,3.5

# Run the example
cargo run -p examples --bin batch_distribute

# Or specify a custom CSV path
export RECIPIENTS_CSV=my_recipients.csv
cargo run -p examples --bin batch_distribute
```

See `recipients_example.csv` for the CSV format.

### Batch Distribute with Callback

Distribute CBTC to multiple recipients with real-time result logging:

```bash
cargo run -p examples --bin batch_with_callback
```

This example demonstrates the callback feature, which allows you to process transfer results as they complete. The callback writes one line per transfer to a timestamped log file.

## Transfer Result Callbacks

The batch distribution system supports optional callbacks that are invoked after each transfer completes (whether successful or failed). This allows for real-time processing of transfer results.

### TransferResult Structure

Each callback receives a `TransferResult` with the following fields:

```rust
pub struct TransferResult {
    pub success: bool,              // Whether the transfer succeeded
    pub transfer_index: usize,      // Index in the batch (0-based)
    pub receiver: String,           // Recipient address
    pub amount: String,             // Transfer amount
    pub transfer_offer_cid: Option<String>,  // Contract ID if successful
    pub update_id: Option<String>,  // Canton ledger update ID if successful
    pub reference: Option<String>,  // Unique reference ID (base64 encoded)
    pub raw_response: Option<String>, // Full JSON response from ledger
    pub error: Option<String>,      // Error message if failed
}
```

### Use Cases

1. **Real-time Logging**: Write results to a file or logging service as they complete
2. **Database Updates**: Store transfer records in a database immediately
3. **Progress Tracking**: Update UI or monitoring dashboards
4. **Notifications**: Send alerts for failed transfers
5. **Custom Retry Logic**: Implement application-specific retry strategies

### Basic Usage

```rust
use std::pin::Pin;
use std::future::Future;

let callback = Box::new(|result: cbtc::transfer::TransferResult| -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        if result.success {
            println!("✅ Transfer succeeded: {} to {}", result.amount, result.receiver);
            // Log to database, send notification, etc.
        } else {
            println!("❌ Transfer failed: {}", result.error.unwrap_or_default());
            // Handle failure (retry, alert, etc.)
        }
    })
}) as Box<cbtc::transfer::TransferResultCallback>;

let result = cbtc::distribute::submit(cbtc::distribute::Params {
    // ... other params
    on_transfer_complete: Some(callback),
})
.await?;
```

### File Logging Example (One Line Per Event)

```rust
use std::fs::OpenOptions;
use std::io::Write;

let log_file = format!("transfer_results_{}.log", chrono::Utc::now().format("%Y%m%d_%H%M%S"));

let callback = Box::new(move |result: cbtc::transfer::TransferResult| -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let log_file = log_file.clone();
    Box::pin(async move {
        let status = if result.success { "SUCCESS" } else { "FAILED" };
        let reference = result.reference.as_deref().unwrap_or("N/A");
        let offer_cid = result.transfer_offer_cid.as_deref().unwrap_or("N/A");
        let update_id = result.update_id.as_deref().unwrap_or("N/A");
        let error = result.error.as_deref().unwrap_or("N/A");

        let log_line = format!(
            "{} | {} | idx={} | to={} | amount={} | ref={} | offer={} | update_id={} | error={}\n",
            chrono::Utc::now().to_rfc3339(),
            status,
            result.transfer_index,
            result.receiver,
            result.amount,
            reference,
            offer_cid,
            update_id,
            error
        );

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            let _ = file.write_all(log_line.as_bytes());
        }
    })
}) as Box<cbtc::transfer::TransferResultCallback>;
```

Example output:

```
2025-01-10T15:23:45.123Z | SUCCESS | idx=0 | to=merchant::1220... | amount=0.001 | ref=YmF0Y2gtMTIz... | offer=00123... | update_id=12208abc... | error=N/A
2025-01-10T15:23:46.456Z | FAILED | idx=1 | to=validator::5678... | amount=0.002 | ref=YmF0Y2gtNDU2... | offer=N/A | update_id=N/A | error=Insufficient funds
```

### Database Logging Example

```rust
let db_pool = /* your DB connection pool */;

let callback = Box::new(move |result: cbtc::transfer::TransferResult| -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let pool = db_pool.clone();
    Box::pin(async move {
        sqlx::query!(
            "INSERT INTO transfer_logs (receiver, amount, success, reference, raw_response, error)
             VALUES ($1, $2, $3, $4, $5, $6)",
            result.receiver,
            result.amount,
            result.success,
            result.reference,
            result.raw_response,
            result.error
        )
        .execute(&pool)
        .await
        .unwrap();
    })
}) as Box<cbtc::transfer::TransferResultCallback>;
```

### Progress Tracking Example

```rust
use std::sync::{Arc, Mutex};

let progress = Arc::new(Mutex::new(0));
let total = recipients.len();

let callback = Box::new(move |result: cbtc::transfer::TransferResult| -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let progress = progress.clone();
    Box::pin(async move {
        let mut count = progress.lock().unwrap();
        *count += 1;
        println!("Progress: {}/{}", count, total);

        if result.success {
            println!("  ✓ {} transferred", result.amount);
        } else {
            println!("  ✗ Failed: {}", result.error.unwrap_or_default());
        }
    })
}) as Box<cbtc::transfer::TransferResultCallback>;
```

### No Callback (Default)

If you don't need callbacks, simply pass `None`:

```rust
let result = cbtc::distribute::submit(cbtc::distribute::Params {
    // ... other params
    on_transfer_complete: None,  // No callback
})
.await?;
```

### Important Notes

1. **Callbacks are async**: They can perform async operations like database writes
2. **Callbacks are called sequentially**: One completes before the next transfer starts
3. **Errors in callbacks don't stop the batch**: If a callback panics, it's isolated
4. **The reference field** contains the base64-encoded unique ID: `base64(reference_base + sender + receiver)`
5. **The update_id field** contains the Canton ledger's unique update ID for successful transfers (used for tracking and idempotency)
6. **The raw_response field** contains the full JSON response from the Canton ledger (for successful submissions)

See `examples/batch_with_callback.rs` for a complete working example.

## Environment Variables

Required for all examples:

- `KEYCLOAK_HOST` - Your Keycloak host
- `KEYCLOAK_REALM` - Your Keycloak realm
- `KEYCLOAK_CLIENT_ID` - Client ID
- `KEYCLOAK_USERNAME` - Username
- `KEYCLOAK_PASSWORD` - Password
- `LEDGER_HOST` - Canton participant node URL
- `PARTY_ID` - Your party ID
- `DECENTRALIZED_PARTY_ID` - CBTC decentralized party ID
- `REGISTRY_URL` - Canton registry URL

Optional:

- `TRANSFER_AMOUNT` - Amount to send (default: 0.1)
- `LIB_TEST_RECEIVER_PARTY_ID` - Receiver party for transfers
- `CONSOLIDATION_THRESHOLD` - UTXO threshold for consolidation (default: 10)
- `RECIPIENTS_CSV` - Path to CSV file for batch distribution (default: recipients.csv)

For stream example:

- `RECEIVER_PARTY` - The party ID to receive all stream transfers
- `TRANSFER_COUNT` - Number of transfers to send in the stream
- `TRANSFER_AMOUNT` - Amount per transfer
