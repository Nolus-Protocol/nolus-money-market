use currency::{CurrencyDef, MemberOf};
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle_platform::Oracle as OracleTrait;
use platform::{
    bank::FixedAddressSender, batch::Emitter as PlatformEmitter,
    message::Response as MessageResponse,
};
use profit::stub::ProfitRef;
use sdk::cosmwasm_std::{Addr, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{
        position::{ChangeCmd, ClosePolicyChange},
        LeaseAssetCurrencies, LeasePaymentCurrencies,
    },
    contract::SplitDTOOut,
    error::{ContractError, ContractResult},
    finance::{LpnCoin, LpnCoinDTO, LpnCurrencies, LpnCurrency, OracleRef, ReserveRef},
    lease::{with_lease::WithLease, IntoDTOResult, Lease as LeaseDO, LeaseDTO},
    loan::RepayReceipt,
    position::CloseStrategy,
};

use super::{close_policy::check, CloseStatusDTO};

pub(crate) trait RepayFn {
    fn do_repay<Asset, Lpp, Oracle, Profit>(
        self,
        lease: &mut LeaseDO<Asset, Lpp, Oracle>,
        amount: LpnCoin,
        now: &Timestamp,
        profit: &mut Profit,
    ) -> ContractResult<RepayReceipt>
    where
        Asset: CurrencyDef,
        Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
        Lpp: LppLoanTrait<LpnCurrency, LpnCurrencies>,
        Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>
            + Into<OracleRef>,
        Profit: FixedAddressSender;
}

pub(crate) trait Emitter {
    fn emit(self, lease: &Addr, receipt: &RepayReceipt) -> PlatformEmitter;
}

