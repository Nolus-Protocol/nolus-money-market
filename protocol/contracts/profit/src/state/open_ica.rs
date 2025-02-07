use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::{Deserialize, Serialize};

use dex::{Account, ConnectionParams, Contract, DexConnectable, IcaConnectee};
use finance::duration::Duration;
use sdk::cosmwasm_std::{QuerierWrapper, Timestamp};

use crate::msg::ConfigResponse;

use super::{idle::Idle, Config, ConfigManagement, IcaConnector, State};

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

impl Contract for OpenIca {
    type StateResponse = ConfigResponse;

    fn state(
        self,
        _: Timestamp,
        _due_projection: Duration,
        _: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        ConfigResponse {
            cadence_hours: self.config.cadence_hours(),
        }
    }
}

impl Display for OpenIca {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("Idle"))
    }
}

impl ConfigManagement for IcaConnector {}
