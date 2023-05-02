use serde::{Deserialize, Serialize};

use finance::coin::CoinDTO;
use platform::{
    batch::{Emit, Emitter},
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Deps, Env, QuerierWrapper, Timestamp};

use crate::{
    response::{self, Handler, Result as HandlerResult},
    swap_task::SwapTask as SwapTaskT,
    transfer_in, Contract, ContractInSwap, Enterable, TransferInFinishState,
};

use super::transfer_in_init::TransferInInit;

#[derive(Serialize, Deserialize)]
pub struct TransferInFinish<SwapTask>
where
    SwapTask: SwapTaskT,
{
    spec: SwapTask,
    amount_in: CoinDTO<SwapTask::OutG>,
    timeout: Timestamp,
}

impl<SwapTask> TransferInFinish<SwapTask>
where
    SwapTask: SwapTaskT,
{
    #[cfg(feature = "migration")]
    pub fn migrate_from(
        spec: SwapTask,
        amount_in: CoinDTO<SwapTask::OutG>,
        timeout: Timestamp,
    ) -> Self {
        Self::new(spec, amount_in, timeout)
    }

    pub(super) fn new(
        spec: SwapTask,
        amount_in: CoinDTO<SwapTask::OutG>,
        timeout: Timestamp,
    ) -> Self {
        Self {
            spec,
            amount_in,
            timeout,
        }
    }
}

impl<SwapTask> TransferInFinish<SwapTask>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
{
    pub(super) fn try_complete(self, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        transfer_in::check_received(&self.amount_in, &env.contract.address, &deps.querier)
            .map_or_else(Into::into, |received| {
                if received {
                    self.complete(&env, &deps.querier)
                } else {
                    self.try_again(env, deps)
                }
            })
    }

    fn complete(self, env: &Env, querier: &QuerierWrapper<'_>) -> HandlerResult<Self> {
        response::res_finished(self.spec.finish(self.amount_in, env, querier))
    }

    fn try_again(self, env: Env, deps: Deps<'_>) -> HandlerResult<Self> {
        let now = env.block.time;
        let emitter = self.emit_ok();
        if now >= self.timeout {
            let next_state = TransferInInit::new(self.spec, self.amount_in);
            next_state
                .enter(now, &deps.querier)
                .map(|batch| MessageResponse::messages_with_events(batch, emitter))
                .and_then(|resp| response::res_continue::<_, _, Self>(resp, next_state))
                .into()
        } else {
            transfer_in::setup_alarm(self.spec.time_alarm().clone(), now)
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

impl<SwapTask> Handler for TransferInFinish<SwapTask>
where
    SwapTask: SwapTaskT,
    SwapTask::OutG: Clone,
{
    type Response = super::out_local::State<SwapTask>;
    type SwapResult = SwapTask::Result;

    fn on_time_alarm(self, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        self.try_complete(deps, env)
    }
}

impl<SwapTask> Contract for TransferInFinish<SwapTask>
where
    SwapTask:
        SwapTaskT + ContractInSwap<TransferInFinishState, <SwapTask as SwapTaskT>::StateResponse>,
{
    type StateResponse = <SwapTask as SwapTaskT>::StateResponse;

    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
        self.spec.state(now, querier)
    }
}
