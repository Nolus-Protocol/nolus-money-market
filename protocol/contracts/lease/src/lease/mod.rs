use currency::{Currency, CurrencyDef, MemberOf};
use finance::duration::Duration;
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle_platform::Oracle as OracleTrait;
use platform::batch::Batch;
use profit::stub::ProfitRef;
use sdk::cosmwasm_std::{Addr, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{LeaseAssetCurrencies, LeasePaymentCurrencies},
    error::{ContractError, ContractResult},
    finance::{LpnCurrencies, LpnCurrency, OracleRef, ReserveRef},
    loan::Loan,
    position::Position,
};

pub(super) use self::{
    close_policy::CloseStatus, dto::LeaseDTO, paid::Lease as LeasePaid, state::State,
};

mod close;
mod close_policy;
mod dto;
mod due;
mod paid;
mod repay;
mod state;
pub(crate) mod with_lease;
pub(crate) mod with_lease_deps;
pub(crate) mod with_lease_paid;

// TODO look into reducing the type parameters to Lpp and Asset only!
// the others could be provided on demand when certain operation is being performed
// then review the methods that take `&mut self` whether could be transformed into `&self`
// and those that take `self` into `&mut self` or `&self`
pub struct Lease<Asset, Lpp, Oracle> {
    addr: Addr,
    customer: Addr,
    position: Position<Asset>,
    loan: Loan<Lpp>,
    oracle: Oracle,
}

#[cfg_attr(feature = "contract_testing", derive(Debug))]
pub struct IntoDTOResult {
    pub lease: LeaseDTO,
    pub batch: Batch,
}

impl<Asset, LppLoan, Oracle> Lease<Asset, LppLoan, Oracle> {
    pub(crate) fn addr(&self) -> &Addr {
        &self.addr
    }
}

impl<Asset, LppLoan, Oracle> Lease<Asset, LppLoan, Oracle>
where
    Asset: Currency + MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
    LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>,
{
    pub(super) fn new(
        addr: Addr,
        customer: Addr,
        position: Position<Asset>,
        loan: Loan<LppLoan>,
        oracle: Oracle,
    ) -> Self {
        debug_assert!(!currency::equal::<LpnCurrency, Asset>());
        // TODO specify that Lpn is of Lpns and Asset is of LeaseGroup

        Self {
            addr,
            customer,
            position,
            loan,
            oracle,
        }
    }

    pub(super) fn from_dto(
        dto: LeaseDTO,
        position: Position<Asset>,
        lpp_loan: LppLoan,
        oracle: Oracle,
    ) -> Self {
        Self::new(
            dto.addr,
            dto.customer,
            position,
            Loan::from_dto(dto.loan, lpp_loan),
            oracle,
        )
    }

    pub(crate) fn state(&self, now: Timestamp, due_projection: Duration) -> State<Asset> {
        let estimate_at = now + due_projection;
        let loan = self.loan.state(&estimate_at);
        let overdue_collect_in = self.position.overdue_collection_in(&loan);

        State {
            amount: self.position.amount(),
            interest_rate: loan.annual_interest,
            interest_rate_margin: loan.annual_interest_margin,
            principal_due: loan.principal_due,
            overdue_margin: loan.overdue.margin(),
            overdue_interest: loan.overdue.interest(),
            overdue_collect_in,
            due_margin: loan.due_margin_interest,
            due_interest: loan.due_interest,
            due_projection,
            close_policy: self.position.close_policy(),
            validity: now,
        }
    }
}

