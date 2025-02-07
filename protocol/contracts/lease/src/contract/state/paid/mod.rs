use serde::{Deserialize, Serialize};

use dex::Enterable;
use finance::duration::Duration;
use sdk::cosmwasm_std::{Env, MessageInfo, QuerierWrapper, Timestamp};

use crate::{api::query::StateResponse, contract::Lease, error::ContractResult};

use super::{Handler, Response};

use self::transfer_in::DexState;

pub mod transfer_in;

#[derive(Serialize, Deserialize)]
pub struct Active {
    lease: Lease,
}

impl Active {
    pub(in super::super) fn new(lease: Lease) -> Self {
        Self { lease }
    }
}

impl Handler for Active {
    fn state(
        self,
        _now: Timestamp,
        _due_projection: Duration,
        _querier: QuerierWrapper<'_>,
    ) -> ContractResult<StateResponse> {
        Ok(StateResponse::paid_from(self.lease.lease, None))
    }

    fn close(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        access_control::check(&self.lease.lease.customer, &info.sender)?;

        let start_transfer_in = transfer_in::start(self.lease);
        start_transfer_in
            .enter(env.block.time, querier)
            .map(|batch| Response::from(batch, DexState::from(start_transfer_in)))
            .map_err(Into::into)
    }
    fn on_time_alarm(
        self,
        _querier: QuerierWrapper<'_>,
        _env: Env,
        _info: MessageInfo,
    ) -> ContractResult<Response> {
        super::ignore_msg(self)
    }
    fn on_price_alarm(
        self,
        _querier: QuerierWrapper<'_>,
        _env: Env,
        _info: MessageInfo,
    ) -> ContractResult<Response> {
        super::ignore_msg(self)
    }
}
