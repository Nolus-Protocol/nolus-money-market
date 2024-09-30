use thiserror::Error;

use sdk::cosmwasm_std::StdError;

#[derive(Error, Debug, PartialEq)]
pub enum AlarmError {
    #[error("[Market Price; Alarm] Failed to load next subscriber! Cause: {0}")]
    IteratorLoadFailed(StdError),

    #[error("[Market Price; Alarm] Failed to load in delivery queue length! Cause: {0}")]
    InDeliveryIsEmptyFailed(StdError),

    #[error("[Market Price; Alarm] Failed to remove \"below price\"! Cause: {0}")]
    RemoveBelow(StdError),

    #[error("[Market Price; Alarm] Failed to store new \"below price\" alarm! Cause: {0}")]
    AddAlarmStoreBelow(StdError),

    #[error("[Market Price; Alarm] Failed to load \"above or equal price\"! Cause: {0}")]
    AddAlarmLoadAboveOrEqual(StdError),

    #[error("[Market Price; Alarm] Failed to store new \"above or equal price\"! Cause: {0}")]
    AddAlarmStoreAboveOrEqual(StdError),

    #[error("[Market Price; Alarm] Failed to remove \"above or equal price\"! Cause: {0}")]
    RemoveAboveOrEqual(StdError),

    #[error("[Market Price; Alarm; InDelivery] Failed to load \"below price\"! Cause: {0}")]
    InDeliveryLoadBelow(StdError),

    #[error("[Market Price; Alarm; In Delivery] Failed to remove \"below price\"! Cause: {0}")]
    InDeliveryRemoveBelow(StdError),

    #[error(
        "[Market Price; Alarm; In Delivery] Failed to load \"above or equal price\"! Cause: {0}"
    )]
    InDeliveryLoadAboveOrEqual(StdError),

    #[error(
        "[Market Price; Alarm; In Delivery] Failed to remove \"above or equal price\"! Cause: {0}"
    )]
    InDeliveryRemoveAboveOrEqual(StdError),

    #[error("[Market Price; Alarm; In Delivery] Failed to append alarm in \"in delivery\" queue! Cause: {0}")]
    InDeliveryAppend(StdError),

    #[error("[Market Price; Alarm] Failed to remove last delivered alarm from queue! Cause: {0}")]
    LastDeliveredRemove(StdError),

    #[error("[Market Price; Alarm] Failed to remove last failed alarm from queue! Cause: {0}")]
    LastFailedRemove(StdError),

    #[error("[Market Price; Alarm] Alarms delivery queue is empty! Cause: {0}")]
    EmptyAlarmsInDeliveryQueue(String),

    #[error("[Market Price; Alarm] Alarms delivery queue is not empty! Cause: {0}")]
    NonEmptyAlarmsInDeliveryQueue(String),

    #[error("[Market Price; Alarm] Failed to create a normalized price! Cause: {0}")]
    CreatingNormalizedPrice(finance::error::Error),
}
