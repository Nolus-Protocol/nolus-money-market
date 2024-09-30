use finance::{
    coin::Coin, duration::Duration, interest, percent::Percent, period::Period, zero::Zero,
};
use lpp::stub::loan::LppLoan as LppLoanTrait;

use crate::finance::{LpnCoin, LpnCurrencies, LpnCurrency};

#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub struct State {
    pub annual_interest: Percent,
    pub annual_interest_margin: Percent,
    pub principal_due: LpnCoin,
    pub due_interest: LpnCoin,
    pub due_margin_interest: LpnCoin,
    pub overdue: Overdue,
}

#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub enum Overdue {
    /// No overdue interest yet
    ///
    /// The period specifies in how much time the overdue will start.
    /// The interest accrued past it will be counted as overdue.
    /// Non-zero value.
    StartIn(Duration),

    /// The accrued interest so far is overdue
    ///
    /// The amounts accrued since the overdue period has started.
    Accrued { interest: LpnCoin, margin: LpnCoin },
}

impl Overdue {
    pub fn new<LppLoan>(
        due_period_margin: &Period,
        max_due: Duration,
        margin_interest: Percent,
        lpp_loan: &LppLoan,
    ) -> Option<Self>
    where
        LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    {
        if due_period_margin.length() < max_due {
            Some(Self::StartIn(max_due - due_period_margin.length()))
        } else {
            // due to the right-opened nature of intervals, if '==' then the due period end is the overdue period start
            let overdue_period = if due_period_margin.length() == max_due {
                Period::till_length(&due_period_margin.start(), Default::default())
            } else {
                let due_period_max = Period::till_length(&due_period_margin.till(), max_due);
                due_period_margin.cut(&due_period_max)
            };

            // TODO consider using the `trait InterestDue`
            interest::interest(
                margin_interest,
                lpp_loan.principal_due(),
                overdue_period.length(),
            )
            .and_then(|margin| {
                lpp_loan
                    .interest_due(&overdue_period.till())
                    .map(|interest| Self::Accrued { interest, margin })
            })
        }
    }

    pub fn start_in(&self) -> Duration {
        match self {
            Self::StartIn(start_in) => *start_in,
            Self::Accrued {
                interest: _,
                margin: _,
            } => Duration::default(),
        }
    }

    pub fn interest(&self) -> LpnCoin {
        match self {
            Self::StartIn(_) => Coin::ZERO,
            Self::Accrued {
                interest,
                margin: _,
            } => *interest,
        }
    }

    pub fn margin(&self) -> LpnCoin {
        match self {
            Self::StartIn(_) => Coin::ZERO,
            Self::Accrued {
                interest: _,
                margin,
            } => *margin,
        }
    }
}

#[cfg(test)]
mod test {
    use finance::{coin::Coin, duration::Duration, interest, percent::Percent, period::Period};
    use lpp::{loan::Loan, stub::loan::LppLoan};
    use sdk::cosmwasm_std::Timestamp;

    use crate::loan::tests::{Lpn, LppLoanLocal};

    use super::Overdue;

    const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(50);
    const LOAN: Loan<Lpn> = Loan {
        principal_due: Coin::new(1000),
        annual_interest_rate: Percent::from_permille(165),
        interest_paid: Timestamp::from_seconds(2425252),
    };

    #[test]
    fn due_period_less_than_max() {
        let max_due = Duration::YEAR;
        let due_period_length = Duration::from_days(130);
        let due_period_margin =
            Period::from_length(Timestamp::from_seconds(100), due_period_length);

        let overdue = Overdue::new(
            &due_period_margin,
            max_due,
            MARGIN_INTEREST_RATE,
            &LppLoanLocal::new(LOAN),
        )
        .unwrap();
        assert_eq!(Overdue::StartIn(max_due - due_period_length), overdue);
        assert!(overdue.interest().is_zero());
        assert!(overdue.margin().is_zero());
        assert_eq!(max_due - due_period_length, overdue.start_in());
    }

    #[test]
    fn due_period_equals_to_max() {
        let max_due = Duration::from_minutes(124);
        let due_period_margin = Period::from_length(Timestamp::from_seconds(100), max_due);

        let overdue = Overdue::new(
            &due_period_margin,
            max_due,
            MARGIN_INTEREST_RATE,
            &LppLoanLocal::new(LOAN),
        )
        .unwrap();
        assert_eq!(
            Overdue::Accrued {
                interest: 0.into(),
                margin: 0.into()
            },
            overdue
        );
        assert!(overdue.interest().is_zero());
        assert!(overdue.margin().is_zero());
        assert_eq!(Duration::default(), overdue.start_in());
    }

    #[test]
    fn due_period_longer_than_max() {
        let max_due = Duration::from_minutes(124);
        let due_period_length = Duration::from_days(130);
        let due_period_margin = Period::from_length(LOAN.interest_paid, due_period_length);
        let overdue_period = due_period_length - max_due;

        let lpp_loan = LppLoanLocal::new(LOAN);
        let overdue =
            Overdue::new(&due_period_margin, max_due, MARGIN_INTEREST_RATE, &lpp_loan).unwrap();
        let exp_interest = lpp_loan
            .interest_due(&(LOAN.interest_paid + due_period_length - max_due))
            .unwrap();
        let exp_margin =
            interest::interest(MARGIN_INTEREST_RATE, LOAN.principal_due, overdue_period).unwrap();
        assert_eq!(
            Overdue::Accrued {
                interest: exp_interest,
                margin: exp_margin,
            },
            overdue
        );
        assert_eq!(exp_interest, overdue.interest());
        assert_eq!(exp_margin, overdue.margin());
        assert_eq!(Duration::default(), overdue.start_in());
    }
}
