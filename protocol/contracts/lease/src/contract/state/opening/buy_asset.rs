use oracle::stub::SwapPath;
use profit::stub::ProfitRef;
use serde::{Deserialize, Serialize};

use currency::CurrencyDTO;
use dex::{
    Account, CoinVisitor, ContractInSwap, IterNext, IterState, StartLocalRemoteState, SwapState,
    SwapTask, TransferOutState,
};
use finance::{coin::CoinDTO, duration::Duration};
use platform::{
    ica::HostAccount, message::Response as MessageResponse,
    state_machine::Response as StateMachineResponse,
};
use sdk::cosmwasm_std::{Env, QuerierWrapper, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{
        open::{NewLeaseContract, NewLeaseForm},
        query::{opening::OngoingTrx, StateResponse as QueryStateResponse},
        DownpaymentCoin, LeaseAssetCurrencies, LeasePaymentCurrencies,
    },
    contract::{
        cmd::{CloseStatusDTO, LeaseFactory, OpenLeaseResult, OpenLoanRespResult},
        finalize::FinalizerRef,
        state::{
            opened::{active::Active, close::liquidation},
            resp_delivery::{ForwardToDexEntry, ForwardToDexEntryContinue},
            SwapClient, SwapResult,
        },
        Lease,
    },
    error::ContractResult,
    event::Type,
    finance::{LppRef, OracleRef, ReserveRef},
    lease::with_lease_deps,
    position::PositionDTO,
};

use super::open_ica::OpenIcaAccount;

type AssetGroup = LeaseAssetCurrencies;
pub(super) type StartState = StartLocalRemoteState<OpenIcaAccount, BuyAsset>;
pub(in super::super) type DexState = dex::StateRemoteOut<
    OpenIcaAccount,
    BuyAsset,
    LeasePaymentCurrencies,
    SwapClient,
    ForwardToDexEntry,
    ForwardToDexEntryContinue,
>;

pub(super) fn start(
    new_lease: NewLeaseContract,
    downpayment: DownpaymentCoin,
    loan: OpenLoanRespResult,
    deps: (LppRef, OracleRef, TimeAlarmsRef, FinalizerRef),
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
    deps: (LppRef, OracleRef, TimeAlarmsRef, FinalizerRef),
    start_opening_at: Timestamp,
}

impl BuyAsset {
    pub(super) fn new(
        form: NewLeaseForm,
        dex_account: Account,
        downpayment: DownpaymentCoin,
        loan: OpenLoanRespResult,
        deps: (LppRef, OracleRef, TimeAlarmsRef, FinalizerRef),
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
        Ok(QueryStateResponse::Opening {
            currency: self.form.currency,
            downpayment: self.downpayment,
            loan: self.loan.principal,
            loan_interest_rate: self.loan.annual_interest_rate,
            in_progress: in_progress_fn(HostAccount::from(self.dex_account).into()),
        })
    }
}

impl SwapTask for BuyAsset {
    type InG = LeasePaymentCurrencies;
    type OutG = AssetGroup;
    type InOutG = LeasePaymentCurrencies;
    type Label = Type;
    type StateResponse = ContractResult<QueryStateResponse>;
    type Result = SwapResult;

    fn label(&self) -> Self::Label {
        Type::OpeningSwap
    }

    fn dex_account(&self) -> &Account {
        &self.dex_account
    }

    fn oracle(&self) -> &impl SwapPath<Self::InOutG> {
        &self.deps.1
    }

    fn time_alarm(&self) -> &TimeAlarmsRef {
        &self.deps.2
    }

    fn out_currency(&self) -> CurrencyDTO<Self::OutG> {
        self.form.currency
    }

    fn on_coins<Visitor>(&self, visitor: &mut Visitor) -> Result<IterState, Visitor::Error>
    where
        Visitor: CoinVisitor<GIn = Self::InG, Result = IterNext>,
    {
        dex::on_coins(&self.downpayment, &self.loan.principal, visitor)
    }

    fn finish(
        self,
        amount_out: CoinDTO<Self::OutG>,
        env: &Env,
        querier: QuerierWrapper<'_>,
    ) -> Self::Result {
        debug_assert_eq!(amount_out.currency(), self.form.currency);
        debug_assert!(amount_out.amount() > 0);

        let position = PositionDTO::new(amount_out, self.form.position_spec.into());
        let profit = ProfitRef::new(self.form.loan.profit.clone(), &querier)?;
        let reserve = ReserveRef::try_new(self.form.reserve.clone(), &querier)?;
        let lease_addr = self.dex_account.owner().clone();
        let cmd = LeaseFactory::new(
            self.form,
            lease_addr.clone(),
            profit,
            reserve,
            (self.deps.2, self.deps.1.clone()),
            self.start_opening_at,
            &env.block.time,
        );
        let OpenLeaseResult { lease, status } =
            with_lease_deps::execute(cmd, lease_addr, position, self.deps.0, self.deps.1, querier)?;

        let lease = Lease::new(lease, self.dex_account, self.deps.3);
        let active = Active::new(lease);
        let emitter = active.emit_opened(env, self.downpayment, self.loan);

        match status {
            CloseStatusDTO::Paid => {
                unimplemented!("a freshly open lease should have some due amount")
            }
            CloseStatusDTO::None {
                current_liability: _, // TODO shouldn't we add warning zone events?
                alarms,
            } => Ok(StateMachineResponse::from(
                MessageResponse::messages_with_events(alarms, emitter),
                active,
            )),
            CloseStatusDTO::NeedLiquidation(liquidation) => {
                liquidation::start(active.into(), liquidation, emitter.into(), env, querier)
            }
            CloseStatusDTO::CloseAsked(_) => unimplemented!("no triggers have been set"),
        }
    }
}

