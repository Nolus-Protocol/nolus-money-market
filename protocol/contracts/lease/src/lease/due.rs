use finance::{duration::Duration, interest};

use crate::{
    finance::LpnCoin,
    loan::State,
    position::{DueTrait, OverdueCollection},
};

impl DueTrait for State {
    fn total_due(&self) -> LpnCoin {
        self.principal_due + self.total_due_interest()
    }

    // If `into_slice_per_ratio` overflows (returns `None`), `Duration::MAX` is returned to indicate that
    // the time to accrue the required amount is effectively infinite, due to the very small overdue amount.
    // The method returns `None` only if the interest calculation overflows.
    fn overdue_collection(&self, min_amount: LpnCoin) -> Option<OverdueCollection> {
        let total_due_interest = self.total_due_interest();

        let time_to_accrue_min_amount = if total_due_interest >= min_amount {
            Duration::default()
        } else {
            let overdue_left = min_amount - total_due_interest;

            let total_interest_a_year = interest::interest(
                self.annual_interest + self.annual_interest_margin,
                self.principal_due,
                Duration::YEAR,
            )?;

            if total_interest_a_year.is_zero() {
                Duration::MAX
            } else {
                Duration::YEAR
                    .into_slice_per_ratio(overdue_left, total_interest_a_year)
                    .unwrap_or(Duration::MAX)
            }
        };

        let time_to_collect = self.overdue.start_in().max(time_to_accrue_min_amount);

        if time_to_collect == Duration::default() {
            Some(OverdueCollection::Overdue(total_due_interest))
        } else {
            Some(OverdueCollection::StartIn(time_to_collect))
        }
    }
}

impl State {
    fn total_due_interest(&self) -> LpnCoin {
        self.due_interest
            + self.due_margin_interest
            + self.overdue.interest()
            + self.overdue.margin()
    }
}

#[cfg(test)]
mod test {
    use finance::{coin::Coin, duration::Duration, interest, percent::Percent, zero::Zero};

    use crate::{
        loan::{Overdue, State},
        position::{DueTrait, OverdueCollection},
    };

    #[test]
    fn test_large_interest_accrual_period() {
        let principal_due = 20.into();
        let due_interest = 5.into();
        let due_margin_interest = 1.into();
        let till_due_end = Duration::from_days(1);
        let s = State {
            annual_interest: Percent::from_percent(15),
            annual_interest_margin: Percent::from_percent(0),
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::StartIn(till_due_end),
        };
        assert_eq!(
            OverdueCollection::StartIn(Duration::MAX),
            s.overdue_collection(1_800.into()).unwrap()
        );
    }

    #[test]
    fn already_above_the_limit_before_due_end() {
        let principal_due = 100_000.into();
        let due_interest = 10.into();
        let due_margin_interest = 5.into();
        let till_due_end = Duration::from_days(3);
        let s = State {
            annual_interest: Percent::from_percent(20),
            annual_interest_margin: Percent::from_percent(5),
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::StartIn(till_due_end),
        };
        let overdue_collection = s
            .overdue_collection(due_interest + due_margin_interest - 1.into())
            .unwrap();
        assert_eq!(till_due_end, overdue_collection.start_in());
        assert_eq!(Coin::ZERO, overdue_collection.amount());
        assert_eq!(
            principal_due + due_interest + due_margin_interest,
            s.total_due()
        );
    }

    #[test]
    fn get_to_limit_before_due_end() {
        let annual_interest = Percent::from_percent(20);
        let annual_interest_margin = Percent::from_percent(5);
        let principal_due = 100_000.into();
        let due_interest = 10.into();
        let due_margin_interest = 5.into();
        let till_due_end = Duration::from_days(3);
        let delta_to_due_end = interest::interest(
            annual_interest + annual_interest_margin,
            principal_due,
            till_due_end,
        )
        .unwrap();
        let s = State {
            annual_interest,
            annual_interest_margin,
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::StartIn(till_due_end),
        };
        let overdue_collection = s
            .overdue_collection(due_interest + due_margin_interest + delta_to_due_end - 1.into())
            .unwrap();
        assert_eq!(till_due_end, overdue_collection.start_in());
        assert_eq!(Coin::ZERO, overdue_collection.amount());
        assert_eq!(
            principal_due + due_interest + due_margin_interest,
            s.total_due()
        );
    }

