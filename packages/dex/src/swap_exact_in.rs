use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};

use currency::{Group, SymbolSlice};
use finance::{
    coin::{self, Amount, CoinDTO},
    zero::Zero,
};
use platform::{batch::Batch, trx};
use sdk::{
    cosmos_sdk_proto::cosmos::base::abci::v1beta1::MsgData,
    cosmwasm_std::{Binary, Deps, Env, QuerierWrapper, Timestamp},
};
use swap::trx as swap_trx;

#[cfg(debug_assertions)]
use crate::swap_task::IterState;
use crate::{
    connectable::DexConnectable,
    connection::ConnectionParams,
    error::{Error, Result},
    filter::CurrencyFilter,
    ica_connector::Enterable,
    response::{self, ContinueResult, Handler, Result as HandlerResult},
    swap_task::{CoinVisitor, IterNext, SwapTask as SwapTaskT},
    timeout,
    transfer_in_init::TransferInInit,
    trx::SwapTrx,
    ContractInSwap, ForwardToInner, TimeAlarm,
};
#[cfg(feature = "migration")]
use crate::{InspectSpec, MigrateSpec};

use super::{Contract, SwapState};

#[derive(Serialize, Deserialize)]
pub struct SwapExactIn<SwapTask, SEnum> {
    spec: SwapTask,
    #[serde(skip)]
    _state_enum: PhantomData<SEnum>,
}

impl<SwapTask, SEnum> SwapExactIn<SwapTask, SEnum>
where
    Self: Into<SEnum>,
{
    pub(super) fn new(spec: SwapTask) -> Self {
        Self {
            spec,
            _state_enum: PhantomData,
        }
    }
}

impl<SwapTask, SEnum> SwapExactIn<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    pub(super) fn enter_state(
        &self,
        _now: Timestamp,
        querier: &QuerierWrapper<'_>,
    ) -> Result<Batch> {
        let swap_trx = self.spec.dex_account().swap(self.spec.oracle(), querier);
        // TODO apply nls_swap_fee on the downpayment only!
        struct SwapWorker<'a>(SwapTrx<'a>, &'a SymbolSlice);
        impl<'a> CoinVisitor for SwapWorker<'a> {
            type Result = IterNext;
            type Error = Error;

            fn visit<G>(&mut self, coin: &CoinDTO<G>) -> Result<Self::Result>
            where
                G: Group,
            {
                self.0.swap_exact_in(coin, self.1)?;
                Ok(IterNext::Continue)
            }
        }

        let mut swapper = SwapWorker(swap_trx, self.spec.out_currency());
        let mut filtered_swapper = CurrencyFilter::new(&mut swapper, self.spec.out_currency());
        let _res = self.spec.on_coins(&mut filtered_swapper)?;

        #[cfg(debug_assertions)]
        self.debug_check(&filtered_swapper, _res);

        Ok(swapper.0.into())
    }

    fn decode_response(&self, resp: &[u8], spec: &SwapTask) -> Result<CoinDTO<SwapTask::OutG>> {
        struct ExactInResponse<I>(I, Amount);
        impl<I> CoinVisitor for ExactInResponse<I>
        where
            I: Iterator<Item = MsgData>,
        {
            type Result = IterNext;
            type Error = Error;

            fn visit<G>(&mut self, _coin: &CoinDTO<G>) -> Result<Self::Result>
            where
                G: Group,
            {
                self.1 += swap_trx::exact_amount_in_resp(&mut self.0)?;
                Ok(IterNext::Continue)
            }
        }
        let mut resp = ExactInResponse(trx::decode_msg_responses(resp)?, Amount::ZERO);
        let mut filtered_resp = CurrencyFilter::new(&mut resp, self.spec.out_currency());
        let _res = self.spec.on_coins(&mut filtered_resp)?;

        #[cfg(debug_assertions)]
        self.debug_check(&filtered_resp, _res);

        coin::from_amount_ticker(
            filtered_resp.filtered() + resp.1,
            spec.out_currency().into(),
        )
        .map_err(Into::into)
    }

    #[cfg(debug_assertions)]
    fn debug_check<V>(&self, filter: &CurrencyFilter<'_, V>, res: IterState)
    where
        V: CoinVisitor,
    {
        debug_assert!(
            filter.passed_any(),
            "No coins with currency != {}",
            self.spec.out_currency()
        );
        debug_assert_eq!(res, IterState::Complete);
    }
}

