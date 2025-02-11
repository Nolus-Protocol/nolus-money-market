use sdk::cosmwasm_std::Timestamp;
use serde::{Deserialize, Serialize};

use finance::{coin::Coin, duration::Duration, interest, percent::Percent};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(any(test, feature = "testing"), derive(Eq, PartialEq))]
#[serde(rename_all = "snake_case", bound(serialize = "", deserialize = ""))]
pub struct Loan<Lpn> {
    pub principal_due: Coin<Lpn>,
    pub annual_interest_rate: Percent,
    pub interest_paid: Timestamp,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct RepayShares<Lpn>
where
    Lpn: 'static,
{
    pub interest: Coin<Lpn>,
    pub principal: Coin<Lpn>,
    pub excess: Coin<Lpn>,
}

impl<Lpn> Loan<Lpn> {
    pub fn interest_due(&self, by: &Timestamp) -> Coin<Lpn> {
        interest::interest(
            self.annual_interest_rate,
            self.principal_due,
            self.due_period(by),
        )
    }

    pub fn repay(&mut self, by: &Timestamp, repayment: Coin<Lpn>) -> RepayShares<Lpn> {
        let (paid_for, interest_change) = interest::pay(
            self.annual_interest_rate,
            self.principal_due,
            repayment,
            self.due_period(by),
        );

        let interest_paid = repayment - interest_change;
        let principal_paid = interest_change.min(self.principal_due);
        let excess = interest_change - principal_paid;

        self.principal_due -= principal_paid;
        self.interest_paid += paid_for;

        RepayShares {
            interest: interest_paid,
            principal: principal_paid,
            excess,
        }
    }

    fn due_period(&self, by: &Timestamp) -> Duration {
        Duration::between(&self.interest_paid, by.max(&self.interest_paid))
    }
}

#[cfg(test)]
mod test {
    use currencies::Lpn;
    use finance::{
        coin::Coin, duration::Duration, fraction::Fraction, percent::Percent, zero::Zero,
    };
    use sdk::cosmwasm_std::Timestamp;

    use crate::loan::{Loan, RepayShares};

    #[test]
    fn interest() {
        let l = Loan {
            principal_due: Coin::<Lpn>::from(100),
            annual_interest_rate: Percent::from_percent(50),
            interest_paid: Timestamp::from_nanos(200),
        };

        assert_eq!(
            Coin::<Lpn>::from(50),
            l.interest_due(&(l.interest_paid + Duration::YEAR))
        );

        assert_eq!(Coin::ZERO, l.interest_due(&l.interest_paid));
        assert_eq!(Coin::ZERO, l.interest_due(&l.interest_paid.minus_nanos(1)));
    }

    #[test]
    fn repay_no_interest() {
        let principal_at_start = Coin::<Lpn>::from(500);
        let interest = Percent::from_percent(50);
        let start_at = Timestamp::from_nanos(200);
        let interest_paid = start_at;
        let mut l = Loan {
            principal_due: principal_at_start,
            annual_interest_rate: interest,
            interest_paid,
        };

        let payment1 = 10.into();
        assert_eq!(
            RepayShares {
                interest: Coin::ZERO,
                principal: payment1,
                excess: Coin::ZERO
            },
            l.repay(&interest_paid, payment1)
        );
        assert_eq!(
            Loan {
                principal_due: principal_at_start - payment1,
                annual_interest_rate: interest,
                interest_paid: l.interest_paid
            },
            l
        );
    }

    #[test]
    fn repay_interest_only() {
        let principal_start = Coin::<Lpn>::from(500);
        let interest = Percent::from_percent(50);
        let mut l = Loan {
            principal_due: principal_start,
            annual_interest_rate: interest,
            interest_paid: Timestamp::from_nanos(200),
        };

        let interest_a_year = interest.of(principal_start);
        let at_first_year_end = l.interest_paid + Duration::YEAR;
        assert_eq!(
            RepayShares {
                interest: interest_a_year,
                principal: Coin::ZERO,
                excess: Coin::ZERO
            },
            l.repay(&at_first_year_end, interest_a_year)
        );
        assert_eq!(
            Loan {
                principal_due: principal_start,
                annual_interest_rate: interest,
                interest_paid: at_first_year_end
            },
            l
        );
    }

    #[test]
    fn repay_all() {
        let principal_start = Coin::<Lpn>::from(50000000000);
        let interest = Percent::from_percent(50);
        let mut l = Loan {
            principal_due: principal_start,
            annual_interest_rate: interest,
            interest_paid: Timestamp::from_nanos(200),
        };

        let interest_a_year = interest.of(principal_start);
        let at_first_hour_end = l.interest_paid + Duration::HOUR;
        let exp_interest = interest_a_year.checked_div(365 * 24).unwrap();
        let excess = 12441.into();
        assert_eq!(
            RepayShares {
                interest: exp_interest,
                principal: principal_start,
                excess,
            },
            l.repay(&at_first_hour_end, exp_interest + principal_start + excess)
        );
        assert_eq!(
            Loan {
                principal_due: Coin::ZERO,
                annual_interest_rate: interest,
                interest_paid: at_first_hour_end
            },
            l
        );
    }
}
