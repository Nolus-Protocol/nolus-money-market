use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use super::super::HigherOrderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub enum Contracts {}

impl HigherOrderType for Contracts {
    type Of<T> = super::Contracts<T>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Protocol {}

impl HigherOrderType for Protocol {
    type Of<T> = super::Protocol<T>;
}
