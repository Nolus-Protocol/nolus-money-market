use serde::{Deserialize, Serialize};

use finance::{
    coin::Coin, duration::Duration, error::Error as FinanceError, interest, percent::Percent,
    period::Period, zero::Zero,
};
use lpp::{
    loan::RepayShares,
    stub::{loan::LppLoan as LppLoanTrait, LppBatch, LppRef as LppGenericRef},
};
use platform::{bank::FixedAddressSender, batch::Batch};
use profit::stub::ProfitRef;
use sdk::cosmwasm_std::Timestamp;

use crate::{
    error::{ContractError, ContractResult},
    finance::{LpnCoin, LpnCurrencies, LpnCurrency},
};

pub(crate) use self::repay::Receipt as RepayReceipt;
pub use self::state::{Overdue, State};

mod repay;
mod state;

type LppRef = LppGenericRef<LpnCurrency, LpnCurrencies>;

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct LoanDTO {
    lpp: LppRef,
    profit: ProfitRef,
    due_period: Duration,
    margin_interest: Percent,
    margin_paid_by: Timestamp, // only this one should vary!
}

impl LoanDTO {
    pub(crate) fn annual_margin_interest(&self) -> Percent {
        self.margin_interest
    }

    pub(crate) fn lpp(&self) -> &LppRef {
        &self.lpp
    }

    pub(crate) fn profit(&self) -> &ProfitRef {
        &self.profit
    }
}

#[cfg_attr(test, derive(Debug))]
pub struct Loan<LppLoan> {
    lpp_loan: LppLoan,
    due_period: Duration,
    margin_interest: Percent,
    margin_paid_by: Timestamp, // only this one should vary!
}

impl<LppLoan> Loan<LppLoan>
where
    LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    LppLoan::Error: Into<ContractError>,
{
    pub(super) fn try_into_dto(self, profit: ProfitRef) -> ContractResult<(LoanDTO, Batch)> {
        Self::try_loan_into(self.lpp_loan).map(|lpp_batch: LppBatch<LppRef>| {
            (
                LoanDTO {
                    lpp: lpp_batch.lpp_ref,
                    profit,
                    due_period: self.due_period,
                    margin_interest: self.margin_interest,
                    margin_paid_by: self.margin_paid_by,
                },
                lpp_batch.batch,
            )
        })
    }

    pub(super) fn try_into_messages(self) -> ContractResult<Batch> {
        Self::try_loan_into(self.lpp_loan).map(|lpp_batch: LppBatch<LppRef>| lpp_batch.batch)
    }

    fn try_loan_into(loan: LppLoan) -> ContractResult<LppBatch<LppRef>> {
        loan.try_into().map_err(Into::<ContractError>::into)
    }
}

