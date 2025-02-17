use serde::{Deserialize, Serialize};

use sdk::{
    cosmwasm_std::{StdResult, Storage, Timestamp},
    cw_storage_plus::Item,
};

use crate::ContractError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DispatchLog {
    pub last_dispatch: Timestamp,
}

impl DispatchLog {
    const STORAGE: Item<Self> = Item::new("dispatch_log");

    pub fn new(last_dispatch: Timestamp) -> Self {
        DispatchLog { last_dispatch }
    }

    // TODO merge the functionality of this and `update`
    pub fn last_dispatch(storage: &dyn Storage) -> Timestamp {
        Self::STORAGE
            .load(storage)
            .map(|log| log.last_dispatch)
            .unwrap_or_default()
    }

    pub fn update(
        storage: &mut dyn Storage,
        current_dispatch: Timestamp,
    ) -> Result<(), ContractError> {
        Self::STORAGE
            .may_load(storage)
            .map_err(ContractError::LoadDispatchLog)
            .and_then(|log| match log {
                None => Self::STORAGE
                    .save(
                        storage,
                        &DispatchLog {
                            last_dispatch: current_dispatch,
                        },
                    )
                    .map_err(ContractError::SaveDispatchLog),
                Some(l) => {
                    if current_dispatch < l.last_dispatch {
                        Err(ContractError::InvalidTimeConfiguration {})
                    } else {
                        Self::STORAGE
                            .update(storage, |mut log| -> StdResult<DispatchLog> {
                                log.last_dispatch = current_dispatch;
                                Ok(log)
                            })
                            .map_err(ContractError::SaveDispatchLog)
                            .map(drop)
                    }
                }
            })
    }
}
