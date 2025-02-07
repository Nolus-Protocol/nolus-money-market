use serde::{Deserialize, Serialize};

use finance::duration::Duration;
use sdk::cosmwasm_std::{Env, MessageInfo, QuerierWrapper, Reply, Timestamp};

use crate::{
    api::{
        position::{ClosePolicyChange, PositionClose},
        query::StateResponse,
    },
    error::ContractResult,
};

use super::{handler::Handler as LeaseHandler, Contract, Response};

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct State<H> {
    handler: H,
}

impl<H> State<H> {
    pub fn new(handler: H) -> Self {
        Self { handler }
    }
}

impl<H> Contract for State<H>
where
    H: LeaseHandler,
{
    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<StateResponse> {
        self.handler.state(now, due_projection, querier)
    }

    fn reply(self, querier: QuerierWrapper<'_>, env: Env, msg: Reply) -> ContractResult<Response> {
        self.handler.reply(querier, env, msg)
    }

    fn repay(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.repay(querier, env, info)
    }

    fn change_close_policy(
        self,
        change: ClosePolicyChange,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.change_close_policy(change, querier, env, info)
    }

    fn close_position(
        self,
        spec: PositionClose,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.close_position(spec, querier, env, info)
    }

    fn close(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.close(querier, env, info)
    }

    fn on_time_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.on_time_alarm(querier, env, info)
    }

    fn on_price_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.on_price_alarm(querier, env, info)
    }

    fn heal(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.heal(querier, env, info)
    }
}
