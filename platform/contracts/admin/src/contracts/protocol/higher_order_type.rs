use super::super::HigherOrderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contracts {}

impl HigherOrderType for Contracts {
    type Of<Unit> = super::Contracts<Unit>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {}

impl HigherOrderType for Protocol {
    type Of<Unit> = super::Protocol<Unit>;
}