impl<LppLoan> Loan<LppLoan>
where
    LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
{
    pub(super) fn new(
        lpp_loan: LppLoan,
        start: Timestamp,
        annual_margin_interest: Percent,
        due_period: Duration,
    ) -> Self {
        Self {
            lpp_loan,
            due_period,
            margin_interest: annual_margin_interest,
            margin_paid_by: start,
        }
    }

    pub(super) fn from_dto(dto: LoanDTO, lpp_loan: LppLoan) -> Self {
        Self {
            lpp_loan,
            due_period: dto.due_period,
            margin_interest: dto.margin_interest,
            margin_paid_by: dto.margin_paid_by,
        }
    }

    /// Repay the loan interests and principal by the given timestamp.
    ///
    /// The time intervals are always open-ended!
    pub(crate) fn repay<Profit>(
        &mut self,
        payment: LpnCoin,
        by: &Timestamp,
        profit: &mut Profit,
    ) -> ContractResult<RepayReceipt>
    where
        Profit: FixedAddressSender,
    {
        self.debug_check_start_due_before(by, "before the 'repay-by' time");

        self.state(by)
            .ok_or(ContractError::FinanceError(FinanceError::Overflow(
                format!(
                    "Failed to calculate the lease state at the specified time: {:?}",
                    by
                ),
            )))
            .and_then(|state| {
                let overdue_interest_payment = state.overdue.interest().min(payment);
                let overdue_margin_payment = state
                    .overdue
                    .margin()
                    .min(payment - overdue_interest_payment);
                let due_interest_payment = state
                    .due_interest
                    .min(payment - overdue_interest_payment - overdue_margin_payment);
                let due_margin_payment = state.due_margin_interest.min(
                    payment
                        - overdue_interest_payment
                        - overdue_margin_payment
                        - due_interest_payment,
                );

                let interest_paid = overdue_interest_payment + due_interest_payment;
                let margin_paid = overdue_margin_payment + due_margin_payment;
                let principal_paid = state
                    .principal_due
                    .min(payment - interest_paid - margin_paid);
                let change = payment - interest_paid - margin_paid - principal_paid;
                debug_assert_eq!(
                    payment,
                    interest_paid + margin_paid + principal_paid + change
                );

                self.repay_margin(state.principal_due, margin_paid, by)
                    .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                        "during margin repayment",
                        state.principal_due,
                        margin_paid,
                    )))
                    .and_then(|()| {
                        profit.send(margin_paid);
                        self.repay_loan(interest_paid, principal_paid, by)
                            .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                                "during loan repayment",
                                interest_paid,
                                principal_paid,
                            )))
                            .map(|()| {
                                let receipt = RepayReceipt::new(
                                    overdue_interest_payment,
                                    overdue_margin_payment,
                                    due_interest_payment,
                                    due_margin_payment,
                                    state.principal_due,
                                    principal_paid,
                                    change,
                                );
                                debug_assert_eq!(payment, receipt.total());

                                receipt
                            })
                    })
            })
    }

    pub(crate) fn state(&self, now: &Timestamp) -> Option<State> {
        self.debug_check_start_due_before(now, "in the past. Now is ");

        let due_period_margin = Period::from_till(self.margin_paid_by, now);

        Overdue::new(
            &due_period_margin,
            self.due_period,
            self.margin_interest,
            &self.lpp_loan,
        )
        .and_then(|overdue| {
            let principal_due = self.lpp_loan.principal_due();

            interest::interest(
                self.margin_interest,
                principal_due,
                due_period_margin.length(),
            )
            .and_then(|interest| {
                let due_margin_interest = interest - overdue.margin();

                self.lpp_loan
                    .interest_due(&due_period_margin.till())
                    .map(|due_interest| State {
                        annual_interest: self.lpp_loan.annual_interest_rate(),
                        annual_interest_margin: self.margin_interest,
                        principal_due,
                        due_interest: due_interest - overdue.interest(),
                        due_margin_interest,
                        overdue,
                    })
            })
        })
    }

    fn repay_margin(
        &mut self,
        principal_due: LpnCoin,
        margin_paid: LpnCoin,
        by: &Timestamp,
    ) -> Option<()> {
        interest::pay(
            self.margin_interest,
            principal_due,
            margin_paid,
            Duration::between(&self.margin_paid_by, by),
        )
        .map(|(margin_paid_for, margin_payment_change)| {
            debug_assert!(margin_payment_change.is_zero());
            self.margin_paid_by += margin_paid_for;
        })
    }

    fn repay_loan(
        &mut self,
        interest_paid: LpnCoin,
        principal_paid: LpnCoin,
        by: &Timestamp,
    ) -> Option<()> {
        self.lpp_loan.repay(by, interest_paid + principal_paid).map(
            |RepayShares {
                 interest,
                 principal,
                 excess,
             }| {
                debug_assert_eq!(interest, interest_paid);
                debug_assert_eq!(principal, principal_paid);
                debug_assert_eq!(excess, Coin::ZERO);
            },
        )
    }

    fn debug_check_start_due_before(&self, when: &Timestamp, when_descr: &str) {
        debug_assert!(
            &self.margin_paid_by <= when,
            "The current due period starting at {}s, should begin {} {}s",
            self.margin_paid_by,
            when_descr,
            when
        );
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    pub use currencies::Lpn;
    use finance::{duration::Duration, percent::Percent};
    use lpp::{
        error::{ContractError as LppError, Result as LppResult},
        loan::RepayShares,
        msg::LoanResponse,
        stub::{loan::LppLoan as LppLoanTrait, LppBatch},
    };
    use platform::bank::FixedAddressSender;
    use profit::stub::ProfitRef;
    use sdk::cosmwasm_std::Timestamp;

    use crate::finance::{LpnCoin, LpnCurrencies};

    use super::{Loan, LppRef};

    const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(50);
    const LOAN_INTEREST_RATE: Percent = Percent::from_permille(500);
    const LEASE_START: Timestamp = Timestamp::from_nanos(100);
    const PROFIT_ADDR: &str = "profit_addr";

    mod test_repay {
        use serde::{Deserialize, Serialize};

        use currency::{Currency, Group};
        use finance::{
            coin::{Amount, Coin, WithCoin},
            duration::Duration,
            fraction::Fraction,
            percent::Percent,
            zero::Zero,
        };
        use lpp::msg::LoanResponse;
        use platform::{
            bank::{self, Aggregate, BalancesResult, BankAccountView},
            batch::Batch,
            result::Result as PlatformResult,
        };
        use sdk::cosmwasm_std::{Addr, Timestamp};

        use crate::{
            finance::LpnCoin,
            loan::{
                repay::Receipt as RepayReceipt,
                tests::{create_loan_custom, profit_stub, PROFIT_ADDR},
                Loan, Overdue, State,
            },
        };

        use super::{
            create_loan, LppLoanLocal, LEASE_START, LOAN_INTEREST_RATE, MARGIN_INTEREST_RATE,
        };

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct BankStub {
            balance: Amount,
        }

        impl BankAccountView for BankStub {
            fn balance<C, G>(&self) -> PlatformResult<Coin<C>>
            where
                C: Currency,
            {
                Ok(Coin::<C>::new(self.balance))
            }

            fn balances<G, Cmd>(&self, _: Cmd) -> BalancesResult<G, Cmd>
            where
                G: Group,
                Cmd: WithCoin<G>,
                Cmd::Output: Aggregate,
            {
                unimplemented!()
            }
        }

        #[test]
        fn full_max_overdue_full_max_due_repay() {
            let principal = 1000;
            let delta_to_fully_paid = 30;
            let payment_at = LEASE_START + Duration::YEAR + Duration::YEAR;
            let one_year_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let one_year_interest = LOAN_INTEREST_RATE.of(principal).unwrap();
            assert!(delta_to_fully_paid < one_year_margin);
            assert!(delta_to_fully_paid < one_year_interest);

            let loan = LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            };

            let mut loan = create_loan(loan);
            {
                let repay_overdue_interest = one_year_interest - delta_to_fully_paid;
                repay(
                    &mut loan,
                    repay_overdue_interest,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: one_year_interest.into(),
                            margin: one_year_margin.into(),
                        },
                    ),
                    receipt(principal, 0, 0, repay_overdue_interest, 0, 0, 0),
                    Duration::default(),
                    &payment_at,
                )
            }

            {
                let repay_fully_overdue_interest_and_some_margin =
                    delta_to_fully_paid + delta_to_fully_paid;
                repay(
                    &mut loan,
                    repay_fully_overdue_interest_and_some_margin,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: delta_to_fully_paid.into(),
                            margin: one_year_margin.into(),
                        },
                    ),
                    receipt(
                        principal,
                        0,
                        repay_fully_overdue_interest_and_some_margin - delta_to_fully_paid,
                        delta_to_fully_paid,
                        0,
                        0,
                        0,
                    ),
                    Duration::default(),
                    &payment_at,
                )
            }

            {
                let overdue_margin = one_year_margin - delta_to_fully_paid;
                let repay_fully_overdue_margin_and_some_due_interest =
                    overdue_margin + delta_to_fully_paid;
                repay(
                    &mut loan,
                    repay_fully_overdue_margin_and_some_due_interest,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: 0.into(),
                            margin: overdue_margin.into(),
                        },
                    ),
                    receipt(
                        principal,
                        0,
                        overdue_margin,
                        0,
                        0,
                        repay_fully_overdue_margin_and_some_due_interest - overdue_margin,
                        0,
                    ),
                    Duration::default(),
                    &payment_at,
                )
            }

            {
                let interest_due = one_year_interest - delta_to_fully_paid;
                let surplus = delta_to_fully_paid;
                let full_repayment = interest_due + one_year_margin + principal + surplus;
                repay(
                    &mut loan,
                    full_repayment,
                    state(
                        principal,
                        one_year_margin,
                        interest_due,
                        Overdue::Accrued {
                            interest: 0.into(),
                            margin: 0.into(),
                        },
                    ),
                    receipt(
                        principal,
                        principal,
                        0,
                        0,
                        one_year_margin,
                        interest_due,
                        surplus,
                    ),
                    Duration::YEAR,
                    &payment_at,
                )
            }
        }

        #[test]
        fn partial_max_due_margin_repay() {
            let principal = 1000;
            let due_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let payment = due_margin / 2;
            let now = LEASE_START + Duration::YEAR;

            let mut loan = create_loan(LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: now,
            });

            repay(
                &mut loan,
                payment,
                state(
                    principal,
                    due_margin,
                    0,
                    Overdue::Accrued {
                        interest: 0.into(),
                        margin: 0.into(),
                    },
                ),
                receipt(principal, 0, 0, 0, payment, 0, 0),
                Duration::YEAR
                    .into_slice_per_ratio::<LpnCoin>(payment.into(), due_margin.into())
                    .unwrap(),
                &now,
            );
        }

        #[test]
        fn partial_overdue_interest_repay() {
            let principal = 1000;
            let one_year_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let one_year_interest = LOAN_INTEREST_RATE.of(principal).unwrap();
            let overdue_period = Duration::from_days(100);
            let overdue_interest = overdue_period
                .annualized_slice_of(one_year_interest)
                .unwrap();
            let overdue_margin = overdue_period.annualized_slice_of(one_year_margin).unwrap();

            let partial_overdue_interest = overdue_interest - 10;
            let repay_at = LEASE_START + Duration::YEAR + overdue_period;

            let loan = LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            };

            let mut loan = create_loan(loan);
            {
                let payment = partial_overdue_interest;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: overdue_interest.into(),
                            margin: overdue_margin.into(),
                        },
                    ),
                    receipt(principal, 0, 0, partial_overdue_interest, 0, 0, 0),
                    Duration::default(),
                    &repay_at,
                );
            }
        }

        #[test]
        fn multiple_periods() {
            let principal = 1000;
            let one_year_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let one_year_interest = LOAN_INTEREST_RATE.of(principal).unwrap();
            let overdue_period_molulo_year = Duration::from_days(120);
            let repay_at = LEASE_START
                + overdue_period_molulo_year
                + Duration::YEAR
                + Duration::YEAR
                + Duration::YEAR;

            let overdue_margin_modulo_year = overdue_period_molulo_year
                .annualized_slice_of(one_year_margin)
                .unwrap();
            let overdue_interest_modulo_year = overdue_period_molulo_year
                .annualized_slice_of(one_year_interest)
                .unwrap();
            let interest_payment = overdue_interest_modulo_year - 10;

            let loan = LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            };

            let mut loan = create_loan(loan);
            {
                let payment = one_year_interest + one_year_interest + interest_payment;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: (one_year_interest * 2 + overdue_interest_modulo_year).into(),
                            margin: (one_year_margin * 2 + overdue_margin_modulo_year).into(),
                        },
                    ),
                    receipt(principal, 0, 0, payment, 0, 0, 0),
                    Duration::default(),
                    &repay_at,
                );
            }
            {
                let payment =
                    overdue_interest_modulo_year - interest_payment + overdue_margin_modulo_year;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: (overdue_interest_modulo_year - interest_payment).into(),
                            margin: (one_year_margin * 2 + overdue_margin_modulo_year).into(),
                        },
                    ),
                    receipt(
                        principal,
                        0,
                        overdue_margin_modulo_year,
                        overdue_interest_modulo_year - interest_payment,
                        0,
                        0,
                        0,
                    ),
                    Duration::default(),
                    &repay_at,
                );
            }
            {
                let payment = one_year_margin * 2 + interest_payment;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest,
                        Overdue::Accrued {
                            interest: 0.into(),
                            margin: (one_year_margin * 2).into(),
                        },
                    ),
                    receipt(principal, 0, one_year_margin * 2, 0, 0, interest_payment, 0),
                    Duration::default(),
                    &repay_at,
                );
            }
            {
                let change = 3;
                let payment =
                    (one_year_interest - interest_payment) + one_year_margin + principal + change;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        one_year_margin,
                        one_year_interest - interest_payment,
                        Overdue::Accrued {
                            interest: 0.into(),
                            margin: 0.into(),
                        },
                    ),
                    receipt(
                        principal,
                        principal,
                        0,
                        0,
                        one_year_margin,
                        one_year_interest - interest_payment,
                        change,
                    ),
                    Duration::YEAR,
                    &repay_at,
                );
            }
        }

        #[test]
        fn full_max_overdue_full_due_repay() {
            let principal = 57326;
            let due_margin_payment = 42;
            let due_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let due_interest = LOAN_INTEREST_RATE.of(principal).unwrap();

            let loan = LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            };

            let overdue_period =
                Duration::YEAR - Duration::HOUR - Duration::HOUR - Duration::HOUR - Duration::HOUR;
            let repay_at = LEASE_START + Duration::YEAR + overdue_period;
            let overdue_margin = overdue_period.annualized_slice_of(due_margin).unwrap();
            let overdue_interest = overdue_period.annualized_slice_of(due_interest).unwrap();
            let payment = overdue_interest + overdue_margin + due_interest + due_margin_payment;
            let due_period_paid = Duration::between(&LEASE_START, &repay_at)
                .into_slice_per_ratio::<LpnCoin>(
                    (overdue_margin + due_margin_payment).into(),
                    (overdue_margin + due_margin).into(),
                )
                .unwrap()
                - overdue_period;

            let mut loan = create_loan(loan);
            repay(
                &mut loan,
                payment,
                state(
                    principal,
                    due_margin,
                    due_interest,
                    Overdue::Accrued {
                        interest: overdue_interest.into(),
                        margin: overdue_margin.into(),
                    },
                ),
                receipt(
                    principal,
                    0,
                    overdue_margin,
                    overdue_interest,
                    due_margin_payment,
                    due_interest,
                    0,
                ),
                due_period_paid,
                &repay_at,
            );
        }

        #[test]
        fn full_partial_due_repay() {
            let principal = 36463892;
            let principal_paid = 234;
            let one_year_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let one_year_interest = LOAN_INTEREST_RATE.of(principal).unwrap();
            let due_period = Duration::HOUR + Duration::HOUR + Duration::HOUR;
            let due_margin = due_period.annualized_slice_of(one_year_margin).unwrap();
            let due_interest = due_period.annualized_slice_of(one_year_interest).unwrap();
            let payment = due_margin + due_interest + principal_paid;

            let repay_at = LEASE_START + due_period;
            let mut loan = create_loan(LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            });
            repay(
                &mut loan,
                payment,
                state(
                    principal,
                    due_margin,
                    due_interest,
                    Overdue::StartIn(Duration::YEAR - due_period),
                ),
                receipt(principal, principal_paid, 0, 0, due_margin, due_interest, 0),
                due_period,
                &repay_at,
            );
        }

        #[test]
        fn full_zero_loan_overdue_partial_due_repay() {
            // selected to have interest > 0 and margin == 0 for the overdue period of 2 hours
            let principal = 9818;
            let loan_interest_rate = MARGIN_INTEREST_RATE; // we aim at simulating the margin paid-by going ahead of the loan paid-by
            let margin_interest_rate = LOAN_INTEREST_RATE;
            let principal_paid = 23;
            let due_margin = margin_interest_rate.of(principal).unwrap();
            let due_interest = loan_interest_rate.of(principal).unwrap();
            let overdue_period = Duration::HOUR + Duration::HOUR;
            let overdue_interest = overdue_period.annualized_slice_of(due_interest).unwrap();
            assert_eq!(Amount::ZERO, overdue_interest);
            let overdue_margin = overdue_period.annualized_slice_of(due_margin).unwrap();
            assert!(Amount::ZERO != overdue_margin);

            let loan = LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: loan_interest_rate,
                interest_paid: LEASE_START,
            };

            let repay_at = LEASE_START + Duration::YEAR + Duration::HOUR + Duration::HOUR;
            let mut loan =
                create_loan_custom(margin_interest_rate, loan, LEASE_START, Duration::YEAR);
            repay(
                &mut loan,
                overdue_interest + overdue_margin,
                state_custom_percents(
                    loan_interest_rate,
                    margin_interest_rate,
                    principal,
                    due_margin,
                    due_interest,
                    Overdue::Accrued {
                        interest: overdue_interest.into(),
                        margin: overdue_margin.into(),
                    },
                ),
                receipt(principal, 0, overdue_margin, overdue_interest, 0, 0, 0),
                Duration::default(),
                &repay_at,
            );
            repay(
                &mut loan,
                due_margin + due_interest + principal_paid,
                state_custom_percents(
                    loan_interest_rate,
                    margin_interest_rate,
                    principal,
                    due_margin,
                    due_interest,
                    Overdue::Accrued {
                        interest: 0.into(),
                        margin: 0.into(),
                    },
                ),
                receipt(principal, principal_paid, 0, 0, due_margin, due_interest, 0),
                Duration::YEAR,
                &repay_at,
            );
        }

        #[test]
        fn full_principal_repay() {
            let principal = 3646389225881;
            let principal_paid = 234;
            let one_year_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let one_year_interest = LOAN_INTEREST_RATE.of(principal).unwrap();
            let due_period = Duration::HOUR + Duration::HOUR + Duration::HOUR;
            let due_margin = due_period.annualized_slice_of(one_year_margin).unwrap();
            let due_interest = due_period.annualized_slice_of(one_year_interest).unwrap();
            let mut loan = create_loan(LoanResponse {
                principal_due: principal.into(),
                annual_interest_rate: LOAN_INTEREST_RATE,
                interest_paid: LEASE_START,
            });
            {
                let payment = due_margin + due_interest + principal_paid;
                let repay_at = LEASE_START + due_period;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        due_margin,
                        due_interest,
                        Overdue::StartIn(Duration::YEAR - due_period),
                    ),
                    receipt(principal, principal_paid, 0, 0, due_margin, due_interest, 0),
                    due_period,
                    &repay_at,
                )
            }

            {
                let principal_due = principal - principal_paid;
                let change = 97;
                let duration_since_prev_payment = Duration::YEAR - due_period;
                let due_margin = duration_since_prev_payment
                    .annualized_slice_of(MARGIN_INTEREST_RATE.of(principal_due).unwrap())
                    .unwrap();
                let due_interest = duration_since_prev_payment
                    .annualized_slice_of(LOAN_INTEREST_RATE.of(principal_due).unwrap())
                    .unwrap();
                let payment = due_margin + due_interest + principal_due + change;
                let repay_at = LEASE_START + Duration::YEAR;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal_due,
                        due_margin,
                        due_interest,
                        Overdue::StartIn(due_period),
                    ),
                    receipt(
                        principal_due,
                        principal_due,
                        0,
                        0,
                        due_margin,
                        due_interest,
                        change,
                    ),
                    duration_since_prev_payment,
                    &repay_at,
                )
            }
        }

        #[test]
        fn repay_zero() {
            let principal = 13;
            let total_margin = MARGIN_INTEREST_RATE.of(principal).unwrap();
            let total_interest = LOAN_INTEREST_RATE.of(principal).unwrap();

            let due_period = Duration::HOUR;
            let since_start = Duration::YEAR;
            let mut loan = create_loan_custom(
                MARGIN_INTEREST_RATE,
                LoanResponse {
                    principal_due: principal.into(),
                    annual_interest_rate: LOAN_INTEREST_RATE,
                    interest_paid: LEASE_START,
                },
                LEASE_START,
                due_period,
            );
            let repay_at = LEASE_START + since_start;
            let principal_left = {
                let due_period_paid = Duration::default();

                let overdue_margin = (since_start - due_period)
                    .annualized_slice_of(total_margin)
                    .unwrap();
                let due_margin = total_margin - overdue_margin;
                assert_eq!(Amount::ZERO, due_margin);
                assert_eq!(Amount::ZERO, total_margin);

                let overdue_interest = (since_start - due_period)
                    .annualized_slice_of(total_interest)
                    .unwrap();
                let due_interest = total_interest - overdue_interest;
                assert_eq!(1, due_interest);

                let payment = 15;
                let principal_paid =
                    payment - overdue_margin - due_margin - overdue_interest - due_interest;

                repay(
                    &mut loan,
                    payment,
                    state(
                        principal,
                        due_margin,
                        due_interest,
                        Overdue::Accrued {
                            interest: overdue_interest.into(),
                            margin: overdue_margin.into(),
                        },
                    ),
                    receipt(
                        principal,
                        principal_paid,
                        overdue_margin,
                        overdue_interest,
                        due_margin,
                        due_interest,
                        payment
                            - principal_paid
                            - overdue_margin
                            - overdue_interest
                            - due_margin
                            - due_interest,
                    ),
                    due_period_paid,
                    &repay_at,
                );
                principal - principal_paid
            };
            {
                let change = 2;
                let payment = principal_left + change;
                let repay_at = LEASE_START + since_start;
                repay(
                    &mut loan,
                    payment,
                    state(
                        principal_left,
                        0,
                        0,
                        Overdue::Accrued {
                            interest: 0.into(),
                            margin: 0.into(),
                        },
                    ),
                    receipt(principal_left, principal_left, 0, 0, 0, 0, change),
                    Duration::default(),
                    &repay_at,
                );
            }
        }

        #[track_caller]
        fn repay<P>(
            loan: &mut Loan<LppLoanLocal>,
            payment: P,
            before_state: State,
            exp_receipt: RepayReceipt,
            exp_due_period_paid: Duration,
            now: &Timestamp,
        ) where
            P: Into<LpnCoin> + Copy,
        {
            let mut profit = profit_stub();

            assert_eq!(
                before_state,
                loan.state(now).unwrap(),
                "Expected state before"
            );
            assert_eq!(
                Ok(exp_receipt),
                loan.repay(payment.into(), now, &mut profit)
            );
            assert_eq!(
                after_state(before_state, exp_due_period_paid, exp_receipt),
                loan.state(now).unwrap(),
                "Expected state after"
            );

            assert_eq!(
                {
                    let margin_paid =
                        exp_receipt.overdue_margin_paid() + exp_receipt.due_margin_paid();
                    if margin_paid != Coin::default() {
                        bank::bank_send(Addr::unchecked(PROFIT_ADDR), margin_paid)
                    } else {
                        Batch::default()
                    }
                },
                Into::<Batch>::into(profit)
            )
        }

        fn after_state(
            before_state: State,
            exp_due_period_paid: Duration,
            exp_receipt: RepayReceipt,
        ) -> State {
            let exp_overdue = if before_state.overdue.start_in() == Duration::default() {
                let exp_interest =
                    before_state.overdue.interest() - exp_receipt.overdue_interest_paid();
                let exp_margin = before_state.overdue.margin() - exp_receipt.overdue_margin_paid();
                if exp_interest.is_zero()
                    && exp_margin.is_zero()
                    && exp_due_period_paid != Duration::default()
                {
                    Overdue::StartIn(exp_due_period_paid)
                } else {
                    Overdue::Accrued {
                        interest: exp_interest,
                        margin: exp_margin,
                    }
                }
            } else {
                Overdue::StartIn(before_state.overdue.start_in() + exp_due_period_paid)
            };
            State {
                annual_interest_margin: before_state.annual_interest_margin,
                annual_interest: before_state.annual_interest,
                principal_due: before_state.principal_due - exp_receipt.principal_paid(),
                due_margin_interest: before_state.due_margin_interest
                    - exp_receipt.due_margin_paid(),
                due_interest: before_state.due_interest - exp_receipt.due_interest_paid(),
                overdue: exp_overdue,
            }
        }

        fn state<P>(
            principal: P,
            due_margin_interest: P,
            due_interest: P,
            overdue: Overdue,
        ) -> State
        where
            P: Into<LpnCoin>,
        {
            state_custom_percents(
                LOAN_INTEREST_RATE,
                MARGIN_INTEREST_RATE,
                principal,
                due_margin_interest,
                due_interest,
                overdue,
            )
        }

        fn state_custom_percents<P>(
            annual_interest: Percent,
            annual_interest_margin: Percent,
            principal: P,
            due_margin_interest: P,
            due_interest: P,
            overdue: Overdue,
        ) -> State
        where
            P: Into<LpnCoin>,
        {
            State {
                annual_interest,
                annual_interest_margin,
                principal_due: principal.into(),
                due_margin_interest: due_margin_interest.into(),
                due_interest: due_interest.into(),
                overdue,
            }
        }

        fn receipt<P>(
            principal: P,
            paid_principal: P,
            paid_overdue_margin: P,
            paid_overdue_interest: P,
            paid_due_margin: P,
            paid_due_interest: P,
            change: P,
        ) -> RepayReceipt
        where
            P: Into<LpnCoin>,
        {
            RepayReceipt::new(
                paid_overdue_interest.into(),
                paid_overdue_margin.into(),
                paid_due_interest.into(),
                paid_due_margin.into(),
                principal.into(),
                paid_principal.into(),
                change.into(),
            )
        }
    }

    #[cfg(test)]
    mod test_state {
        use finance::{duration::Duration, interest, percent::Percent, period::Period};
        use lpp::{msg::LoanResponse, stub::loan::LppLoan};
        use sdk::cosmwasm_std::Timestamp;

        use crate::loan::{
            tests::{create_loan_custom, LppLoanLocal},
            Overdue, State,
        };

        use super::{LEASE_START, MARGIN_INTEREST_RATE};

        #[track_caller]
        fn test_state(interest_paid_by: Timestamp, margin_paid_by: Timestamp, now: &Timestamp) {
            let principal_due = 10000.into();
            let due_period_len = Duration::YEAR;
            let annual_interest_margin = MARGIN_INTEREST_RATE;
            let annual_interest = Percent::from_permille(145);

            let loan_resp = LoanResponse {
                principal_due,
                annual_interest_rate: annual_interest,
                interest_paid: interest_paid_by,
            };

            let loan = create_loan_custom(
                MARGIN_INTEREST_RATE,
                loan_resp.clone(),
                margin_paid_by,
                due_period_len,
            );
            let due_period_margin = Period::from_till(margin_paid_by, now);
            let lpp_loan = LppLoanLocal::new(loan_resp);
            let overdue = Overdue::new(
                &due_period_margin,
                due_period_len,
                annual_interest_margin,
                &lpp_loan,
            )
            .unwrap();
            let due_period = due_period_len.min(due_period_margin.length());
            let expected_margin_due =
                interest::interest(annual_interest_margin, principal_due, due_period).unwrap();
            let expected_interest_due =
                lpp_loan.interest_due(&due_period_margin.till()).unwrap() - overdue.interest();

            assert_eq!(
                State {
                    annual_interest,
                    annual_interest_margin,
                    principal_due,
                    due_interest: expected_interest_due,
                    due_margin_interest: expected_margin_due,
                    overdue,
                },
                loan.state(now).unwrap(),
                "Got different state than expected!",
            );
        }

        fn test_states_paid_by(since_paid: Duration) {
            let paid_by = LEASE_START + since_paid;
            test_state(LEASE_START, LEASE_START, &paid_by);
            test_state(LEASE_START, paid_by, &paid_by);
            test_state(paid_by, LEASE_START, &paid_by);
        }

        #[test]
        fn state_at_open() {
            test_states_paid_by(Duration::default())
        }

        #[test]
        fn state_in_aday() {
            test_states_paid_by(Duration::from_days(1));
        }

        #[test]
        fn state_in_half_due_period() {
            test_states_paid_by(Duration::from_days(188));
        }

        #[test]
        fn state_year() {
            test_states_paid_by(Duration::YEAR)
        }

        #[test]
        fn state_year_plus_day() {
            test_states_paid_by(Duration::YEAR + Duration::from_days(1))
        }

        #[test]
        fn state_year_minus_day() {
            test_states_paid_by(Duration::YEAR - Duration::from_days(1))
        }

        #[test]
        fn state_two_years_plus_day() {
            test_states_paid_by(Duration::YEAR + Duration::YEAR + Duration::from_days(1))
        }
    }

    // TODO migrate to using lpp::stub::unchecked_lpp_loan
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub(super) struct LppLoanLocal {
        loan: LoanResponse<Lpn>,
    }

    impl LppLoanLocal {
        pub fn new(loan: LoanResponse<Lpn>) -> Self {
            Self { loan }
        }
    }

    impl LppLoanTrait<Lpn, LpnCurrencies> for LppLoanLocal {
        fn principal_due(&self) -> LpnCoin {
            self.loan.principal_due
        }

        fn interest_due(&self, by: &Timestamp) -> Option<LpnCoin> {
            self.loan.interest_due(by)
        }

        fn repay(&mut self, by: &Timestamp, repayment: LpnCoin) -> Option<RepayShares<Lpn>> {
            self.loan.repay(by, repayment)
        }

        fn annual_interest_rate(&self) -> Percent {
            self.loan.annual_interest_rate
        }
    }

    impl TryFrom<LppLoanLocal> for LppBatch<LppRef> {
        type Error = LppError;
        fn try_from(_: LppLoanLocal) -> LppResult<Self> {
            unreachable!()
        }
    }

    fn create_loan(loan: LoanResponse<Lpn>) -> Loan<LppLoanLocal> {
        create_loan_custom(MARGIN_INTEREST_RATE, loan, LEASE_START, Duration::YEAR)
    }

    fn create_loan_custom(
        annual_margin_interest: Percent,
        loan: LoanResponse<Lpn>,
        due_start: Timestamp,
        due_period: Duration,
    ) -> Loan<LppLoanLocal> {
        Loan::new(
            LppLoanLocal::new(loan),
            due_start,
            annual_margin_interest,
            due_period,
        )
    }

    fn profit_stub() -> impl FixedAddressSender {
        ProfitRef::unchecked(PROFIT_ADDR).into_stub()
    }
}
