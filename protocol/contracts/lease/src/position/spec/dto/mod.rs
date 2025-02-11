use serde::{Deserialize, Serialize};

use crate::{
    api::open::PositionSpecDTO,
    position::{close::Policy as ClosePolicy, PositionError, PositionResult, Spec},
};

mod migrate_v0_7_5;

/// Position specification past position open
///
/// It is created from an initial specification and a default policy.
/// Only the latter may change over the lifetime of the lease.
///
/// Designed to be used as a non-public API component! Invariant checks are not done on deserialization!
#[derive(Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "contract_testing", derive(Debug, PartialEq, Eq))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    try_from = "migrate_v0_7_5::SpecDTO"
)]
pub struct SpecDTO {
    r#const: PositionSpecDTO,
    close: ClosePolicy,
}

impl SpecDTO {
    fn initial(r#const: PositionSpecDTO) -> Self {
        Self {
            r#const,
            close: Default::default(),
        }
    }

    fn new(r#const: PositionSpecDTO, close: ClosePolicy) -> Self {
        Self { r#const, close }
    }
}

impl From<PositionSpecDTO> for SpecDTO {
    fn from(value: PositionSpecDTO) -> Self {
        Self::initial(value)
    }
}

impl TryFrom<PositionSpecDTO> for Spec {
    type Error = PositionError;

    fn try_from(value: PositionSpecDTO) -> PositionResult<Self> {
        SpecDTO::from(value).try_into()
    }
}

impl From<Spec> for SpecDTO {
    fn from(spec: Spec) -> Self {
        SpecDTO::new(
            PositionSpecDTO::new_internal(
                spec.liability,
                spec.min_asset.into(),
                spec.min_transaction.into(),
            ),
            spec.close,
        )
    }
}

impl TryFrom<SpecDTO> for Spec {
    type Error = PositionError;

    fn try_from(dto: SpecDTO) -> PositionResult<Self> {
        dto.r#const
            .min_asset
            .try_into()
            .and_then(|min_asset| {
                dto.r#const
                    .min_transaction
                    .try_into()
                    .map(|min_transaction| {
                        Self::new(dto.r#const.liability, dto.close, min_asset, min_transaction)
                    })
            })
            .map_err(Into::into)
    }
}
