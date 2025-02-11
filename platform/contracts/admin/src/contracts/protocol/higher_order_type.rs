use serde::{Deserialize, Serialize};

use super::super::HigherOrderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolContracts;

impl HigherOrderType for ProtocolContracts {
    type Of<T> = super::ProtocolContracts<T>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Protocol;

impl HigherOrderType for Protocol {
    type Of<T> = super::Protocol<T>;
}