impl<SwapTask, SEnum> Enterable for SwapExactIn<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn enter(&self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Result<Batch> {
        self.enter_state(now, querier)
    }
}

impl<SwapTask, SEnum> DexConnectable for SwapExactIn<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn dex(&self) -> &ConnectionParams {
        self.spec.dex_account().dex()
    }
}

impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> Handler
    for SwapExactIn<
        SwapTask,
        super::out_local::State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>,
    >
where
    SwapTask: SwapTaskT,
    ForwardToInnerMsg: ForwardToInner,
{
    type Response = super::out_local::State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>;
    type SwapResult = SwapTask::Result;

    fn on_response(self, resp: Binary, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        // TODO transfer (downpayment - transferred_and_swapped), i.e. the nls_swap_fee to the profit
        self.decode_response(resp.as_slice(), &self.spec)
            .map(|amount_out| TransferInInit::new(self.spec, amount_out))
            .and_then(|next_state| {
                next_state
                    .enter(env.block.time, &deps.querier)
                    .and_then(|resp| response::res_continue::<_, _, Self>(resp, next_state))
            })
            .into()
    }

    fn on_timeout(self, _deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
        let state_label = self.spec.label();
        let timealarms = self.spec.time_alarm().clone();
        timeout::on_timeout_repair_channel(self, state_label, timealarms, env)
    }
}

impl<OpenIca, SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> Handler
    for SwapExactIn<
        SwapTask,
        super::out_remote::State<OpenIca, SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>,
    >
where
    SwapTask: SwapTaskT,
{
    type Response =
        super::out_remote::State<OpenIca, SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>;
    type SwapResult = SwapTask::Result;

    fn on_response(self, resp: Binary, deps: Deps<'_>, env: Env) -> HandlerResult<Self> {
        // TODO transfer (downpayment - transferred_and_swapped), i.e. the nls_swap_fee to the profit
        self.decode_response(resp.as_slice(), &self.spec)
            .map_or_else(
                |err| HandlerResult::Continue(Err(err)),
                |amount_out| {
                    response::res_finished(self.spec.finish(amount_out, &env, &deps.querier))
                },
            )
    }

    fn on_timeout(self, _deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
        let state_label = self.spec.label();
        let timealarms = self.spec.time_alarm().clone();
        timeout::on_timeout_repair_channel(self, state_label, timealarms, env)
    }
}

impl<SwapTask, SEnum> Contract for SwapExactIn<SwapTask, SEnum>
where
    SwapTask: ContractInSwap<SwapState, <SwapTask as SwapTaskT>::StateResponse> + SwapTaskT,
{
    type StateResponse = <SwapTask as SwapTaskT>::StateResponse;

    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
        self.spec.state(now, querier)
    }
}

impl<SwapTask, SEnum> Display for SwapExactIn<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("SwapExactIn at {}", self.spec.label().into()))
    }
}

impl<SwapTask, SEnum> TimeAlarm for SwapExactIn<SwapTask, SEnum>
where
    SwapTask: SwapTaskT,
{
    fn setup_alarm(&self, forr: Timestamp) -> Result<Batch> {
        self.spec.time_alarm().setup_alarm(forr).map_err(Into::into)
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, SwapTaskNew, SEnum, SEnumNew> MigrateSpec<SwapTask, SwapTaskNew, SEnumNew>
    for SwapExactIn<SwapTask, SEnum>
where
    Self: Sized,
    SwapExactIn<SwapTaskNew, SEnumNew>: Into<SEnumNew>,
{
    type Out = SwapExactIn<SwapTaskNew, SEnumNew>;

    fn migrate_spec<MigrateFn>(self, migrate_fn: MigrateFn) -> Self::Out
    where
        MigrateFn: FnOnce(SwapTask) -> SwapTaskNew,
    {
        Self::Out::new(migrate_fn(self.spec))
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, R, SEnum> InspectSpec<SwapTask, R> for SwapExactIn<SwapTask, SEnum> {
    fn inspect_spec<InspectFn>(&self, inspect_fn: InspectFn) -> R
    where
        InspectFn: FnOnce(&SwapTask) -> R,
    {
        inspect_fn(&self.spec)
    }
}
