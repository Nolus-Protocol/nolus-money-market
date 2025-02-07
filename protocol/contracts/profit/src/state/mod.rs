use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use currencies::PaymentGroup;
use dex::{
    ConnectionParams, ContinueResult, Contract, Handler, Response as DexResponse,
    Result as DexResult, StateLocalOut,
};
use finance::duration::Duration;
use platform::{
    batch::Batch,
    state_machine::{self, Response as StateMachineResponse},
};
use sdk::{
    cosmwasm_std::{Binary, Env, QuerierWrapper, Reply as CwReply, Storage, Timestamp},
    cw_storage_plus::Item,
};
use swap::Impl;

use crate::{
    error::ContractError, msg::ConfigResponse, result::ContractResult, typedefs::CadenceHours,
};

pub(crate) use self::config::Config;
use self::{buy_back::BuyBack, idle::Idle, open_ica::OpenIca, resp_delivery::ForwardToDexEntry};

mod buy_back;
mod config;
mod idle;
mod open_ica;
mod resp_delivery;

const STATE: Item<State> = Item::new("contract_state");

type IcaConnector = dex::IcaConnector<OpenIca, ContractResult<DexResponse<Idle>>>;
type ProfitCurrencies = PaymentGroup;
type SwapClient = Impl;

pub(crate) trait ConfigManagement
where
    Self: Sized,
{
    fn try_update_config(
        self,
        _: Timestamp,
        _: CadenceHours,
    ) -> ContractResult<StateMachineResponse<Self>> {
        Err(ContractError::unsupported_operation(
            "Configuration changes are not allowed in this state!",
        ))
    }
}

#[derive(Serialize, Deserialize)]
enum StateEnum {
    OpenIca(IcaConnector),
    Idle(Idle),
    BuyBack(StateLocalOut<BuyBack, PaymentGroup, SwapClient, ForwardToDexEntry>),
}

#[derive(Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct State(StateEnum);

impl ConfigManagement for State {
    fn try_update_config(
        self,
        now: Timestamp,
        cadence_hours: CadenceHours,
    ) -> ContractResult<StateMachineResponse<Self>> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica
                .try_update_config(now, cadence_hours)
                .map(state_machine::from),
            StateEnum::Idle(idle) => idle
                .try_update_config(now, cadence_hours)
                .map(state_machine::from),
            StateEnum::BuyBack(buy_back) => buy_back
                .try_update_config(now, cadence_hours)
                .map(state_machine::from),
        }
    }
}

impl State {
    pub fn start(config: Config, dex: ConnectionParams) -> (Self, Batch) {
        let init_state = IcaConnector::new(OpenIca::new(config, dex));

        let response = init_state.enter();
        let state: State = init_state.into();
        (state, response)
    }

    pub fn load(storage: &dyn Storage) -> ContractResult<Self> {
        STATE.load(storage).map_err(Into::into)
    }

    pub fn store(&self, storage: &mut dyn Storage) -> ContractResult<()> {
        STATE.save(storage, self).map_err(Into::into)
    }
}

impl From<IcaConnector> for State {
    fn from(value: IcaConnector) -> Self {
        Self(StateEnum::OpenIca(value))
    }
}

impl From<Idle> for State {
    fn from(value: Idle) -> Self {
        Self(StateEnum::Idle(value))
    }
}

impl From<StateLocalOut<BuyBack, ProfitCurrencies, SwapClient, ForwardToDexEntry>> for State {
    fn from(
        value: StateLocalOut<BuyBack, ProfitCurrencies, SwapClient, ForwardToDexEntry>,
    ) -> Self {
        Self(StateEnum::BuyBack(value))
    }
}

impl Contract for State {
    type StateResponse = ConfigResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        match self.0 {
            StateEnum::OpenIca(open_ica) => open_ica.state(now, due_projection, querier),
            StateEnum::Idle(idle) => idle.state(now, due_projection, querier),
            StateEnum::BuyBack(buy_back) => buy_back.state(now, due_projection, querier),
        }
    }
}

impl Handler for State {
    type Response = State;
    type SwapResult = ContractResult<DexResponse<State>>;

    fn on_open_ica(
        self,
        counterparty_version: String,
        querier: QuerierWrapper<'_>,
        env: Env,
    ) -> ContinueResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_open_ica(counterparty_version, querier, env),
            StateEnum::Idle(idle) => idle.on_open_ica(counterparty_version, querier, env),
            StateEnum::BuyBack(buy_back) => buy_back
                .on_open_ica(counterparty_version, querier, env)
                .map(state_machine::from),
        }
    }

    fn on_response(self, data: Binary, querier: QuerierWrapper<'_>, env: Env) -> DexResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_response(data, querier, env).map_into(),
            StateEnum::Idle(idle) => idle.on_response(data, querier, env).map_into(),
            StateEnum::BuyBack(buy_back) => buy_back.on_response(data, querier, env).map_into(),
        }
    }

    fn on_error(self, querier: QuerierWrapper<'_>, env: Env) -> ContinueResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_error(querier, env),
            StateEnum::Idle(idle) => idle.on_error(querier, env),
            StateEnum::BuyBack(buy_back) => {
                buy_back.on_error(querier, env).map(state_machine::from)
            }
        }
    }

    fn on_timeout(self, querier: QuerierWrapper<'_>, env: Env) -> ContinueResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_timeout(querier, env).map(state_machine::from),
            StateEnum::Idle(idle) => idle.on_timeout(querier, env).map(state_machine::from),
            StateEnum::BuyBack(buy_back) => {
                buy_back.on_timeout(querier, env).map(state_machine::from)
            }
        }
    }

    fn on_inner(self, querier: QuerierWrapper<'_>, env: Env) -> DexResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_inner(querier, env).map_into(),
            StateEnum::Idle(idle) => idle.on_inner(querier, env).map_into(),
            StateEnum::BuyBack(buy_back) => buy_back.on_inner(querier, env).map_into(),
        }
    }

    fn on_inner_continue(self, querier: QuerierWrapper<'_>, env: Env) -> ContinueResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_inner_continue(querier, env).map(state_machine::from),
            StateEnum::Idle(idle) => idle
                .on_inner_continue(querier, env)
                .map(state_machine::from),
            StateEnum::BuyBack(buy_back) => buy_back
                .on_inner_continue(querier, env)
                .map(state_machine::from),
        }
    }

    fn reply(self, querier: QuerierWrapper<'_>, env: Env, msg: CwReply) -> ContinueResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.reply(querier, env, msg).map(state_machine::from),
            StateEnum::Idle(idle) => idle.reply(querier, env, msg).map(state_machine::from),
            StateEnum::BuyBack(buy_back) => {
                buy_back.reply(querier, env, msg).map(state_machine::from)
            }
        }
    }

    fn on_time_alarm(self, querier: QuerierWrapper<'_>, env: Env) -> DexResult<Self> {
        match self.0 {
            StateEnum::OpenIca(ica) => ica.on_time_alarm(querier, env).map_into(),
            StateEnum::Idle(idle) => idle.on_time_alarm(querier, env).map_into(),
            StateEnum::BuyBack(buy_back) => buy_back.on_time_alarm(querier, env).map_into(),
        }
    }
}

impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            StateEnum::OpenIca(ica) => Display::fmt(&ica, f),
            StateEnum::Idle(idle) => Display::fmt(&idle, f),
            StateEnum::BuyBack(buy_back) => Display::fmt(&buy_back, f),
        }
    }
}
