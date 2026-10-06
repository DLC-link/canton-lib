use canton_api_client::models;
use serde::{Deserialize, Serialize};

pub const TRANSACTION_SHAPE_ACS_DELTA: &str = "TRANSACTION_SHAPE_ACS_DELTA";

// Canton 3.6 rejects the top-level `filter` and `verbose` request fields with
// DEPRECATED_API_DISABLED, so these requests carry an `updateFormat` or an
// `eventFormat` instead.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateRequest {
    #[serde(rename = "updateFormat")]
    pub update_format: UpdateFormat,
    #[serde(rename = "beginExclusive")]
    pub begin_exclusive: i64,
    #[serde(rename = "endInclusive")]
    pub end_inclusive: Option<i64>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetActiveContractsRequest {
    #[serde(rename = "eventFormat")]
    pub event_format: EventFormat,
    #[serde(rename = "activeAtOffset")]
    pub active_at_offset: i64,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventFormat {
    #[serde(rename = "filtersByParty")]
    pub filters_by_party: std::collections::HashMap<String, Filters>,
    #[serde(rename = "filtersForAnyParty", skip_serializing_if = "Option::is_none")]
    pub filters_for_any_party: Option<Filters>,
    #[serde(rename = "verbose")]
    pub verbose: bool,
}

impl EventFormat {
    /// Builds an event format that matches `filter` for the single `party`.
    pub fn for_party(party: String, filter: IdentifierFilter, verbose: bool) -> Self {
        let mut filters_by_party = std::collections::HashMap::new();
        filters_by_party.insert(
            party,
            Filters {
                cumulative: Some(vec![CumulativeFilter {
                    identifier_filter: filter,
                }]),
            },
        );
        Self {
            filters_by_party,
            filters_for_any_party: None,
            verbose,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateFormat {
    #[serde(
        rename = "includeTransactions",
        skip_serializing_if = "Option::is_none"
    )]
    pub include_transactions: Option<TransactionFormat>,
    #[serde(
        rename = "includeReassignments",
        skip_serializing_if = "Option::is_none"
    )]
    pub include_reassignments: Option<EventFormat>,
}