    #[test]
    fn below_the_limit_past_due_end() {
        let annual_interest = Percent::from_percent(20);
        let annual_interest_margin = Percent::from_percent(5);
        let principal_due = 100_000.into();
        let due_interest = 15.into();
        let due_margin_interest = 5.into();
        let overdue_interest = 7.into();
        let overdue_margin_interest = 2.into();
        let total_interest =
            due_interest + due_margin_interest + overdue_interest + overdue_margin_interest;

        let delta_to_overdue = 40.into();
        let till_overdue = Duration::YEAR
            .into_slice_per_ratio(
                delta_to_overdue,
                interest::interest(
                    annual_interest + annual_interest_margin,
                    principal_due,
                    Duration::YEAR,
                )
                .unwrap(),
            )
            .unwrap();

        let s = State {
            annual_interest,
            annual_interest_margin,
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::Accrued {
                interest: overdue_interest,
                margin: overdue_margin_interest,
            },
        };
        let overdue_collection = s
            .overdue_collection(total_interest + delta_to_overdue)
            .unwrap();
        assert_eq!(till_overdue, overdue_collection.start_in());
        assert_eq!(Coin::ZERO, overdue_collection.amount());
        assert_eq!(principal_due + total_interest, s.total_due());
    }

    #[test]
    fn above_the_limit_past_due_end() {
        let annual_interest = Percent::from_percent(20);
        let annual_interest_margin = Percent::from_percent(5);
        let principal_due = 100_000.into();
        let due_interest = 15.into();
        let due_margin_interest = 5.into();
        let overdue_interest = 7.into();
        let overdue_margin_interest = 2.into();
        let total_interest =
            due_interest + due_margin_interest + overdue_interest + overdue_margin_interest;

        let s = State {
            annual_interest,
            annual_interest_margin,
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::Accrued {
                interest: overdue_interest,
                margin: overdue_margin_interest,
            },
        };
        let overdue_collection = s.overdue_collection(total_interest - 1.into()).unwrap();
        assert_eq!(Duration::default(), overdue_collection.start_in());
        assert_eq!(total_interest, overdue_collection.amount());
        assert_eq!(principal_due + total_interest, s.total_due());
    }

    #[test]
    fn fully_paid_lease_no_collectable_overdue() {
        let principal_due = Coin::ZERO;
        let due_interest = Coin::ZERO;
        let due_margin_interest = Coin::ZERO;
        let overdue_interest = Coin::ZERO;
        let overdue_margin_interest = Coin::ZERO;
        let total_interest =
            due_interest + due_margin_interest + overdue_interest + overdue_margin_interest;

        let overdue_start_in = Duration::from_days(6);
        let s = State {
            annual_interest: Percent::from_percent(20),
            annual_interest_margin: Percent::from_percent(5),
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::StartIn(overdue_start_in),
        };
        let overdue_collection = s.overdue_collection(100.into()).unwrap();
        assert_eq!(Duration::MAX, overdue_collection.start_in());
        assert_eq!(Coin::ZERO, overdue_collection.amount());
        assert_eq!(principal_due + total_interest, s.total_due());
    }

    #[test]
    fn fully_paid_lease_with_collectable_overdue() {
        let principal_due = Coin::ZERO;
        let due_interest = Coin::ZERO;
        let due_margin_interest = Coin::ZERO;
        let overdue_interest = Coin::ZERO;
        let overdue_margin_interest = Coin::ZERO;
        let total_interest =
            due_interest + due_margin_interest + overdue_interest + overdue_margin_interest;

        let s = State {
            annual_interest: Percent::from_percent(20),
            annual_interest_margin: Percent::from_percent(5),
            principal_due,
            due_interest,
            due_margin_interest,
            overdue: Overdue::Accrued {
                interest: overdue_interest,
                margin: overdue_margin_interest,
            },
        };
        let overdue_collection = s.overdue_collection(100.into()).unwrap();
        assert_eq!(Duration::MAX, overdue_collection.start_in());
        assert_eq!(Coin::ZERO, overdue_collection.amount());
        assert_eq!(principal_due + total_interest, s.total_due());
    }
}