impl<Asset, LppLoan, Oracle> Lease<Asset, LppLoan, Oracle>
where
    Asset: CurrencyDef,
    Asset::Group: MemberOf<LeaseAssetCurrencies>,
    LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    LppLoan::Error: Into<ContractError>,
    Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>
        + Into<OracleRef>,
{
    // TODO add fn into_dto() without profit and use it when no payment has been done
    // TODO rename this fn to try_into_dto_past_payments
    pub(super) fn try_into_dto(
        self,
        profit: ProfitRef,
        time_alarms: TimeAlarmsRef,
        reserve: ReserveRef,
    ) -> ContractResult<IntoDTOResult> {
        let (loan_dto, loan_batch) = self.loan.try_into_dto(profit)?;

        Ok(IntoDTOResult {
            lease: LeaseDTO::new(
                self.addr,
                self.customer,
                self.position.into(),
                loan_dto,
                time_alarms,
                self.oracle.into(),
                reserve,
            ),
            batch: loan_batch,
        })
    }

    pub(super) fn try_into_messages(self) -> ContractResult<Batch> {
        self.loan.try_into_messages()
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
pub mod tests {
    use std::ops::Add;

    use serde::{Deserialize, Serialize};

    pub(super) use currencies::PaymentGroup as PriceG;
    use currencies::{testing::PaymentC7, Lpn};
    use currency::{Currency, Group, MemberOf};
    use finance::{
        coin::Coin, duration::Duration, fraction::Fraction, liability::Liability, percent::Percent,
        price::Price,
    };
    use lpp::{
        error::{Error as LppError, Result as LppResult},
        loan::RepayShares,
        msg::LoanResponse,
        stub::{loan::LppLoan, LppBatch, LppRef},
    };
    use oracle_platform::{error::Result as PriceOracleResult, Oracle};
    use platform::batch::Batch;
    use sdk::cosmwasm_std::{Addr, Timestamp};

    use crate::{
        api::{
            position::{ChangeCmd, ClosePolicyChange},
            query::opened::ClosePolicy,
        },
        finance::{LpnCurrencies, OracleRef},
        loan::Loan,
        position::{Position, Spec as PositionSpec},
    };

    use super::{Lease, State};

    const CUSTOMER: &str = "customer";
    const LEASE_ADDR: &str = "lease_addr";
    const ORACLE_ADDR: &str = "oracle_addr";
    const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(23);
    pub(super) const LEASE_START: Timestamp = Timestamp::from_nanos(100);
    pub(super) const DUE_PERIOD: Duration = Duration::from_days(100);
    pub(crate) const FIRST_LIQ_WARN: Percent = Percent::from_permille(730);
    pub(super) const SECOND_LIQ_WARN: Percent = Percent::from_permille(750);
    pub(super) const THIRD_LIQ_WARN: Percent = Percent::from_permille(780);
    pub(super) const RECHECK_TIME: Duration = Duration::from_hours(24);
    pub(super) const MIN_TRANSACTION: Coin<TestLpn> = Coin::new(10_000);
    pub(crate) type TestLpn = Lpn;
    pub(crate) type TestCurrency = PaymentC7;
    pub(crate) type TestLease = Lease<TestCurrency, LppLoanLocal<TestLpn>, OracleLocalStub>;

    // TODO migrate to using lpp::stub::unchecked_lpp_loan
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LppLoanLocal<Lpn> {
        loan: LoanResponse<Lpn>,
    }

    impl<Lpn> From<LoanResponse<Lpn>> for LppLoanLocal<Lpn> {
        fn from(loan: LoanResponse<Lpn>) -> Self {
            Self { loan }
        }
    }

    impl<Lpn> LppLoan<Lpn, LpnCurrencies> for LppLoanLocal<Lpn> {
        fn principal_due(&self) -> Coin<Lpn> {
            self.loan.principal_due
        }

        fn interest_due(&self, by: &Timestamp) -> Coin<Lpn> {
            self.loan.interest_due(by)
        }

        fn repay(&mut self, by: &Timestamp, repayment: Coin<Lpn>) -> RepayShares<Lpn> {
            self.loan.repay(by, repayment)
        }

        fn annual_interest_rate(&self) -> Percent {
            self.loan.annual_interest_rate
        }
    }

    impl<Lpn> TryFrom<LppLoanLocal<Lpn>> for LppBatch<LppRef<Lpn, LpnCurrencies>> {
        type Error = LppError;

        fn try_from(_: LppLoanLocal<Lpn>) -> LppResult<Self> {
            Ok(Self {
                lpp_ref: LppRef::<Lpn, _>::unchecked(Addr::unchecked("test_lpp")),
                batch: Batch::default(),
            })
        }
    }

    pub struct OracleLocalStub {
        ref_: OracleRef,
    }

    impl From<Addr> for OracleLocalStub {
        fn from(oracle: Addr) -> Self {
            Self {
                ref_: OracleRef::unchecked(oracle),
            }
        }
    }

    impl<OracleG> Oracle<OracleG> for OracleLocalStub
    where
        OracleG: Group + MemberOf<PriceG>,
        Self: Into<OracleRef>,
    {
        type QuoteC = TestLpn;
        type QuoteG = LpnCurrencies;

        fn price_of<C>(&self) -> PriceOracleResult<Price<C, TestLpn>>
        where
            C: Currency,
        {
            Ok(Price::identity())
        }
    }

    impl AsRef<OracleRef> for OracleLocalStub {
        fn as_ref(&self) -> &OracleRef {
            &self.ref_
        }
    }

    impl From<OracleLocalStub> for OracleRef {
        fn from(stub: OracleLocalStub) -> Self {
            stub.ref_
        }
    }

    pub fn open_lease(amount: Coin<TestCurrency>, loan: LoanResponse<TestLpn>) -> TestLease {
        open_lease_with_payment_spec(amount, loan, DUE_PERIOD)
    }

    pub fn open_lease_with_payment_spec(
        amount: Coin<TestCurrency>,
        loan: LoanResponse<TestLpn>,
        due_period: Duration,
    ) -> TestLease {
        let lease = Addr::unchecked(LEASE_ADDR);
        let oracle: OracleLocalStub = Addr::unchecked(ORACLE_ADDR).into();

        let loan = loan.into();
        let loan = Loan::new(loan, LEASE_START, MARGIN_INTEREST_RATE, due_period);
        let liability = Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(70),
            FIRST_LIQ_WARN,
            SECOND_LIQ_WARN,
            THIRD_LIQ_WARN,
            Percent::from_percent(80),
            RECHECK_TIME,
        );
        let position_spec =
            PositionSpec::no_close(liability, Coin::<TestLpn>::new(15_000_000), MIN_TRANSACTION);
        Lease::new(
            lease,
            Addr::unchecked(CUSTOMER),
            Position::<TestCurrency>::new(amount, position_spec),
            loan,
            oracle,
        )
    }

    pub fn coin(a: u128) -> Coin<TestCurrency> {
        Coin::new(a)
    }

    pub fn lpn_coin(a: u128) -> Coin<TestLpn> {
        Coin::new(a)
    }

    #[test]
    fn state_opened() {
        let lease_amount = coin(1000);
        let interest_rate = Percent::from_permille(50);
        let overdue_collect_in = Duration::from_days(500); //=min_transaction/principal_due/(interest+margin)*1000*365

        let principal_due = lpn_coin(100_000);
        let loan = LoanResponse {
            principal_due,
            annual_interest_rate: interest_rate,
            interest_paid: LEASE_START,
        };
        let mut lease = open_lease(lease_amount, loan.clone());

        let state_since_open = Duration::from_nanos(150);
        let state_at = LEASE_START.add(state_since_open);
        let take_profit = Percent::from_percent(20);
        lease
            .change_close_policy(
                ClosePolicyChange {
                    stop_loss: None,
                    take_profit: Some(ChangeCmd::Set(take_profit)),
                },
                &state_at,
            )
            .unwrap();

        {
            let due_projection = Duration::default();
            assert_eq!(
                State {
                    amount: lease_amount,
                    interest_rate,
                    interest_rate_margin: MARGIN_INTEREST_RATE,
                    principal_due: loan.principal_due,
                    overdue_margin: lpn_coin(0),
                    overdue_interest: lpn_coin(0),
                    overdue_collect_in,
                    due_margin: lpn_coin(0),
                    due_interest: lpn_coin(0),
                    due_projection,
                    close_policy: ClosePolicy::new(Some(take_profit), None),
                    validity: state_at,
                },
                lease.state(state_at, due_projection)
            );
        }

        compare_now_vs_projected(&lease, state_at);

        assert_state(
            principal_due,
            interest_rate,
            lease_amount,
            take_profit,
            state_at,
            &lease,
            Duration::default(),
        );

        assert_state(
            principal_due,
            interest_rate,
            lease_amount,
            take_profit,
            state_at,
            &lease,
            Duration::from_days(12),
        );
    }

    fn assert_state(
        principal_due: Coin<TestLpn>,
        interest_rate: Percent,
        lease_amount: Coin<TestCurrency>,
        take_profit: Percent,
        state_at: Timestamp,
        lease: &TestLease,
        due_projection: Duration,
    ) {
        let exp_due_margin =
            due_projection.annualized_slice_of(MARGIN_INTEREST_RATE.of(principal_due));
        let exp_due_interest = due_projection.annualized_slice_of(interest_rate.of(principal_due));
        assert_eq!(
            State {
                amount: lease_amount,
                interest_rate,
                interest_rate_margin: MARGIN_INTEREST_RATE,
                principal_due,
                overdue_margin: lpn_coin(0),
                overdue_interest: lpn_coin(0),
                overdue_collect_in: Duration::YEAR.into_slice_per_ratio(
                    MIN_TRANSACTION - exp_due_interest - exp_due_margin,
                    (interest_rate + MARGIN_INTEREST_RATE).of(principal_due)
                ),
                due_margin: exp_due_margin,
                due_interest: exp_due_interest,
                due_projection,
                close_policy: ClosePolicy::new(Some(take_profit), None),
                validity: state_at,
            },
            lease.state(state_at, due_projection)
        );
    }

    fn compare_now_vs_projected(lease: &TestLease, state_at: Timestamp) {
        let due_projection = Duration::from_days(12);
        let state_now = lease.state(state_at + due_projection, Duration::default());
        let state_projected = lease.state(state_at, due_projection);
        assert_eq!(state_now.amount, state_projected.amount);
        assert_eq!(state_now.interest_rate, state_projected.interest_rate);
        assert_eq!(
            state_now.interest_rate_margin,
            state_projected.interest_rate_margin
        );
        assert_eq!(state_now.overdue_margin, state_projected.overdue_margin);
        assert_eq!(state_now.overdue_interest, state_projected.overdue_interest);
        assert_eq!(
            state_now.overdue_collect_in,
            state_projected.overdue_collect_in
        );
        assert_eq!(state_now.due_margin, state_projected.due_margin);
        assert_eq!(state_now.due_interest, state_projected.due_interest);
        assert_eq!(
            state_now.validity + state_now.due_projection,
            state_projected.validity + state_projected.due_projection
        );
        assert_eq!(
            state_now.validity,
            state_projected.validity + state_projected.due_projection
        );
        assert_eq!(state_now.close_policy, state_projected.close_policy);
    }
}