impl UpdateFormat {
    /// Builds the update format that the deprecated `filter` field selected:
    /// flat (ACS delta) transactions plus reassignments, with no topology
    /// events.
    pub fn flat_transactions(event_format: EventFormat) -> Self {
        Self {
            include_transactions: Some(TransactionFormat {
                event_format: event_format.clone(),
                transaction_shape: TRANSACTION_SHAPE_ACS_DELTA.to_string(),
            }),
            include_reassignments: Some(event_format),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionFormat {
    #[serde(rename = "eventFormat")]
    pub event_format: EventFormat,
    #[serde(rename = "transactionShape")]
    pub transaction_shape: String,
}

// TODO: It is duplicated with filters.rs in crates/common/src/filters.rs, let's remove later.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filters {
    #[serde(rename = "cumulative", skip_serializing_if = "Option::is_none")]
    pub cumulative: Option<Vec<CumulativeFilter>>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CumulativeFilter {
    #[serde(rename = "identifierFilter")]
    pub identifier_filter: IdentifierFilter,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdentifierFilter {
    EmptyIdentifierFilter(EmptyIdentifierFilter),
    InterfaceIdentifierFilter(InterfaceIdentifierFilter),
    TemplateIdentifierFilter(TemplateIdentifierFilter),
    WildcardIdentifierFilter(WildcardIdentifierFilter),
}

impl Default for IdentifierFilter {
    fn default() -> Self {
        Self::EmptyIdentifierFilter(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmptyIdentifierFilter {
    #[serde(rename = "Empty")]
    pub empty: serde_json::Value,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterfaceIdentifierFilter {
    #[serde(rename = "InterfaceFilter")]
    pub interface_filter: InterfaceFilter,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterfaceFilter {
    #[serde(rename = "value")]
    pub value: InterfaceFilterValue,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterfaceFilterValue {
    // TODO: tighten to `String` in a follow-up PR after the canton-lib 3.6.0 bump
    // cascades through cbtc-lib / dlc-attestor-stack / dec-party-manager. Canton
    // requires this field non-empty; today `None` is coerced to "" via
    // `.unwrap_or_default()` in `convert_identifier_filter`, which the server
    // will reject. Keeping `Option<String>` here for now to avoid breaking
    // downstream callers mid-cascade.
    #[serde(rename = "interfaceId", skip_serializing_if = "Option::is_none")]
    pub interface_id: Option<String>,
    #[serde(rename = "includeInterfaceView")]
    pub include_interface_view: bool,
    #[serde(rename = "includeCreatedEventBlob")]
    pub include_created_event_blob: bool,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateIdentifierFilter {
    #[serde(rename = "TemplateFilter")]
    pub template_filter: TemplateFilter,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateFilter {
    #[serde(rename = "value")]
    pub value: TemplateFilterValue,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateFilterValue {
    // TODO: tighten to `String` in a follow-up PR (see InterfaceFilterValue
    // above). `.unwrap_or_default()` in the converter sends "" if `None`, which
    // Canton rejects.
    #[serde(rename = "templateId", skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(rename = "includeCreatedEventBlob")]
    pub include_created_event_blob: bool,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WildcardIdentifierFilter {
    #[serde(rename = "WildcardFilter")]
    pub wildcard_filter: WildcardFilter,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WildcardFilter {
    #[serde(rename = "value")]
    pub value: WildcardFilterValue,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WildcardFilterValue {
    #[serde(rename = "includeCreatedEventBlob")]
    pub include_created_event_blob: bool,
}

pub fn convert_get_active_contracts_request(
    req: GetActiveContractsRequest,
) -> models::GetActiveContractsRequest {
    models::GetActiveContractsRequest {
        filter: None,
        verbose: None,
        active_at_offset: req.active_at_offset,
        event_format: Some(convert_event_format(req.event_format)),
        // None requests the first page from the server (no resume token).
        // TODO: surface pagination — if Canton ever paginates active-contracts
        // responses, callers need a way to pass the server-returned
        // continuation token back on subsequent requests, or results will be
        // silently truncated.
        stream_continuation_token: None,
    }
}

pub fn convert_event_format(ef: EventFormat) -> Box<models::EventFormat> {
    let mut filters_by_party: std::collections::HashMap<String, models::Filters> =
        std::collections::HashMap::new();
    for (party, filter) in ef.filters_by_party {
        filters_by_party.insert(party, convert_filters(filter));
    }
    Box::new(models::EventFormat {
        filters_by_party: Some(filters_by_party),
        filters_for_any_party: ef
            .filters_for_any_party
            .map(|f| Box::new(convert_filters(f))),
        verbose: Some(ef.verbose),
    })
}

pub fn convert_filters(f: Filters) -> models::Filters {
    models::Filters {
        cumulative: f
            .cumulative
            .map(|vec| vec.into_iter().map(convert_cumulative_filter).collect()),
    }
}

pub fn convert_cumulative_filter(cf: CumulativeFilter) -> models::CumulativeFilter {
    models::CumulativeFilter {
        identifier_filter: Some(Box::new(convert_identifier_filter(cf.identifier_filter))),
    }
}

pub fn convert_identifier_filter(idf: IdentifierFilter) -> models::IdentifierFilter {
    match idf {
        IdentifierFilter::EmptyIdentifierFilter(_) => {
            models::IdentifierFilter::IdentifierFilterOneOf1(Box::new(
                models::IdentifierFilterOneOf1 {
                    interface_filter: Box::default(),
                },
            ))
        }
        IdentifierFilter::InterfaceIdentifierFilter(i) => {
            models::IdentifierFilter::IdentifierFilterOneOf1(Box::new(
                models::IdentifierFilterOneOf1 {
                    interface_filter: Box::new(models::InterfaceFilter {
                        value: Box::new(models::InterfaceFilter1 {
                            interface_id: i.interface_filter.value.interface_id.unwrap_or_default(),
                            include_interface_view: Some(
                                i.interface_filter.value.include_interface_view,
                            ),
                            include_created_event_blob: Some(
                                i.interface_filter.value.include_created_event_blob,
                            ),
                        }),
                    }),
                },
            ))
        }
        IdentifierFilter::TemplateIdentifierFilter(t) => {
            models::IdentifierFilter::IdentifierFilterOneOf2(Box::new(
                models::IdentifierFilterOneOf2 {
                    template_filter: Box::new(models::TemplateFilter {
                        value: Box::new(models::TemplateFilter1 {
                            template_id: t.template_filter.value.template_id.unwrap_or_default(),
                            include_created_event_blob: Some(
                                t.template_filter.value.include_created_event_blob,
                            ),
                        }),
                    }),
                },
            ))
        }
        IdentifierFilter::WildcardIdentifierFilter(w) => {
            models::IdentifierFilter::IdentifierFilterOneOf3(Box::new(
                models::IdentifierFilterOneOf3 {
                    wildcard_filter: Box::new(models::WildcardFilter {
                        value: Box::new(models::WildcardFilter1 {
                            include_created_event_blob: Some(
                                w.wildcard_filter.value.include_created_event_blob,
                            ),
                        }),
                    }),
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_get_active_contracts_request_sends_event_format_and_no_deprecated_fields() {
        let req = GetActiveContractsRequest {
            event_format: EventFormat {
                verbose: true,
                ..Default::default()
            },
            active_at_offset: 42,
        };
        let out = convert_get_active_contracts_request(req);
        assert_eq!(out.active_at_offset, 42);
        assert!(out.filter.is_none());
        assert!(out.verbose.is_none());
        assert_eq!(
            out.event_format.expect("event_format is set").verbose,
            Some(true)
        );
        assert!(out.stream_continuation_token.is_none());

        let json = serde_json::to_value(convert_get_active_contracts_request(
            GetActiveContractsRequest::default(),
        ))
        .unwrap();
        assert!(json.get("filter").is_none());
        assert!(json.get("verbose").is_none());
        assert!(json.get("eventFormat").is_some());
    }

    #[test]
    fn convert_event_format_wraps_filters_by_party_in_some() {
        let ef = EventFormat::for_party("alice".to_string(), IdentifierFilter::default(), false);

        let out = convert_event_format(ef);
        let by_party = out
            .filters_by_party
            .as_ref()
            .expect("filters_by_party should be Some after conversion");
        assert!(by_party.contains_key("alice"));
        assert!(out.filters_for_any_party.is_none());
        assert_eq!(out.verbose, Some(false));
    }

    #[test]
    fn get_active_contracts_request_serializes_event_format_only() {
        let req = GetActiveContractsRequest {
            event_format: EventFormat::for_party(
                "alice".to_string(),
                IdentifierFilter::default(),
                false,
            ),
            active_at_offset: 7,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("filter").is_none());
        assert!(json.get("verbose").is_none());
        assert_eq!(json["activeAtOffset"], 7);
        assert_eq!(json["eventFormat"]["verbose"], false);
        assert!(json["eventFormat"]["filtersByParty"]["alice"]["cumulative"].is_array());
    }

    #[test]
    fn update_request_serializes_update_format_only() {
        let req = UpdateRequest {
            update_format: UpdateFormat::flat_transactions(EventFormat::for_party(
                "alice".to_string(),
                IdentifierFilter::default(),
                true,
            )),
            begin_exclusive: 3,
            end_inclusive: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("filter").is_none());
        assert!(json.get("verbose").is_none());
        assert_eq!(json["beginExclusive"], 3);

        let transactions = &json["updateFormat"]["includeTransactions"];
        assert_eq!(
            transactions["transactionShape"],
            TRANSACTION_SHAPE_ACS_DELTA
        );
        assert_eq!(transactions["eventFormat"]["verbose"], true);
        assert!(transactions["eventFormat"]["filtersByParty"]["alice"].is_object());
        assert!(
            json["updateFormat"]["includeReassignments"]["filtersByParty"]["alice"].is_object()
        );
        assert!(json["updateFormat"].get("includeTopologyEvents").is_none());
    }

    #[test]
    fn convert_filters_empty_maps_cumulative_to_none() {
        let out = convert_filters(Filters { cumulative: None });
        assert!(out.cumulative.is_none());
    }

    #[test]
    fn convert_cumulative_filter_wraps_identifier_in_some() {
        let out = convert_cumulative_filter(CumulativeFilter::default());
        assert!(out.identifier_filter.is_some());
    }

    #[test]
    fn convert_identifier_filter_interface_unwraps_id_and_wraps_bools() {
        let out = convert_identifier_filter(IdentifierFilter::InterfaceIdentifierFilter(
            InterfaceIdentifierFilter {
                interface_filter: InterfaceFilter {
                    value: InterfaceFilterValue {
                        interface_id: Some("pkg:Mod:Iface".to_string()),
                        include_interface_view: true,
                        include_created_event_blob: false,
                    },
                },
            },
        ));
        match out {
            models::IdentifierFilter::IdentifierFilterOneOf1(b) => {
                assert_eq!(b.interface_filter.value.interface_id, "pkg:Mod:Iface");
                assert_eq!(b.interface_filter.value.include_interface_view, Some(true));
                assert_eq!(
                    b.interface_filter.value.include_created_event_blob,
                    Some(false)
                );
            }
            _ => panic!("Interface variant should map to IdentifierFilterOneOf1"),
        }
    }

    #[test]
    fn convert_identifier_filter_interface_none_id_becomes_empty_string() {
        // Regression guard for the documented footgun: None coerces to "" on the
        // wire, which Canton will reject. Will go away when the field is
        // tightened to `String` in the follow-up PR.
        let out = convert_identifier_filter(IdentifierFilter::InterfaceIdentifierFilter(
            InterfaceIdentifierFilter::default(),
        ));
        match out {
            models::IdentifierFilter::IdentifierFilterOneOf1(b) => {
                assert_eq!(b.interface_filter.value.interface_id, "");
            }
            _ => panic!("Interface variant should map to IdentifierFilterOneOf1"),
        }
    }

    #[test]
    fn convert_identifier_filter_template_unwraps_id_and_wraps_bool() {
        let out = convert_identifier_filter(IdentifierFilter::TemplateIdentifierFilter(
            TemplateIdentifierFilter {
                template_filter: TemplateFilter {
                    value: TemplateFilterValue {
                        template_id: Some("pkg:Mod:Tmpl".to_string()),
                        include_created_event_blob: true,
                    },
                },
            },
        ));
        match out {
            models::IdentifierFilter::IdentifierFilterOneOf2(b) => {
                assert_eq!(b.template_filter.value.template_id, "pkg:Mod:Tmpl");
                assert_eq!(
                    b.template_filter.value.include_created_event_blob,
                    Some(true)
                );
            }
            _ => panic!("Template variant should map to IdentifierFilterOneOf2"),
        }
    }

    #[test]
    fn convert_identifier_filter_wildcard_wraps_bool() {
        let out = convert_identifier_filter(IdentifierFilter::WildcardIdentifierFilter(
            WildcardIdentifierFilter {
                wildcard_filter: WildcardFilter {
                    value: WildcardFilterValue {
                        include_created_event_blob: true,
                    },
                },
            },
        ));
        match out {
            models::IdentifierFilter::IdentifierFilterOneOf3(b) => {
                assert_eq!(
                    b.wildcard_filter.value.include_created_event_blob,
                    Some(true)
                );
            }
            _ => panic!("Wildcard variant should map to IdentifierFilterOneOf3"),
        }
    }
}
