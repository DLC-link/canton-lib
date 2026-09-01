use serde::{Deserialize, Serialize};

pub mod accept;
pub mod allocation;
pub mod allocation_factory;
pub mod consts;
pub mod decimal;
pub mod filters;
pub mod submission;
pub mod transfer;
pub mod transfer_factory;

/// Which registry API a call uses.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenStandardVersion {
    #[default]
    V1,
    V2,
}
