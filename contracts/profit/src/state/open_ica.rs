use serde::{Deserialize, Serialize};

use dex::{Account, ConnectionParams, DexConnectable, IcaConnectee};

use crate::{error::ContractError, msg::ConfigResponse, result::ContractResult};

use super::{idle::Idle, Config, ConfigManagement, IcaConnector, SetupDexHandler, State};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct OpenIca {
    config: Config,
    dex: ConnectionParams,
}

impl OpenIca {
    pub fn new(config: Config, connection: ConnectionParams) -> Self {
        Self {
            config,
            dex: connection,
        }
    }
}

impl IcaConnectee for OpenIca {
    type State = State;
    type NextState = Idle;

    fn connected(self, account: Account) -> Self::NextState {
        Idle::new(self.config, account)
    }
}

impl DexConnectable for OpenIca {
    fn dex(&self) -> &ConnectionParams {
        &self.dex
    }
}

impl ConfigManagement for IcaConnector {
    fn try_update_config(self, _: u16) -> ContractResult<Self> {
        Err(ContractError::UnsupportedOperation(String::from(
            "Configuration changes are not allowed during ICA opening process.",
        )))
    }

    fn try_query_config(&self) -> ContractResult<ConfigResponse> {
        Err(ContractError::UnsupportedOperation(String::from(
            "Querying configuration is not allowed during ICA opening process.",
        )))
    }
}

impl SetupDexHandler for IcaConnector {
    type State = Self;
}
