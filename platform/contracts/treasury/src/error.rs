use thiserror::Error;

use sdk::cosmwasm_std::StdError;

#[derive(Error, PartialEq, Debug)]
pub enum ContractError {
    #[error("[Treasury] [Std] {0}")]
    Std(#[from] StdError),

    #[error("[Treasury] Failed to serialize! Cause: {0}")]
    Serialize(StdError),

    #[error("[Treasury] Failed to init the contract version! Cause: {0}")]
    InitVersion(StdError),

    #[error("[Treasury] Failed to validate the Registry address! Cause: {0}")]
    ValidateRegistryAddr(StdError),

    #[error("[Treasury] Failed to validate the Timealarms address! Cause: {0}")]
    ValidateTimeAlarmsAddr(platform::error::Error),

    #[error("[Treasury] Failed to update the storage! Cause: {0}")]
    UpdateStorage(StdError),

    #[error("[Treasury] Failed to update the software! Cause: {0}")]
    UpdateSoftware(StdError),

    #[error("[Treasury] Failed to retrieve all protocols! Cause: {0}")]
    QueryProtocols(StdError),

    #[error("[Treasury] Failed to protocol contracts! Cause: {0}")]
    QueryProtocol(StdError),

    #[error("[Treasury] {0}")]
    SerializeResponse(#[from] platform::error::Error),

    #[error("[Treasury] Failed to load the configuration! Cause: {0}")]
    LoadConfig(StdError),

    #[error("[Treasury] Failed to save the configuration! Cause: {0}")]
    SaveConfig(StdError),

    #[error("[Treasury] Failed to load the dispatch log! Cause: {0}")]
    LoadDispatchLog(StdError),

    #[error("[Treasury] Failed to save the dispatch log! Cause: {0}")]
    SaveDispatchLog(StdError),

    #[error("[Treasury] Failed to obtain Lpp balance! Cause: {0}")]
    ReadLppBalance(lpp_platform::error::Error),

    #[error("[Treasury] Failed to distribute rewards to an Lpp! Cause: {0}")]
    DistributeLppReward(lpp_platform::error::Error),

    #[error("[Treasury] Failed to convert rewards to NLS! Cause: {0}")]
    ConvertRewardsToNLS(oracle_platform::error::Error),

    #[error("[Treasury] Failed to setup a time alarms stub! Cause: {0}")]
    SetupTimeAlarmStub(timealarms::error::ContractError),

    #[error("[Treasury] Failed to setup a time alarm! Cause: {0}")]
    SetupTimeAlarm(timealarms::error::ContractError),

    #[error("[Treasury] {0}")]
    Unauthorized(#[from] access_control::error::Error),

    #[error("[Treasury] Invalid time configuration. Current reward distribution time is before the last distribution time")]
    InvalidTimeConfiguration {},

    #[error("[Treasury] Error calculating interest: {0}")]
    InterestCalculation(#[from] finance::error::Error),
}
