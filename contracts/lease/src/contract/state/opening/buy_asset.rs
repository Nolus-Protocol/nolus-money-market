use serde::{Deserialize, Serialize};

use currency::{lease::LeaseGroup, Symbol};
use dex::{
    Account, CoinVisitor, ContractInSwap, IterNext, IterState, StartLocalRemoteState, SwapState,
    SwapTask, TransferOutState,
};
use finance::coin::CoinDTO;
use lpp::stub::LppRef;
use oracle::stub::OracleRef;
use platform::{
    ica::HostAccount, message::Response as MessageResponse,
    state_machine::Response as StateMachineResponse,
};
use sdk::cosmwasm_std::{Env, QuerierWrapper, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{
        self, opening::OngoingTrx, DownpaymentCoin, NewLeaseContract, NewLeaseForm, StateResponse,
    },
    contract::{
        cmd::{self, OpenLoanRespResult},
        state::{
            opened::active::Active,
            resp_delivery::{ForwardToDexEntry, ForwardToDexEntryContinue},
            SwapResult,
        },
        Lease,
    },
    error::ContractResult,
    event::Type,
    lease::IntoDTOResult,
};

use super::open_ica::OpenIcaAccount;

type AssetGroup = LeaseGroup;
pub(super) type StartState = StartLocalRemoteState<OpenIcaAccount, BuyAsset>;
pub(in crate::contract::state) type DexState =
    dex::StateRemoteOut<OpenIcaAccount, BuyAsset, ForwardToDexEntry, ForwardToDexEntryContinue>;

pub(in crate::contract::state::opening) fn start(
    new_lease: NewLeaseContract,
    downpayment: DownpaymentCoin,
    loan: OpenLoanRespResult,
    deps: (LppRef, OracleRef, TimeAlarmsRef),
    start_opening_at: Timestamp,
) -> StartState {
    dex::start_local_remote::<_, BuyAsset>(OpenIcaAccount::new(
        new_lease,
        downpayment,
        loan,
        deps,
        start_opening_at,
    ))
}

type BuyAssetStateResponse = <BuyAsset as SwapTask>::StateResponse;

#[derive(Serialize, Deserialize)]
pub(crate) struct BuyAsset {
    form: NewLeaseForm,
    dex_account: Account,
    downpayment: DownpaymentCoin,
    loan: OpenLoanRespResult,
    deps: (LppRef, OracleRef, TimeAlarmsRef),
    start_opening_at: Timestamp,
}

impl BuyAsset {
    pub(super) fn new(
        form: NewLeaseForm,
        dex_account: Account,
        downpayment: DownpaymentCoin,
        loan: OpenLoanRespResult,
        deps: (LppRef, OracleRef, TimeAlarmsRef),
        start_opening_at: Timestamp,
    ) -> Self {
        Self {
            form,
            dex_account,
            downpayment,
            loan,
            deps,
            start_opening_at,
        }
    }

    fn state<InP>(self, in_progress_fn: InP) -> BuyAssetStateResponse
    where
        InP: FnOnce(String) -> OngoingTrx,
    {
        Ok(StateResponse::Opening {
            downpayment: self.downpayment,
            loan: self.loan.principal,
            loan_interest_rate: self.loan.annual_interest_rate,
            in_progress: in_progress_fn(HostAccount::from(self.dex_account).into()),
        })
    }

    // fn emit_ok(&self) -> Emitter {
    //     Emitter::of_type(Type::OpeningTransferOut)
    // }
}

impl SwapTask for BuyAsset {
    type OutG = AssetGroup;
    type Label = Type;
    type StateResponse = ContractResult<api::StateResponse>;
    type Result = SwapResult;

    fn label(&self) -> Self::Label {
        Type::OpeningSwap
    }

    fn dex_account(&self) -> &Account {
        &self.dex_account
    }

    fn oracle(&self) -> &OracleRef {
        &self.deps.1
    }

    fn time_alarm(&self) -> &TimeAlarmsRef {
        &self.deps.2
    }

    fn out_currency(&self) -> Symbol<'_> {
        &self.form.currency
    }

    fn on_coins<Visitor>(&self, visitor: &mut Visitor) -> Result<IterState, Visitor::Error>
    where
        Visitor: CoinVisitor<Result = IterNext>,
    {
        dex::on_coins(&self.downpayment, &self.loan.principal, visitor)
    }

    fn finish(
        self,
        amount_out: CoinDTO<Self::OutG>,
        env: &Env,
        querier: &QuerierWrapper<'_>,
    ) -> Self::Result {
        let IntoDTOResult { lease, batch } = cmd::open_lease(
            self.form,
            self.dex_account.owner().clone(),
            self.start_opening_at,
            &amount_out,
            querier,
            (self.deps.0, self.deps.1),
        )?;

        let active = Active::new(Lease {
            lease,
            dex: self.dex_account,
        });
        let emitter = active.emit_opened(env, self.downpayment, self.loan);
        Ok(StateMachineResponse::from(
            MessageResponse::messages_with_events(batch, emitter),
            active,
        ))
    }
}

impl ContractInSwap<TransferOutState, BuyAssetStateResponse> for BuyAsset {
    fn state(self, _now: Timestamp, _querier: &QuerierWrapper<'_>) -> BuyAssetStateResponse {
        let in_progress_fn = |ica_account| OngoingTrx::TransferOut { ica_account };
        self.state(in_progress_fn)
    }
}

impl ContractInSwap<SwapState, BuyAssetStateResponse> for BuyAsset {
    fn state(self, _now: Timestamp, _querier: &QuerierWrapper<'_>) -> BuyAssetStateResponse {
        let in_progress_fn = |ica_account| OngoingTrx::BuyAsset { ica_account };
        self.state(in_progress_fn)
    }
}
