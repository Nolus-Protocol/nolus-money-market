use serde::ser::Serialize;

use finance::duration::Duration;
use platform::batch::Batch;
use sdk::cosmwasm_std::{Binary, Env, QuerierWrapper, Timestamp};

pub use crate::error::Result as DexResult;

pub use self::{
    account::Account,
    connectable::DexConnectable,
    ica_connector::{
        Enterable, IcaConnectee, IcaConnector, ICS27_MESSAGE_ENTERING_NEXT_STATE,
        NO_ICS27_MESSAGE_ENTERING_NEXT_STATE,
    },
    out_local::{
        start_local_local, start_remote_local, StartLocalLocalState, StartRemoteLocalState,
        StartTransferInState, State as StateLocalOut,
    },
    out_remote::{start as start_local_remote, StartLocalRemoteState, State as StateRemoteOut},
    resp_delivery::{ICAOpenResponseDelivery, ResponseDelivery},
    response::{ContinueResult, Handler, Response, Result},
    swap_coins::{on_coin, on_coins},
    swap_exact_in::SwapExactIn,
    swap_task::{CoinVisitor, CoinsNb, IterNext, IterState, SwapTask},
    transfer_in_finish::TransferInFinish,
    transfer_in_init::TransferInInit,
    transfer_out::TransferOut,
    SwapTask as SwapTaskT,
};
#[cfg(feature = "migration")]
pub use migration::{InspectSpec, MigrateSpec};

mod account;
mod coin_index;
mod connectable;
mod filter;
mod ica_connector;
#[cfg(feature = "migration")]
mod migration;
mod out_local;
mod out_remote;
mod resp_delivery;
mod response;
mod swap_coins;
mod swap_exact_in;
mod swap_task;
mod timeout;
mod transfer_in;
mod transfer_in_finish;
mod transfer_in_init;
mod transfer_out;
mod trx;

pub type TransferOutRespDelivery<SwapTask, SEnum, SwapGroup, SwapClient, ForwardToInnerMsg> =
    ResponseDelivery<TransferOut<SwapTask, SEnum, SwapGroup, SwapClient>, ForwardToInnerMsg>;

pub type SwapExactInRespDelivery<SwapTask, SEnum, SwapGroup, SwapClient, ForwardToInnerMsg> =
    ResponseDelivery<SwapExactIn<SwapTask, SEnum, SwapGroup, SwapClient>, ForwardToInnerMsg>;

pub type TransferInInitRespDelivery<SwapTask, SEnum, ForwardToInnerMsg> =
    ResponseDelivery<TransferInInit<SwapTask, SEnum>, ForwardToInnerMsg>;

/// Contract during DEX
pub trait Contract
where
    Self: Sized,
{
    type StateResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse;
}

pub struct TransferOutState {}
pub struct SwapState {}
pub struct TransferInInitState {}
pub struct TransferInFinishState {}

/// Contract in a swap state
///
/// The states are `TransferOutState`, `SwapState`, `TransferInInitState`, and `TransferInFinishState`
pub trait ContractInSwap<State>
where
    Self: Sized,
{
    type StateResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse;
}

/// The message that the integrating module should propagate to `Handler::on_inner`
pub trait ForwardToInner {
    type Msg: Serialize;

    fn msg() -> Self::Msg;
}

pub(crate) fn forward_to_inner<H, ForwardToInnerMsg, SEnum>(
    inner: H,
    response: Binary,
    env: Env,
) -> Result<SEnum>
where
    ForwardToInnerMsg: ForwardToInner,
    SEnum: Handler,
    ResponseDelivery<H, ForwardToInnerMsg>: Into<SEnum::Response>,
{
    let next_state = ResponseDelivery::<H, ForwardToInnerMsg>::new(inner, response);
    next_state
        .enter(env.contract.address)
        .and_then(|msgs| response::res_continue::<_, _, SEnum>(msgs, next_state))
        .into()
}

pub(crate) fn forward_to_inner_ica<H, ForwardToInnerContinueMsg, SEnum>(
    inner: H,
    counterparty_version: String,
    env: Env,
) -> ContinueResult<SEnum>
where
    ForwardToInnerContinueMsg: ForwardToInner,
    SEnum: Handler,
    ICAOpenResponseDelivery<H, ForwardToInnerContinueMsg>: Into<SEnum::Response>,
{
    let next_state =
        ICAOpenResponseDelivery::<H, ForwardToInnerContinueMsg>::new(inner, counterparty_version);
    next_state
        .enter(env.contract.address)
        .and_then(|msgs| response::res_continue::<_, _, SEnum>(msgs, next_state))
}

pub trait TimeAlarm {
    fn setup_alarm(&self, forr: Timestamp) -> DexResult<Batch>;
}
