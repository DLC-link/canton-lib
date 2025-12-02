use canton_api_client::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateRequest {
    #[serde(rename = "filter", skip_serializing_if = "Option::is_none")]
    pub filter: Option<TransactionFilter>,
    #[serde(rename = "verbose")]
    pub verbose: bool,
    #[serde(rename = "beginExclusive")]
    pub begin_exclusive: i64,
    #[serde(rename = "endInclusive")]
    pub end_inclusive: Option<i64>,
    // #[serde(rename = "eventFormat", skip_serializing_if = "Option::is_none")]
    // pub update_format: Option<Box<models::EventFormat>>, TODO
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct GetActiveContractsRequest {
    #[serde(rename = "filter", skip_serializing_if = "Option::is_none")]
    pub filter: Option<TransactionFilter>,
    #[serde(rename = "verbose")]
    pub verbose: bool,
    #[serde(rename = "activeAtOffset")]
    pub active_at_offset: i64,
    // #[serde(rename = "eventFormat", skip_serializing_if = "Option::is_none")]
    // pub event_format: Option<Box<models::EventFormat>>, // TODO
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransactionFilter {
    #[serde(rename = "filtersByParty")]
    pub filters_by_party: std::collections::HashMap<String, Filters>,
    #[serde(rename = "filtersForAnyParty", skip_serializing_if = "Option::is_none")]
    pub filters_for_any_party: Option<Filters>,
}

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
        filter: req.filter.map(convert_transaction_filter),
        verbose: req.verbose,
        active_at_offset: req.active_at_offset,
        event_format: None, // TODO
    }
}

pub fn convert_transaction_filter(tf: TransactionFilter) -> Box<models::TransactionFilter> {
    let mut filters_by_party: std::collections::HashMap<String, models::Filters> =
        std::collections::HashMap::new();
    for (party, filter) in tf.filters_by_party {
        filters_by_party.insert(party, convert_filters(filter));
    }
    Box::new(models::TransactionFilter {
        filters_by_party,
        filters_for_any_party: tf
            .filters_for_any_party
            .map(|f| Box::new(convert_filters(f))),
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
        identifier_filter: Box::new(convert_identifier_filter(cf.identifier_filter)),
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
                            interface_id: i.interface_filter.value.interface_id,
                            include_interface_view: i.interface_filter.value.include_interface_view,
                            include_created_event_blob: i
                                .interface_filter
                                .value
                                .include_created_event_blob,
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
                            template_id: t.template_filter.value.template_id,
                            include_created_event_blob: t
                                .template_filter
                                .value
                                .include_created_event_blob,
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
                            include_created_event_blob: w
                                .wildcard_filter
                                .value
                                .include_created_event_blob,
                        }),
                    }),
                },
            ))
        }
    }
}
