use serde::{Deserialize, Serialize};

use currency::CurrencyDTO;
use dex::{
    Account, CoinVisitor, ContractInSwap, IterNext, IterState, StartLocalLocalState, SwapState,
    SwapTask, TransferInFinishState, TransferInInitState, TransferOutState,
};
use finance::{coin::CoinDTO, duration::Duration};
use oracle::stub::SwapPath;
use sdk::cosmwasm_std::{Env, QuerierWrapper, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{
        query::{
            opened::{OngoingTrx, RepayTrx},
            StateResponse as QueryStateResponse,
        },
        LeasePaymentCurrencies, PaymentCoin,
    },
    contract::{
        state::{
            opened::{self, repay},
            resp_delivery::ForwardToDexEntry,
            StateResponse as ContractStateResponse, SwapClient, SwapResult,
        },
        Lease,
    },
    error::ContractResult,
    event::Type,
    finance::LpnCurrencies,
};

pub(super) type StartState =
    StartLocalLocalState<BuyLpn, LeasePaymentCurrencies, SwapClient, ForwardToDexEntry>;
pub(crate) type DexState =
    dex::StateLocalOut<BuyLpn, LeasePaymentCurrencies, SwapClient, ForwardToDexEntry>;

pub(in super::super) fn start(lease: Lease, payment: PaymentCoin) -> StartState {
    dex::start_local_local(BuyLpn::new(lease, payment))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BuyLpn {
    lease: Lease,
    payment: PaymentCoin,
}

impl BuyLpn {
    fn new(lease: Lease, payment: PaymentCoin) -> Self {
        Self { lease, payment }
    }

    fn query(
        self,
        in_progress: RepayTrx,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<ContractStateResponse> {
        let in_progress = OngoingTrx::Repayment {
            payment: self.payment,
            in_progress,
        };

        opened::lease_state(self.lease, Some(in_progress), now, due_projection, querier)
    }
}

impl SwapTask for BuyLpn {
    type InG = LeasePaymentCurrencies;
    type OutG = LpnCurrencies;
    type InOutG = LeasePaymentCurrencies;
    type Label = Type;
    type StateResponse = ContractResult<QueryStateResponse>;
    type Result = SwapResult;

    fn label(&self) -> Self::Label {
        Type::RepaymentSwap
    }

    fn dex_account(&self) -> &Account {
        &self.lease.dex
    }

    fn oracle(&self) -> &impl SwapPath<Self::InOutG> {
        &self.lease.lease.oracle
    }

    fn time_alarm(&self) -> &TimeAlarmsRef {
        &self.lease.lease.time_alarms
    }

    fn out_currency(&self) -> CurrencyDTO<Self::OutG> {
        self.lease.lease.loan.lpp().lpn()
    }

    fn on_coins<Visitor>(&self, visitor: &mut Visitor) -> Result<IterState, Visitor::Error>
    where
        Visitor: CoinVisitor<GIn = Self::InG, Result = IterNext>,
    {
        dex::on_coin(&self.payment, visitor)
    }

    fn finish(
        self,
        amount_out: CoinDTO<Self::OutG>,
        env: &Env,
        querier: QuerierWrapper<'_>,
    ) -> Self::Result {
        repay::repay(self.lease, amount_out, env, querier)
    }
}

impl<DexState> ContractInSwap<DexState> for BuyLpn
where
    DexState: InProgressTrx,
{
    type StateResponse = <Self as SwapTask>::StateResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        self.query(DexState::trx_in_progress(), now, due_projection, querier)
    }
}

trait InProgressTrx {
    fn trx_in_progress() -> RepayTrx;
}

impl InProgressTrx for TransferOutState {
    fn trx_in_progress() -> RepayTrx {
        RepayTrx::TransferOut
    }
}

impl InProgressTrx for SwapState {
    fn trx_in_progress() -> RepayTrx {
        RepayTrx::Swap
    }
}

impl InProgressTrx for TransferInInitState {
    fn trx_in_progress() -> RepayTrx {
        RepayTrx::TransferInInit
    }
}

impl InProgressTrx for TransferInFinishState {
    fn trx_in_progress() -> RepayTrx {
        RepayTrx::TransferInFinish
    }
}
