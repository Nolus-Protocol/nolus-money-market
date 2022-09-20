use std::result::Result as StdResult;

use cosmwasm_std::{Addr, StdError};
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum ContractError {
    #[error("[Profit] [Std] {0}")]
    Std(#[from] StdError),

    #[error("[Profit] {0}")]
    Platform(#[from] platform::error::Error),

    #[error("[Profit] Unauthorized")]
    Unauthorized {},

    #[error("[Profit] Alarm time validation failed")]
    AlarmTimeValidation {},

    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("[Profit] Invalid contract address {0}")]
    InvalidContractAddress(Addr),

    #[error("[Profit] Invalid alarm notification address: {0:?}")]
    InvalidAlarmAddress(Addr),

    #[error("[Profit] Alarm comming from unknown address: {0:?}")]
    UnrecognisedAlarm(Addr),

    #[error(
        "[Profit] Invalid time configuration. Current profit transfer time is before the last transfer time"
    )]
    InvalidTimeConfiguration {},

    #[error("[Profit] EmptyBalance. No profit to dispatch")]
    EmptyBalance {},
}

pub type Result<T> = StdResult<T, ContractError>;
