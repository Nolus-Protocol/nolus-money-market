use std::{fmt::Debug, marker::PhantomData};

use serde::{Deserialize, Serialize};

use finance::{
    coin::Coin,
    currency::Currency,
    duration::Duration,
    interest::InterestPeriod,
    percent::{Percent, Units},
};
use lpp::{
    loan::LoanData,
    msg::QueryLoanResponse,
    stub::{
        lender::{LppLender as LppLenderTrait, LppLenderRef},
        LppBatch,
    },
};
use platform::batch::Batch;
use profit::stub::{Profit as ProfitTrait, ProfitBatch, ProfitRef};
use sdk::cosmwasm_std::{Addr, Timestamp};

use crate::{
    api::InterestPaymentSpec,
    error::{ContractError, ContractResult},
};

pub use self::state::State;
pub(crate) use self::{liability::LiabilityStatus, repay::Receipt as RepayReceipt};

mod liability;
mod repay;
mod state;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LoanDTO {
    annual_margin_interest: Percent,
    lpp: LppLenderRef,
    interest_payment_spec: InterestPaymentSpec,
    current_period: InterestPeriod<Units, Percent>,
    profit: ProfitRef,
}

impl LoanDTO {
    fn new(
        annual_margin_interest: Percent,
        lpp: LppLenderRef,
        interest_payment_spec: InterestPaymentSpec,
        current_period: InterestPeriod<Units, Percent>,
        profit: ProfitRef,
    ) -> Self {
        Self {
            annual_margin_interest,
            lpp,
            interest_payment_spec,
            current_period,
            profit,
        }
    }

    pub(crate) fn annual_margin_interest(&self) -> Percent {
        self.annual_margin_interest
    }

    pub(crate) fn lpp(&self) -> &LppLenderRef {
        &self.lpp
    }

    pub(crate) fn profit(&self) -> &ProfitRef {
        &self.profit
    }
}

pub struct Loan<Lpn, Lpp, Profit> {
    annual_margin_interest: Percent,
    lpn: PhantomData<Lpn>,
    lpp: Lpp,
    interest_payment_spec: InterestPaymentSpec,
    current_period: InterestPeriod<Units, Percent>,
    profit: Profit,
}