pub(crate) struct Repay<'now, 'price_alarm, RepayableT, EmitterT>
where
    RepayableT: RepayFn,
    EmitterT: Emitter,
{
    repay_fn: RepayableT,
    amount: LpnCoinDTO,
    now: &'now Timestamp,
    emitter_fn: EmitterT,
    profit: ProfitRef,
    alarms: (TimeAlarmsRef, &'price_alarm OracleRef),
    reserve: ReserveRef,
}

impl<'now, 'price_alarm, RepayableT, EmitterT> Repay<'now, 'price_alarm, RepayableT, EmitterT>
where
    RepayableT: RepayFn,
    EmitterT: Emitter,
{
    pub fn new(
        repay_fn: RepayableT,
        amount: LpnCoinDTO,
        now: &'now Timestamp,
        emitter_fn: EmitterT,
        profit: ProfitRef,
        alarms: (TimeAlarmsRef, &'price_alarm OracleRef),
        reserve: ReserveRef,
    ) -> Self {
        Self {
            repay_fn,
            amount,
            now,
            emitter_fn,
            profit,
            alarms,
            reserve,
        }
    }

    fn check_close_with_init<'time_alarm, Asset, Lpp, Oracle>(
        lease: &mut LeaseDO<Asset, Lpp, Oracle>,
        receipt_close: bool,
        now: &'now Timestamp,
        time_alarm: &'time_alarm TimeAlarmsRef,
        price_alarm: &'price_alarm OracleRef,
    ) -> ContractResult<CloseStatusDTO>
    where
        Asset: CurrencyDef,
        Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
        Lpp: LppLoanTrait<LpnCurrency, LpnCurrencies>,
        Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>,
    {
        check::check(lease, now, time_alarm, price_alarm).and_then(|close_status| {
            debug_assert!(!(receipt_close ^ matches!(close_status, CloseStatusDTO::Paid))); // receipt.close() <=> status is CloseStatusDTO::Paid
            if matches!(
                close_status,
                CloseStatusDTO::CloseAsked(CloseStrategy::TakeProfit(_))
            ) {
                lease
                    .change_close_policy(
                        ClosePolicyChange {
                            take_profit: Some(ChangeCmd::Reset),
                            stop_loss: None,
                        },
                        now,
                    )
                    .and_then(|()| check::check(lease, now, time_alarm, price_alarm))
            } else {
                Ok(close_status)
            }
        })
    }
}

pub(crate) struct RepayLeaseResult {
    lease: LeaseDTO,
    result: RepayResult,
}

impl SplitDTOOut for RepayLeaseResult {
    type Other = RepayResult;

    fn split_into(self) -> (LeaseDTO, Self::Other) {
        (self.lease, self.result)
    }
}

pub(crate) struct RepayResult {
    pub response: MessageResponse,
    pub close_status: CloseStatusDTO,
}

impl<RepayableT, EmitterT> WithLease for Repay<'_, '_, RepayableT, EmitterT>
where
    RepayableT: RepayFn,
    EmitterT: Emitter,
{
    type Output = RepayLeaseResult;

    type Error = ContractError;

    fn exec<Asset, Lpp, Oracle>(
        self,
        mut lease: LeaseDO<Asset, Lpp, Oracle>,
    ) -> Result<Self::Output, Self::Error>
    where
        Asset: CurrencyDef,
        Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
        Lpp: LppLoanTrait<LpnCurrency, LpnCurrencies>,
        Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>
            + Into<OracleRef>,
    {
        let amount = self.amount.try_into()?;
        let mut profit_sender = self.profit.clone().into_stub();

        let receipt = self
            .repay_fn
            .do_repay(&mut lease, amount, self.now, &mut profit_sender)?;

        let events = self.emitter_fn.emit(lease.addr(), &receipt);

        // not a method since self has been partially moved
        Self::check_close_with_init(
            &mut lease,
            receipt.close(),
            self.now,
            &self.alarms.0,
            self.alarms.1,
        )
        .and_then(|close_status| {
            lease
                .try_into_dto(self.profit, self.alarms.0, self.reserve)
                .map(
                    |IntoDTOResult {
                         lease,
                         batch: messages,
                     }| {
                        RepayLeaseResult {
                            lease,
                            result: RepayResult {
                                response: MessageResponse::messages_with_events(
                                    messages.merge(profit_sender.into()),
                                    events,
                                ),
                                close_status,
                            },
                        }
                    },
                )
        })
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod test {
    use finance::{
        coin::Coin,
        fraction::Fraction,
        liability::Zone,
        percent::Percent,
        price::{self, Price},
    };
    use lpp::msg::LoanResponse;
    use platform::batch::Emitter as PlatformEmitter;
    use profit::stub::ProfitRef;
    use sdk::cosmwasm_std::{Addr, Timestamp};
    use timealarms::stub::TimeAlarmsRef;

    use crate::{
        api::position::{ChangeCmd, ClosePolicyChange},
        contract::cmd::RepayLeaseFn,
        lease::{
            tests::{self, TestLease, TestLpn, FIRST_LIQ_WARN},
            with_lease::WithLease,
        },
    };

    use super::{
        CloseStatusDTO, Emitter, OracleRef, Repay, RepayReceipt, RepayResult, ReserveRef,
        SplitDTOOut,
    };

    struct DummyEmitter {}
    impl Emitter for DummyEmitter {
        fn emit(self, _lease: &Addr, _receipt: &RepayReceipt) -> PlatformEmitter {
            PlatformEmitter::of_type("test")
        }
    }
    #[test]
    fn reset_take_profit() {
        let now = Timestamp::from_seconds(24412515);
        let lease_amount = 1000.into();
        let lease_lpn = price::total(lease_amount, Price::identity());
        let current_ltv = Percent::from_percent(30);
        let stop_loss = Percent::from_percent(59);
        let take_profit = Percent::from_percent(29);

        let due_amount = current_ltv.of(lease_lpn);
        let loan = LoanResponse {
            principal_due: due_amount,
            annual_interest_rate: Percent::from_permille(50),
            interest_paid: now,
        };
        let mut lease = tests::open_lease(lease_amount, loan.clone());
        lease
            .change_close_policy(
                ClosePolicyChange {
                    stop_loss: Some(ChangeCmd::Set(stop_loss)),
                    take_profit: Some(ChangeCmd::Set(take_profit)),
                },
                &now,
            )
            .expect("change close policy succeed");

        let payment: Coin<TestLpn> = due_amount - take_profit.of(lease_lpn);
        let close_status = repay(lease, payment, &now);
        match close_status {
            CloseStatusDTO::None {
                current_liability,
                alarms: _,
            } => assert_eq!(Zone::no_warnings(FIRST_LIQ_WARN), current_liability),
            _ => panic!("unexpected close status!"),
        }
    }

    fn repay(lease: TestLease, payment: Coin<TestLpn>, now: &Timestamp) -> CloseStatusDTO {
        let oracle = OracleRef::unchecked(Addr::unchecked("price_alarms_addr"));

        let cmd = Repay::new(
            RepayLeaseFn {},
            payment.into(),
            now,
            DummyEmitter {},
            ProfitRef::unchecked("profit_addr"),
            (TimeAlarmsRef::unchecked("time_alarms_addr"), &oracle),
            ReserveRef::unchecked(Addr::unchecked("reserve_addr")),
        );
        let (
            _dto,
            RepayResult {
                response: _,
                close_status,
            },
        ) = cmd.exec(lease).expect("payment succeed").split_into();
        close_status
    }
}
