use serde::{Deserialize, Serialize};

use currency::{lpn::Lpns, SymbolSlice};
use dex::{
    Account, CoinVisitor, ContractInSwap, IterNext, IterState, StartLocalLocalState, SwapState,
    SwapTask, TransferInFinishState, TransferInInitState, TransferOutState,
};
use finance::coin::CoinDTO;
use oracle::stub::OracleRef;
use sdk::cosmwasm_std::{Env, QuerierWrapper, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{self, opened::RepayTrx, PaymentCoin},
    contract::{
        state::{
            opened::repay,
            resp_delivery::{ForwardToDexEntry, ForwardToDexEntryContinue},
            SwapResult,
        },
        Lease,
    },
    error::ContractResult,
    event::Type,
};

pub(super) type StartState =
    StartLocalLocalState<BuyLpn, ForwardToDexEntry, ForwardToDexEntryContinue>;
pub(crate) type DexState = dex::StateLocalOut<BuyLpn, ForwardToDexEntry, ForwardToDexEntryContinue>;

pub(in crate::contract::state) fn start(lease: Lease, payment: PaymentCoin) -> StartState {
    dex::start_local_local(BuyLpn::new(lease, payment))
}

type BuyLpnStateResponse = <BuyLpn as SwapTask>::StateResponse;

#[derive(Serialize, Deserialize)]
pub(crate) struct BuyLpn {
    lease: Lease,
    payment: PaymentCoin,
}

impl BuyLpn {
    fn new(lease: Lease, payment: PaymentCoin) -> Self {
        Self { lease, payment }
    }

    #[cfg(feature = "migration")]
    pub(super) fn migrate_to(lease: Lease, payment: PaymentCoin) -> Self {
        Self::new(lease, payment)
    }
}

impl SwapTask for BuyLpn {
    type OutG = Lpns;
    type Label = Type;
    type StateResponse = ContractResult<api::StateResponse>;
    type Result = SwapResult;

    fn label(&self) -> Self::Label {
        Type::RepaymentSwap
    }

    fn dex_account(&self) -> &Account {
        &self.lease.dex
    }

    fn oracle(&self) -> &OracleRef {
        &self.lease.lease.oracle
    }

    fn time_alarm(&self) -> &TimeAlarmsRef {
        &self.lease.lease.time_alarms
    }

    fn out_currency(&self) -> &SymbolSlice {
        self.lease.lease.loan.lpp().currency()
    }

    fn on_coins<Visitor>(&self, visitor: &mut Visitor) -> Result<IterState, Visitor::Error>
    where
        Visitor: CoinVisitor<Result = IterNext>,
    {
        dex::on_coin(&self.payment, visitor)
    }

    fn finish(
        self,
        amount_out: CoinDTO<Self::OutG>,
        env: &Env,
        querier: &QuerierWrapper<'_>,
    ) -> Self::Result {
        repay::repay(self.lease, amount_out, env, querier)
    }
}

impl ContractInSwap<TransferOutState, BuyLpnStateResponse> for BuyLpn {
    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> BuyLpnStateResponse {
        repay::query(
            self.lease,
            self.payment,
            RepayTrx::TransferOut,
            now,
            querier,
        )
    }
}

impl ContractInSwap<SwapState, BuyLpnStateResponse> for BuyLpn {
    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> BuyLpnStateResponse {
        repay::query(self.lease, self.payment, RepayTrx::Swap, now, querier)
    }
}

impl ContractInSwap<TransferInInitState, BuyLpnStateResponse> for BuyLpn {
    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> BuyLpnStateResponse {
        repay::query(
            self.lease,
            self.payment,
            RepayTrx::TransferInInit,
            now,
            querier,
        )
    }
}

impl ContractInSwap<TransferInFinishState, BuyLpnStateResponse> for BuyLpn {
    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> BuyLpnStateResponse {
        repay::query(
            self.lease,
            self.payment,
            RepayTrx::TransferInInit,
            now,
            querier,
        )
    }
}
