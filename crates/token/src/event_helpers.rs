// Semantic accessors for `canton_api_client::models::Event` (re-exported
// as `ledger::models::Event`).
//
// The OpenAPI generator names the three `oneOf` variants positionally
// (`EventOneOf` / `EventOneOf1` / `EventOneOf2`) because the upstream
// Canton OpenAPI spec doesn't give each branch a name. The Cargo.toml
// constraint `canton-api-client = "3.3.0-0.1.0"` resolves to crates.io
// version 3.3.0-0.1.1 (per Cargo.lock); that resolution locks the
// variant ordering for us, but matching on those positional names at
// every call site is fragile if the upstream spec is ever regenerated.
// These helpers centralise the match so a future variant renumbering
// only touches this one file.
//
// The variant -> event mapping (verified against the resolved
// canton-api-client 3.3.0-0.1.1's `src/models/event_one_of*.rs`) is:
//   - `Event::EventOneOf`  wraps `EventOneOf`  -> `archived_event`
//   - `Event::EventOneOf1` wraps `EventOneOf1` -> `created_event`
//   - `Event::EventOneOf2` wraps `EventOneOf2` -> `exercised_event`
//
// Reported upstream:
// https://github.com/scolear/canton-api-rust-client/issues/3

use ledger::models::{ArchivedEvent, CreatedEvent, Event, ExercisedEvent};

pub(crate) fn as_created_event(event: &Event) -> Option<&CreatedEvent> {
    match event {
        Event::EventOneOf1(wrapper) => Some(&wrapper.created_event),
        _ => None,
    }
}

pub(crate) fn as_exercised_event(event: &Event) -> Option<&ExercisedEvent> {
    match event {
        Event::EventOneOf2(wrapper) => Some(&wrapper.exercised_event),
        _ => None,
    }
}

#[allow(dead_code)]
pub(crate) fn as_archived_event(event: &Event) -> Option<&ArchivedEvent> {
    match event {
        Event::EventOneOf(wrapper) => Some(&wrapper.archived_event),
        _ => None,
    }
}
