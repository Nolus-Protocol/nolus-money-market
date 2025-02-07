use serde::{Deserialize, Serialize};

use dex::{Contract as DexContract, Handler as DexHandler};
use finance::duration::Duration;
use platform::state_machine;
use sdk::cosmwasm_std::{Binary, Env, MessageInfo, QuerierWrapper, Reply, Timestamp};

use crate::{
    api::query::StateResponse as QueryStateResponse,
    contract::{api::Contract, state::StateResponse as ContractStateResponse},
    error::ContractResult,
};

use super::{Response, State as ContractState};

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
    H: DexHandler<SwapResult = ContractResult<Response>>,
    H: DexContract<StateResponse = ContractResult<QueryStateResponse>>,
    H::Response: Into<ContractState>,
    Self: Into<ContractState>,
{
    fn on_open_ica(
        self,
        counterparty_version: String,
        querier: QuerierWrapper<'_>,
        env: Env,
    ) -> ContractResult<Response> {
        self.handler
            .on_open_ica(counterparty_version, querier, env)
            .map(state_machine::from)
            .map_err(Into::into)
    }

    fn on_dex_response(
        self,
        data: Binary,
        querier: QuerierWrapper<'_>,
        env: Env,
    ) -> ContractResult<Response> {
        self.handler.on_response(data, querier, env).into()
    }

    fn on_dex_error(self, querier: QuerierWrapper<'_>, env: Env) -> ContractResult<Response> {
        self.handler
            .on_error(querier, env)
            .map(state_machine::from)
            .map_err(Into::into)
    }

    fn on_dex_timeout(self, querier: QuerierWrapper<'_>, env: Env) -> ContractResult<Response> {
        self.handler
            .on_timeout(querier, env)
            .map(state_machine::from)
            .map_err(Into::into)
    }

    fn on_dex_inner(self, querier: QuerierWrapper<'_>, env: Env) -> ContractResult<Response> {
        self.handler.on_inner(querier, env).into()
    }

    fn on_dex_inner_continue(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
    ) -> ContractResult<Response> {
        self.handler
            .on_inner_continue(querier, env)
            .map(state_machine::from)
            .map_err(Into::into)
    }

    fn heal(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        _info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.heal(querier, env).into()
    }

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<ContractStateResponse> {
        self.handler.state(now, due_projection, querier)
    }

    fn reply(self, querier: QuerierWrapper<'_>, env: Env, msg: Reply) -> ContractResult<Response> {
        self.handler
            .reply(querier, env, msg)
            .map(state_machine::from)
            .map_err(Into::into)
    }

    fn on_time_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        _info: MessageInfo,
    ) -> ContractResult<Response> {
        self.handler.on_time_alarm(querier, env).into()
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
