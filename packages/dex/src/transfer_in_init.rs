use std::fmt::{Display, Formatter, Result as FmtResult};
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use finance::coin::CoinDTO;
use platform::batch::Batch;
use sdk::cosmwasm_std::{Binary, Deps, Env, QuerierWrapper, Timestamp};

use crate::{
    error::Result,
    response::{ContinueResult, Handler, Result as HandlerResult},
    swap_task::SwapTask as SwapTaskT,
    timeout,
    trx::IBC_TIMEOUT,
    ConnectionParams, Contract, ContractInSwap, DexConnectable, Enterable, TimeAlarm,
    TransferInInitPostRecoverIca, TransferInInitPreRecoverIca, TransferInInitRecoverIca,
    TransferInInitState,
};
#[cfg(feature = "migration")]
use crate::{InspectSpec, MigrateSpec};

use super::transfer_in_finish::TransferInFinish;

/// Transfer in a coin from DEX
///
#[derive(Serialize, Deserialize)]
pub struct TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    spec: SwapTask,
    amount_in: CoinDTO<SwapTask::OutG>,
    #[serde(skip)]
    _state_enum: PhantomData<SEnum>,
}

impl<SwapTask, SEnum> TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    pub fn new(spec: SwapTask, amount_in: CoinDTO<SwapTask::OutG>) -> Self {
        Self {
            spec,
            amount_in,
            _state_enum: Default::default(),
        }
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, SwapTaskNew, SEnum, SEnumNew> MigrateSpec<SwapTask, SwapTaskNew, SEnumNew>
    for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTaskNew: SwapTaskT<OutG = SwapTask::OutG>,
{
    type Out = TransferInInit<SwapTaskNew, SEnumNew>;

    fn migrate_spec<MigrateFn>(self, migrate_fn: MigrateFn) -> Self::Out
    where
        MigrateFn: FnOnce(SwapTask) -> SwapTaskNew,
    {
        Self::Out::new(migrate_fn(self.spec), self.amount_in)
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, R, SEnum> InspectSpec<SwapTask, R> for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn inspect_spec<InspectFn>(&self, inspect_fn: InspectFn) -> R
    where
        InspectFn: FnOnce(&SwapTask) -> R,
    {
        inspect_fn(&self.spec)
    }
}

impl<SwapTask, SEnum> TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn enter_state(&self, now: Timestamp) -> Result<Batch> {
        let mut sender = self.spec.dex_account().transfer_from(now);
        sender.send(&self.amount_in)?;
        Ok(sender.into())
    }
}

impl<SwapTask, SEnum> TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
    Self: Into<SEnum>,
    TransferInFinish<SwapTask, SEnum>: Into<SEnum>,
    SEnum: From<TransferInInitPreRecoverIca<SwapTask, SEnum>>,
    SEnum: From<TransferInInitRecoverIca<SwapTask, SEnum>>,
    SEnum: From<TransferInInitPostRecoverIca<SwapTask, SEnum>>,
{
    fn on_response(self, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        let finish: TransferInFinish<SwapTask, SEnum> =
            TransferInFinish::new(self.spec, self.amount_in, env.block.time + IBC_TIMEOUT);
        finish.try_complete(deps, env).map_into()
    }
}

impl<SwapTask, SEnum> DexConnectable for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn dex(&self) -> &ConnectionParams {
        self.spec.dex_account().dex()
    }
}

impl<SwapTask, SEnum> Enterable for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn enter(&self, now: Timestamp, _querier: &QuerierWrapper<'_>) -> Result<Batch> {
        self.enter_state(now)
    }
}

impl<SwapTask, SEnum> Handler for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
    Self: Into<SEnum>,
    TransferInFinish<SwapTask, SEnum>: Into<SEnum>,
    SEnum: From<TransferInInitPreRecoverIca<SwapTask, SEnum>>,
    SEnum: From<TransferInInitRecoverIca<SwapTask, SEnum>>,
    SEnum: From<TransferInInitPostRecoverIca<SwapTask, SEnum>>,
{
    type Response = SEnum;
    type SwapResult = SwapTask::Result;

    fn on_response(self, _data: Binary, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        self.on_response(deps, env)
    }

    fn on_timeout(self, _deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
        let state_label = self.spec.label();
        let timealarms = self.spec.time_alarm().clone();
        timeout::on_timeout_repair_channel(self, state_label, timealarms, env)
    }
    fn heal(self, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        self.on_response(deps, env)
    }
}

impl<SwapTask, SEnum> Contract for TransferInInit<SwapTask, SEnum>
where
    SwapTask:
        SwapTaskT + ContractInSwap<TransferInInitState, <SwapTask as SwapTaskT>::StateResponse>,
{
    type StateResponse = <SwapTask as SwapTaskT>::StateResponse;

    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
        self.spec.state(now, querier)
    }
}

impl<SwapTask, ForwardToInnerMsg> Display for TransferInInit<SwapTask, ForwardToInnerMsg>
where
    SwapTask: SwapTaskT,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "TransferInInit at {}",
            self.spec.label().into()
        ))
    }
}

impl<SwapTask, SEnum> TimeAlarm for TransferInInit<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn setup_alarm(&self, forr: Timestamp) -> Result<Batch> {
        self.spec.time_alarm().setup_alarm(forr).map_err(Into::into)
    }
}
