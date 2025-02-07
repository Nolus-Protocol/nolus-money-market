use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};

use finance::{coin::CoinDTO, duration::Duration};
use platform::{
    batch::{Emit, Emitter},
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Env, QuerierWrapper, Timestamp};

#[cfg(feature = "migration")]
use crate::{InspectSpec, MigrateSpec};

use super::{
    response::{self, Handler, Result as HandlerResult},
    swap_task::SwapTask as SwapTaskT,
    transfer_in,
    transfer_in_init::TransferInInit,
    Contract, ContractInSwap, Enterable, TransferInFinishState,
};

#[derive(Serialize, Deserialize)]
pub struct TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    spec: SwapTask,
    amount_in: CoinDTO<SwapTask::OutG>,
    timeout: Timestamp,
    #[serde(skip)]
    _state_enum: PhantomData<SEnum>,
}

impl<SwapTask, SEnum> TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    pub(super) fn new(
        spec: SwapTask,
        amount_in: CoinDTO<SwapTask::OutG>,
        timeout: Timestamp,
    ) -> Self {
        Self {
            spec,
            amount_in,
            timeout,
            _state_enum: Default::default(),
        }
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, SEnum> TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    pub fn into_init(self) -> TransferInInit<SwapTask, SEnum> {
        TransferInInit::new(self.spec, self.amount_in)
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, SwapTaskNew, SEnum, SEnumNew> MigrateSpec<SwapTask, SwapTaskNew, SEnumNew>
    for TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTaskNew: SwapTaskT<OutG = SwapTask::OutG>,
{
    type Out = TransferInFinish<SwapTaskNew, SEnumNew>;

    fn migrate_spec<MigrateFn>(self, migrate_fn: MigrateFn) -> Self::Out
    where
        MigrateFn: FnOnce(SwapTask) -> SwapTaskNew,
    {
        Self::Out::new(migrate_fn(self.spec), self.amount_in, self.timeout)
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, R, SEnum> InspectSpec<SwapTask, R> for TransferInFinish<SwapTask, SEnum>
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

impl<SwapTask, SEnum> TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
    Self: Into<SEnum>,
    TransferInInit<SwapTask, SEnum>: Into<SEnum>,
{
    pub(super) fn try_complete(self, querier: QuerierWrapper<'_>, env: Env) -> HandlerResult<Self> {
        transfer_in::check_received(&self.amount_in, &env.contract.address, querier).map_or_else(
            Into::into,
            |received| {
                if received {
                    self.complete(&env, querier)
                } else {
                    self.try_again(env, querier)
                }
            },
        )
    }

    fn complete(self, env: &Env, querier: QuerierWrapper<'_>) -> HandlerResult<Self> {
        response::res_finished(self.spec.finish(self.amount_in, env, querier))
    }

    fn try_again(self, env: Env, querier: QuerierWrapper<'_>) -> HandlerResult<Self> {
        let now = env.block.time;
        let emitter = self.emit_ok();
        if now >= self.timeout {
            let next_state = TransferInInit::new(self.spec, self.amount_in);
            next_state
                .enter(now, querier)
                .map(|batch| MessageResponse::messages_with_events(batch, emitter))
                .and_then(|resp| response::res_continue::<_, _, Self>(resp, next_state))
                .into()
        } else {
            transfer_in::setup_alarm(self.spec.time_alarm(), now)
                .map(|batch| MessageResponse::messages_with_events(batch, emitter))
                .and_then(|resp| response::res_continue::<_, _, Self>(resp, self))
                .into()
        }
    }

    fn emit_ok(&self) -> Emitter {
        Emitter::of_type(self.spec.label())
            .emit("stage", "transfer-in")
            .emit_coin_dto("amount", &self.amount_in)
    }
}

impl<SwapTask, SEnum> Handler for TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
    Self: Into<SEnum>,
    TransferInInit<SwapTask, SEnum>: Into<SEnum>,
{
    type Response = SEnum;
    type SwapResult = SwapTask::Result;

    fn heal(self, querier: QuerierWrapper<'_>, env: Env) -> HandlerResult<Self> {
        self.on_time_alarm(querier, env)
    }

    fn on_time_alarm(self, querier: QuerierWrapper<'_>, env: Env) -> HandlerResult<Self> {
        self.try_complete(querier, env)
    }
}

impl<SwapTask, SEnum> Contract for TransferInFinish<SwapTask, SEnum>
where
    SwapTask: SwapTaskT
        + ContractInSwap<
            TransferInFinishState,
            StateResponse = <SwapTask as SwapTaskT>::StateResponse,
        >,
{
    type StateResponse = <SwapTask as SwapTaskT>::StateResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        self.spec.state(now, due_projection, querier)
    }
}

impl<SwapTask, ForwardToInnerMsg> Display for TransferInFinish<SwapTask, ForwardToInnerMsg>
where
    SwapTask: SwapTaskT,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "TransferInFinish at {}",
            self.spec.label().into()
        ))
    }
}
