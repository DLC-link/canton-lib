use canton_api_client::models;
use serde::{Deserialize, Serialize};

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
                            include_interface_view: Some(i.interface_filter.value.include_interface_view),
                            include_created_event_blob: Some(i
                                .interface_filter
                                .value
                                .include_created_event_blob),
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
                            include_created_event_blob: Some(t
                                .template_filter
                                .value
                                .include_created_event_blob),
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
                            include_created_event_blob: Some(w
                                .wildcard_filter
                                .value
                                .include_created_event_blob),
                        }),
                    }),
                },
            ))
        }
    }
}