impl ContractInSwap<TransferOutState> for BuyAsset {
    type StateResponse = <Self as SwapTask>::StateResponse;

    fn state(
        self,
        _now: Timestamp,
        _due_projection: Duration,
        _querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        let in_progress_fn = |ica_account| OngoingTrx::TransferOut { ica_account };
        self.state(in_progress_fn)
    }
}

impl ContractInSwap<SwapState> for BuyAsset {
    type StateResponse = <Self as SwapTask>::StateResponse;

    fn state(
        self,
        _now: Timestamp,
        _due_projection: Duration,
        _querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        let in_progress_fn = |ica_account| OngoingTrx::BuyAsset { ica_account };
        self.state(in_progress_fn)
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod test {
    use sdk::cosmwasm_std;

    use crate::contract::state::State;

    #[test]
    fn parse_old() {
        const STATE: &str = r#"{"BuyAsset":{"SwapExactInRespDelivery":{"handler":{"spec":{"form":{"customer":"nolus17rjgmry3w2xcc8yer4h4m8vuypkhkh8he3u8xv","currency":"LC1","max_ltd":250,"position_spec":{"liability":{"initial":600,"healthy":830,"first_liq_warn":850,"second_liq_warn":865,"third_liq_warn":880,"max":900,"recalc_time":7200000000000},"min_asset":{"amount":"15000000","ticker":"LPN"},"min_transaction":{"amount":"10000","ticker":"LPN"}},"loan":{"lpp":"nolus1qqcr7exupnymvg6m63eqwu8pd4n5x6r5t3pyyxdy7r97rcgajmhqy3gn94","profit":"nolus1udkxyfeh7kxjnzm0exfaq9hncqzm3rj59gut4qnll0gq2z4yff0sda5aw2","annual_margin_interest":40,"due_period":1209600000000000},"reserve":"nolus10hzky830fafe5ffzt6vqprmpxjsy0fk8gcq5wvnvgr6lt4s6he3s045c4n","time_alarms":"nolus1zwv6feuzhy6a9wekh96cd57lsarmqlwxdypdsplw6zhfncqw6ftqmx7chl","market_price_oracle":"nolus1jew4l5nq7m3xhkqzy8j7cc99083m5j8d9w004ayyv8xl3yv4h0dql2dd4e"},"dex_account":{"owner":"nolus1yhcph5r2x9rss6tluptttma736rknasjwn3659620ysu5fhmx2wq47gmch","host":"neutron1kdfwfa2pxf7jfth0pej3ds8v4fqa5nhc4nxdm6lr3ctqzvqjfg0shuxdcy","dex":{"connection_id":"connection-11","transfer_channel":{"local_endpoint":"channel-3839","remote_endpoint":"channel-44"}}},"downpayment":{"amount":"40000000","ticker":"LPN"},"loan":{"principal":{"amount":"10000000","ticker":"LPN"},"annual_interest_rate":87},"deps":[{"addr":"nolus1qqcr7exupnymvg6m63eqwu8pd4n5x6r5t3pyyxdy7r97rcgajmhqy3gn94"},{"addr":"nolus1jew4l5nq7m3xhkqzy8j7cc99083m5j8d9w004ayyv8xl3yv4h0dql2dd4e","base_currency":"LPN"},{"addr":"nolus1zwv6feuzhy6a9wekh96cd57lsarmqlwxdypdsplw6zhfncqw6ftqmx7chl"},{"addr":"nolus1et45v5gepxs44jxewfxah0hk4wqmw34m8pm4alf44ucxvj895kas5yrxd8"}],"start_opening_at":"1705072797559458289"}},"response":"Ek4KLC9jb3Ntd2FzbS53YXNtLnYxLk1zZ0V4ZWN1dGVDb250cmFjdFJlc3BvbnNlEh4KHHsicmV0dXJuX2Ftb3VudCI6IjI4ODgzNTQyIn0STQosL2Nvc213YXNtLndhc20udjEuTXNnRXhlY3V0ZUNvbnRyYWN0UmVzcG9uc2USHQobeyJyZXR1cm5fYW1vdW50IjoiNzIyMDgwOCJ9","_forward_to_inner_msg":null,"_delivery_adapter":null}}}"#;
        let state: State = cosmwasm_std::from_json(STATE).unwrap();
        assert!(matches!(state, State::BuyAsset(_)));
    }
}