impl<Lpn, Lpp, Profit> Loan<Lpn, Lpp, Profit>
where
    Lpn: Currency + Debug,
    Lpp: LppLenderTrait<Lpn>,
    Profit: ProfitTrait,
{
    pub(super) fn new(
        start: Timestamp,
        lpp: Lpp,
        annual_margin_interest: Percent,
        interest_payment_spec: InterestPaymentSpec,
        profit: Profit,
    ) -> Self {
        let current_period = Self::due_period(
            annual_margin_interest,
            start,
            interest_payment_spec.due_period(),
        );
        Self {
            annual_margin_interest,
            lpn: PhantomData,
            lpp,
            interest_payment_spec,
            current_period,
            profit,
        }
    }

    pub(super) fn from_dto(dto: LoanDTO, lpp: Lpp, profit: Profit) -> Self {
        {
            let annual_margin_interest = dto.annual_margin_interest;
            let interest_payment_spec = dto.interest_payment_spec;
            let current_period = dto.current_period;
            Self {
                annual_margin_interest,
                lpn: PhantomData,
                lpp,
                interest_payment_spec,
                current_period,
                profit,
            }
        }
    }

    pub(super) fn into_dto(self) -> (LoanDTO, Batch) {
        let LppBatch {
            lpp_ref,
            batch: lpp_batch,
        } = self.lpp.into();

        let ProfitBatch {
            profit_ref,
            batch: profit_batch,
        } = self.profit.into();

        let dto = LoanDTO::new(
            self.annual_margin_interest,
            lpp_ref,
            self.interest_payment_spec,
            self.current_period,
            profit_ref,
        );

        (dto, lpp_batch.merge(profit_batch))
    }

    pub(crate) fn grace_period_end(&self) -> Timestamp {
        self.current_period.till() + self.interest_payment_spec.grace_period()
    }

    pub(crate) fn repay<A>(
        &mut self,
        payment: Coin<Lpn>,
        by: Timestamp,
        lease: A,
    ) -> ContractResult<RepayReceipt<Lpn>>
    where
        A: Into<Addr>,
    {
        self.debug_check_start_due_before(by, "before the 'repay-by' time");
        self.debug_check_before_next_due_end(by);

        let mut receipt = RepayReceipt::default();

        let loan = self.load_loan(lease)?.ok_or(ContractError::LoanClosed())?;

        let (change, loan_prev_period_payment) = if self.overdue_at(by) {
            self.repay_previous_period(payment, by, &loan, &mut receipt)?
        } else {
            (payment, Coin::default())
        };
        debug_assert_eq!(payment, change + receipt.total());
        debug_assert_eq!(loan_prev_period_payment, receipt.previous_interest_paid());
        debug_assert!(!self.overdue_at(by) || change == Coin::default());

        let (change, loan_curr_period_payment) = if !self.overdue_at(by) {
            self.repay_current_period(change, by, &loan, &mut receipt)?
        } else {
            (change, Coin::default())
        };
        debug_assert_eq!(payment, change + receipt.total());
        debug_assert_eq!(
            loan_curr_period_payment,
            receipt.current_interest_paid() + receipt.principal_paid(),
        );

        receipt.keep_change(change);
        debug_assert_eq!(payment, receipt.total());

        let loan_payment = loan_prev_period_payment + loan_curr_period_payment;
        if !loan_payment.is_zero() {
            // In theory, zero loan payment may occur if two consecutive repayments are executed within the same time.
            // In practice, that means two repayment transactions of the same lease enter the same block.
            self.lpp.repay_loan_req(loan_payment)?;
        }
        Ok(receipt)
    }

    pub(crate) fn state(&self, now: Timestamp, lease: Addr) -> ContractResult<Option<State<Lpn>>> {
        self.debug_check_start_due_before(now, "in the past of");

        let loan = if let Some(loan) = self.load_loan(lease)? {
            loan
        } else {
            return Ok(None);
        };

        let principal_due = loan.principal_due;

        let margin_interest_overdue_period = if self.overdue_at(now) {
            self.current_period
        } else {
            self.due_period_from_with_length(
                self.current_period.till() - self.interest_payment_spec.due_period(),
                Duration::default(),
            )
        };

        let margin_interest_due_period = self
            .current_period
            .from(margin_interest_overdue_period.till())
            .spanning(Duration::between(
                margin_interest_overdue_period.till(),
                now,
            ));

        debug_assert_eq!(margin_interest_due_period.till(), now);

        let previous_margin_interest_due = margin_interest_overdue_period.interest(principal_due);
        let current_margin_interest_due = margin_interest_due_period.interest(principal_due);

        let previous_interest_due = loan.interest_due(margin_interest_overdue_period.till());
        let current_interest_due = loan.interest_due(now) - previous_interest_due;

        Ok(Some(State {
            annual_interest: loan.annual_interest_rate,
            annual_interest_margin: self.annual_margin_interest,
            principal_due,
            previous_interest_due,
            current_interest_due,
            previous_margin_interest_due,
            current_margin_interest_due,
        }))
    }

    fn load_loan(&self, lease: impl Into<Addr>) -> ContractResult<QueryLoanResponse<Lpn>> {
        self.lpp.loan(lease).map_err(ContractError::from)
    }

    fn repay_previous_period(
        &mut self,
        payment: Coin<Lpn>,
        by: Timestamp,
        loan: &LoanData<Lpn>,
        receipt: &mut RepayReceipt<Lpn>,
    ) -> ContractResult<(Coin<Lpn>, Coin<Lpn>)> {
        let (prev_margin_paid, change) =
            self.repay_margin_interest(loan.principal_due, by, payment)?;
        receipt.pay_previous_margin(prev_margin_paid);

        if change.is_zero() {
            return Ok((Coin::default(), Coin::default()));
        }

        debug_assert!(self.current_period.zero_length()); // no prev_margin due

        let previous_interest_due = loan.interest_due(self.current_period.till());
        let previous_interest_paid = previous_interest_due.min(change);
        receipt.pay_previous_interest(previous_interest_paid);

        if previous_interest_paid == previous_interest_due {
            self.open_next_period();
        }

        Ok((change - previous_interest_paid, previous_interest_paid))
    }

    fn repay_current_period(
        &mut self,
        payment: Coin<Lpn>,
        by: Timestamp,
        loan: &LoanData<Lpn>,
        receipt: &mut RepayReceipt<Lpn>,
    ) -> ContractResult<(Coin<Lpn>, Coin<Lpn>)> {
        let mut loan_repay = Coin::default();

        let (curr_margin_paid, mut change) =
            self.repay_margin_interest(loan.principal_due, by, payment)?;

        receipt.pay_current_margin(curr_margin_paid);

        {
            let curr_interest_paid =
                change.min(loan.interest_due(by) - receipt.previous_interest_paid());

            change -= curr_interest_paid;

            loan_repay += curr_interest_paid;

            receipt.pay_current_interest(curr_interest_paid);
        }

        {
            let principal_paid = change.min(loan.principal_due);

            change -= principal_paid;

            loan_repay += principal_paid;

            receipt.pay_principal(loan.principal_due, principal_paid);
        }

        Ok((change, loan_repay))
    }

    fn repay_margin_interest(
        &mut self,
        principal_due: Coin<Lpn>,
        by: Timestamp,
        payment: Coin<Lpn>,
    ) -> ContractResult<(Coin<Lpn>, Coin<Lpn>)> {
        let (period, change) = self.current_period.pay(principal_due, payment, by);
        self.current_period = period;

        let paid = payment - change;

        if !paid.is_zero() {
            self.profit.send(paid);
        }

        Ok((paid, change))
    }

    fn open_next_period(&mut self) {
        debug_assert!(self.current_period.zero_length());

        self.current_period = self.due_period_from(self.current_period.till());
    }

    fn overdue_at(&self, when: Timestamp) -> bool {
        self.current_period.till() <= when
    }

    fn due_period_from(&self, start: Timestamp) -> InterestPeriod<Units, Percent> {
        Self::due_period(
            self.annual_margin_interest,
            start,
            self.interest_payment_spec.due_period(),
        )
    }

    fn due_period_from_with_length(
        &self,
        start: Timestamp,
        period: Duration,
    ) -> InterestPeriod<Units, Percent> {
        Self::due_period(self.annual_margin_interest, start, period)
    }

    fn due_period(
        margin_interest: Percent,
        start: Timestamp,
        period: Duration,
    ) -> InterestPeriod<Units, Percent> {
        InterestPeriod::with_interest(margin_interest)
            .from(start)
            .spanning(period)
    }

    fn debug_check_start_due_before(&self, when: Timestamp, when_descr: &str) {
        debug_assert!(
            self.current_period.start() <= when,
            "The current due period starting at {}s, should begin {} {}s",
            self.current_period.start(),
            when_descr,
            when
        );
    }
    fn debug_check_before_next_due_end(&self, when: Timestamp) {
        let next_due_end = self.current_period.till() + self.interest_payment_spec.due_period();
        debug_assert!(
            when < next_due_end,
            "Payment is tried at {}s which is not before the next period ending at {}s",
            when,
            next_due_end,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use serde::{Deserialize, Serialize};

    use finance::{
        coin::{Amount, Coin, WithCoin},
        currency::{Currency, Group},
        duration::Duration,
        error::Error as FinanceError,
        fraction::Fraction,
        interest::InterestPeriod,
        percent::Percent,
        test::currency::Usdc,
    };
    use lpp::{
        error::ContractError as LppError,
        loan::LoanData,
        msg::{LoanResponse, QueryLoanResponse, QueryQuoteResponse},
        stub::{
            lender::{LppLender, LppLenderRef},
            LppBatch,
        },
    };
    use platform::{
        bank::{Aggregate, BankAccountView},
        error::{Error as PlatformError, Result as PlatformResult},
    };
    use profit::stub::{Profit, ProfitBatch};
    use sdk::cosmwasm_std::{Addr, Timestamp};

    use crate::{
        api::InterestPaymentSpec,
        loan::{repay::Receipt as RepayReceipt, Loan},
    };

    // 50%
    const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(500);
    const LEASE_START: Timestamp = Timestamp::from_nanos(100);

    type TestCurrency = Usdc;
    type LppResult<T> = Result<T, LppError>;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct BankStub {
        balance: Amount,
    }

    impl BankAccountView for BankStub {
        fn balance<C>(&self) -> PlatformResult<Coin<C>>
        where
            C: Currency,
        {
            Ok(Coin::<C>::new(self.balance))
        }

        fn balances<G, Cmd>(&self, _: Cmd) -> Result<Option<Cmd::Output>, Cmd::Error>
        where
            G: Group,
            Cmd: WithCoin,
            Cmd::Output: Aggregate,
            Cmd::Error: StdError,
            PlatformError: Into<Cmd::Error>,
            FinanceError: Into<Cmd::Error>,
        {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct LppLenderLocalStub {
        loan: LoanResponse<TestCurrency>,
        now: Timestamp,
    }

    impl LppLenderLocalStub {
        fn new(loan: LoanData<TestCurrency>, now: Timestamp) -> Self {
            Self { loan, now }
        }
    }

    impl LppLender<TestCurrency> for LppLenderLocalStub {
        fn open_loan_req(&mut self, _amount: Coin<TestCurrency>) -> LppResult<()> {
            unreachable!()
        }

        fn open_loan_resp(
            &self,
            _resp: cosmwasm_std::Reply,
        ) -> LppResult<LoanResponse<TestCurrency>> {
            unreachable!()
        }

        fn repay_loan_req(&mut self, repayment: Coin<TestCurrency>) -> LppResult<()> {
            let (due_period, change) =
                InterestPeriod::with_interest(self.loan.annual_interest_rate)
                    .from(self.loan.interest_paid)
                    .spanning(Duration::between(self.loan.interest_paid, self.now))
                    .pay(self.loan.principal_due, repayment, self.now);
            self.loan.interest_paid = due_period.start();
            assert!(
                change <= self.loan.principal_due,
                "{change} <= {principal_due}",
                change = change,
                principal_due = self.loan.principal_due
            );
            self.loan.principal_due -= change;
            Ok(())
        }

        fn loan(&self, _lease: impl Into<Addr>) -> LppResult<QueryLoanResponse<TestCurrency>> {
            Ok(Some(self.loan.clone()))
        }

        fn quote(&self, _amount: Coin<TestCurrency>) -> LppResult<QueryQuoteResponse> {
            unreachable!()
        }
    }

    impl From<LppLenderLocalStub> for LppBatch<LppLenderRef> {
        fn from(_: LppLenderLocalStub) -> Self {
            unreachable!()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct ProfitLocalStub {}

    impl Profit for ProfitLocalStub {
        fn send<C>(&mut self, _coins: Coin<C>)
        where
            C: Currency,
        {
        }
    }

    impl From<ProfitLocalStub> for ProfitBatch {
        fn from(_: ProfitLocalStub) -> Self {
            unreachable!()
        }
    }

    fn create_loan(
        loan: LoanResponse<TestCurrency>,
        now: Timestamp,
    ) -> Loan<TestCurrency, LppLenderLocalStub, ProfitLocalStub> {
        Loan::new(
            LEASE_START,
            LppLenderLocalStub::new(loan, now),
            MARGIN_INTEREST_RATE,
            InterestPaymentSpec::new(Duration::YEAR, Duration::from_secs(0)),
            ProfitLocalStub {},
        )
    }

    #[test]
    fn two_periods_span_repay() {
        let lease = Addr::unchecked("my lease");

        let lease_coin = coin(1000);
        let interest_rate = Percent::from_percent(80);
        let delta_to_fully_due = coin(50);
        let payment_at = LEASE_START + Duration::YEAR + Duration::YEAR;

        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: LEASE_START,
        };

        let mut loan = create_loan(loan, payment_at);

        {
            let repay_prev_margin = MARGIN_INTEREST_RATE.of(lease_coin) - delta_to_fully_due;
            let mut receipt = RepayReceipt::default();
            receipt.pay_previous_margin(repay_prev_margin);

            assert_eq!(
                loan.repay(
                    repay_prev_margin,
                    payment_at - Duration::from_nanos(1),
                    lease.clone(),
                ),
                Ok(receipt)
            );
        }
        {
            let repay_fully_prev_margin_and_some_interest = interest_rate.of(lease_coin);
            let mut receipt = RepayReceipt::default();
            receipt.pay_previous_margin(delta_to_fully_due);
            receipt.pay_previous_interest(
                repay_fully_prev_margin_and_some_interest - delta_to_fully_due,
            );
            assert_eq!(
                loan.repay(
                    repay_fully_prev_margin_and_some_interest,
                    payment_at - Duration::from_nanos(1),
                    lease.clone(),
                ),
                Ok(receipt)
            );
        }
        {
            let repay_fully_prev_amount_and_some_curr_margin = MARGIN_INTEREST_RATE.of(lease_coin);
            let mut receipt = RepayReceipt::default();
            receipt.pay_previous_interest(delta_to_fully_due);
            receipt.pay_current_margin(
                repay_fully_prev_amount_and_some_curr_margin - delta_to_fully_due,
            );
            assert_eq!(
                loan.repay(
                    repay_fully_prev_amount_and_some_curr_margin,
                    payment_at - Duration::from_nanos(1),
                    lease.clone(),
                ),
                Ok(receipt)
            );
        }
        {
            let margin_due = delta_to_fully_due - coin(1); // rounding
            let surplus = delta_to_fully_due;
            let repay_fully = margin_due + interest_rate.of(lease_coin) + lease_coin + surplus;
            let mut receipt = RepayReceipt::default();
            receipt.pay_previous_margin(margin_due);
            receipt.pay_previous_interest(interest_rate.of(lease_coin));
            receipt.pay_principal(lease_coin, lease_coin);
            receipt.keep_change(surplus);
            assert_eq!(loan.repay(repay_fully, payment_at, lease,), Ok(receipt));
        }
    }

    #[test]
    fn partial_current_margin_repay() {
        let addr = "unused_addr";

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let repay_amount = lease_amount / 4;
        let repay_coin = coin(repay_amount);

        let interest_rate = Percent::from_permille(50);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let now = LEASE_START + Duration::from_nanos(Duration::YEAR.nanos() - 1);

        let mut loan = create_loan(loan, now);

        let receipt = loan.repay(repay_coin, now, Addr::unchecked(addr)).unwrap();

        assert_eq!(receipt, {
            let mut receipt = RepayReceipt::default();

            receipt.pay_current_margin(repay_coin);

            receipt
        },);

        let state = loan.state(now, Addr::unchecked(addr)).unwrap().unwrap();

        assert_eq!(state.previous_margin_interest_due, Coin::default());

        assert_eq!(state.previous_interest_due, Coin::default());
    }

    #[test]
    fn partial_previous_interest_repay() {
        let addr = "unused_addr";
        let addr_obj = Addr::unchecked(addr);

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let repay_amount = lease_amount / 4;
        let repay_coin = coin(repay_amount);
        let repay_at = LEASE_START + Duration::YEAR;

        let interest_rate = Percent::from_permille(500);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let mut loan = create_loan(loan, repay_at);
        let margin_interest = MARGIN_INTEREST_RATE.of(lease_coin);
        {
            let mut exp_full_prev_margin = RepayReceipt::default();
            exp_full_prev_margin.pay_previous_margin(margin_interest);
            assert_eq!(
                exp_full_prev_margin,
                loan.repay(margin_interest, repay_at, addr_obj.clone())
                    .unwrap()
            );
        }

        {
            let mut exp_receipt = RepayReceipt::default();
            exp_receipt.pay_previous_interest(repay_coin);
            assert_eq!(
                exp_receipt,
                loan.repay(repay_coin, repay_at, addr_obj).unwrap()
            );
        }
    }

    #[test]
    fn full_previous_partial_current_interest_repay() {
        let addr = "unused_addr";

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let interest_amount = lease_amount / 2;
        let interest_coin = coin(interest_amount);

        let repay_amount = lease_amount;
        let repay_coin = coin(repay_amount);

        let interest_rate = Percent::from_permille(500);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let repay_at = LEASE_START + Duration::YEAR;
        let mut loan = create_loan(loan, repay_at);

        let receipt = loan
            .repay(repay_coin, repay_at, Addr::unchecked(addr))
            .unwrap();

        assert_eq!(receipt, {
            let mut receipt = RepayReceipt::default();

            receipt.pay_previous_margin(MARGIN_INTEREST_RATE.of(lease_coin));

            receipt.pay_previous_interest(interest_coin);

            receipt
        },);
    }

    #[test]
    fn partial_principal_repay() {
        let addr = "unused_addr";

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let repay_amount = lease_amount / 2;
        let repay_coin = coin(repay_amount);

        let interest_rate = Percent::from_permille(500);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let repay_at = LEASE_START;
        let mut lease = create_loan(loan, repay_at);

        let receipt = lease
            .repay(repay_coin, repay_at, Addr::unchecked(addr))
            .unwrap();

        assert_eq!(receipt, {
            let mut receipt = RepayReceipt::default();

            receipt.pay_principal(lease_coin, repay_coin);

            receipt
        },);
    }

    #[test]
    fn partial_interest_principal_repay() {
        let addr = "unused_addr";

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let interest_amount = lease_amount / 4;
        let interest_coin = coin(interest_amount);

        let repay_amount = lease_amount;
        let repay_coin = coin(repay_amount);

        let interest_rate = Percent::from_permille(250);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let repay_at = LEASE_START + Duration::YEAR;
        let mut loan = create_loan(loan, repay_at);

        let receipt = loan
            .repay(repay_coin, repay_at, Addr::unchecked(addr))
            .unwrap();

        assert_eq!(receipt, {
            let mut receipt = RepayReceipt::default();

            receipt.pay_previous_margin(MARGIN_INTEREST_RATE.of(lease_coin));

            receipt.pay_previous_interest(interest_coin);

            receipt.pay_principal(
                lease_coin,
                repay_coin - interest_coin - receipt.previous_margin_paid(),
            );

            receipt
        },);
    }

    #[test]
    fn full_principal_repay() {
        let addr = "unused_addr";

        let lease_amount = 1000;
        let lease_coin = coin(lease_amount);

        let interest_rate = Percent::from_permille(500);

        // LPP loan
        let loan = LoanResponse {
            principal_due: lease_coin,
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let repay_at = LEASE_START;
        let mut lease = create_loan(loan, repay_at);

        let receipt = lease
            .repay(lease_coin, repay_at, Addr::unchecked(addr))
            .unwrap();

        assert_eq!(receipt, {
            let mut receipt = RepayReceipt::default();

            receipt.pay_principal(lease_coin, lease_coin);

            receipt
        },);
    }

    fn interest<Lpn>(period: Duration, principal_due: Coin<Lpn>, rate: Percent) -> Coin<Lpn>
    where
        Lpn: Currency,
    {
        InterestPeriod::with_interest(rate)
            .spanning(period)
            .interest(principal_due)
    }

    fn interests<Lpn>(
        paid: Timestamp,
        now: Timestamp,
        principal_due: Coin<Lpn>,
        rate: Percent,
    ) -> (Coin<Lpn>, Coin<Lpn>)
    where
        Lpn: Currency,
    {
        (
            interest(
                if now < LEASE_START + Duration::YEAR {
                    Duration::default()
                } else {
                    Duration::between(paid, LEASE_START + Duration::YEAR)
                },
                principal_due,
                rate,
            ),
            interest(
                Duration::between(
                    if now < LEASE_START + Duration::YEAR {
                        paid
                    } else {
                        LEASE_START + Duration::YEAR
                    },
                    now,
                ),
                principal_due,
                rate,
            ),
        )
    }

    fn margin_interests<Lpn>(
        paid: Timestamp,
        now: Timestamp,
        principal_due: Coin<Lpn>,
    ) -> (Coin<Lpn>, Coin<Lpn>)
    where
        Lpn: Currency,
    {
        interests(paid, now, principal_due, MARGIN_INTEREST_RATE)
    }

    fn test_state(period: Duration) {
        const LEASE_ADDRESS: &str = "";

        let principal_due = coin(10000);

        let interest_rate = Percent::from_permille(25);

        let loan_resp = LoanResponse {
            principal_due,
            annual_interest_rate: interest_rate,
            interest_paid: LEASE_START,
        };

        let now = LEASE_START + period;
        let loan = create_loan(loan_resp.clone(), now);

        let (expected_margin_overdue, expected_margin_due) =
            margin_interests(loan_resp.interest_paid, now, principal_due);

        let (expected_interest_overdue, expected_interest_due) = interests(
            loan_resp.interest_paid,
            now,
            principal_due,
            loan_resp.annual_interest_rate,
        );

        let res = loan
            .state(now, Addr::unchecked(LEASE_ADDRESS))
            .unwrap()
            .unwrap();

        assert_eq!(
            res.previous_margin_interest_due, expected_margin_overdue,
            "Got different margin overdue than expected!",
        );

        assert_eq!(
            res.current_margin_interest_due, expected_margin_due,
            "Got different margin due than expected!",
        );

        assert_eq!(
            res.previous_interest_due, expected_interest_overdue,
            "Got different interest overdue than expected!",
        );

        assert_eq!(
            res.current_interest_due, expected_interest_due,
            "Got different interest due than expected!",
        );
    }

    #[test]
    fn state_zero() {
        test_state(Duration::default())
    }

    #[test]
    fn state_day() {
        test_state(Duration::from_days(1))
    }

    #[test]
    fn state_year() {
        test_state(Duration::YEAR)
    }

    #[test]
    fn state_year_plus_day() {
        test_state(Duration::YEAR + Duration::from_days(1))
    }

    #[test]
    fn state_year_minus_day() {
        test_state(Duration::YEAR - Duration::from_days(1))
    }

    fn coin(a: Amount) -> Coin<TestCurrency> {
        Coin::<TestCurrency>::new(a)
    }
}
