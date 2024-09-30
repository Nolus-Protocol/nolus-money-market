use std::mem;

use sdk::{
    cosmwasm_std::{Addr, Storage, Timestamp},
    cw_storage_plus::Map,
};
use serde::{Deserialize, Serialize};

use finance::{coin::Coin, duration::Duration, interest, percent::Percent};
use sdk::schemars::{self, JsonSchema};

use crate::error::{ContractError, Result};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
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
    const STORAGE: Map<'static, Addr, Loan<Lpn>> = Map::new("loans");

    pub fn interest_due(&self, by: &Timestamp) -> Option<Coin<Lpn>> {
        interest::interest(
            self.annual_interest_rate,
            self.principal_due,
            self.due_period(by),
        )
    }

    pub fn repay(&mut self, by: &Timestamp, repayment: Coin<Lpn>) -> Option<RepayShares<Lpn>> {
        interest::pay(
            self.annual_interest_rate,
            self.principal_due,
            repayment,
            self.due_period(by),
        )
        .map(|(paid_for, interest_change)| {
            self.settle_repayment(interest_change, repayment, paid_for)
        })
    }

    fn due_period(&self, by: &Timestamp) -> Duration {
        Duration::between(&self.interest_paid, by.max(&self.interest_paid))
    }

    fn settle_repayment(
        &mut self,
        interest_change: Coin<Lpn>,
        repayment: Coin<Lpn>,
        paid_for: Duration,
    ) -> RepayShares<Lpn> {
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
}

impl<Lpn> Loan<Lpn> {
    pub fn open(storage: &mut dyn Storage, addr: Addr, loan: &Self) -> Result<()> {
        if Self::STORAGE.has(storage, addr.clone()) {
            return Err(ContractError::LoanExists {});
        }

        Self::STORAGE.save(storage, addr, loan).map_err(Into::into)
    }

    pub fn load(storage: &dyn Storage, addr: Addr) -> Result<Self> {
        Self::STORAGE.load(storage, addr).map_err(Into::into)
    }

    pub fn save(storage: &mut dyn Storage, addr: Addr, loan: Self) -> Result<()> {
        if loan.principal_due.is_zero() {
            Self::STORAGE.remove(storage, addr);
            Ok(())
        } else {
            Self::STORAGE
                .update(storage, addr, |loaded_loan| {
                    let mut loaded_loan = loaded_loan.ok_or(ContractError::NoLoan {})?;
                    loaded_loan.principal_due = loan.principal_due;
                    loaded_loan.interest_paid = loan.interest_paid;

                    Ok::<_, ContractError>(loaded_loan)
                })
                .map(mem::drop)
        }
    }

    pub fn query(storage: &dyn Storage, lease_addr: Addr) -> Result<Option<Loan<Lpn>>> {
        Self::STORAGE
            .may_load(storage, lease_addr)
            .map_err(Into::into)
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
            l.interest_due(&(l.interest_paid + Duration::YEAR)).unwrap()
        );

        assert_eq!(Coin::ZERO, l.interest_due(&l.interest_paid).unwrap());
        assert_eq!(
            Coin::ZERO,
            l.interest_due(&l.interest_paid.minus_nanos(1)).unwrap()
        );
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
            l.repay(&interest_paid, payment1).unwrap()
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

        let interest_a_year = interest.of(principal_start).unwrap();
        let at_first_year_end = l.interest_paid + Duration::YEAR;
        assert_eq!(
            RepayShares {
                interest: interest_a_year,
                principal: Coin::ZERO,
                excess: Coin::ZERO
            },
            l.repay(&at_first_year_end, interest_a_year).unwrap()
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

        let interest_a_year = interest.of(principal_start).unwrap();
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
                .unwrap()
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

    mod persistence {
        use currencies::Lpn;
        use finance::{coin::Coin, duration::Duration, percent::Percent, zero::Zero};
        use sdk::cosmwasm_std::{testing, Addr, Timestamp};

        use crate::{error::ContractError, loan::Loan};

        #[test]
        fn test_open_and_repay_loan() {
            let mut deps = testing::mock_dependencies();

            let mut time = Timestamp::from_nanos(0);

            let addr = Addr::unchecked("leaser");
            let loan = Loan {
                principal_due: Coin::<Lpn>::new(1000),
                annual_interest_rate: Percent::from_percent(20),
                interest_paid: time,
            };
            Loan::open(deps.as_mut().storage, addr.clone(), &loan).expect("should open loan");

            let result = Loan::open(deps.as_mut().storage, addr.clone(), &loan);
            assert_eq!(result, Err(ContractError::LoanExists {}));

            let mut loan: Loan<Lpn> =
                Loan::load(deps.as_ref().storage, addr.clone()).expect("should load loan");

            time = Timestamp::from_nanos(Duration::YEAR.nanos() / 2);
            let interest: Coin<Lpn> = loan.interest_due(&time).unwrap();
            assert_eq!(interest, 100u128.into());

            // partial repay
            let payment = loan.repay(&time, 600u128.into()).unwrap();
            assert_eq!(payment.interest, 100u128.into());
            assert_eq!(payment.principal, 500u128.into());
            assert_eq!(payment.excess, 0u128.into());

            assert_eq!(loan.principal_due, 500u128.into());
            Loan::save(deps.as_mut().storage, addr.clone(), loan).unwrap();

            let mut loan: Loan<Lpn> =
                Loan::load(deps.as_ref().storage, addr.clone()).expect("should load loan");

            // repay with excess, should close the loan
            let payment = loan.repay(&time, 600u128.into()).unwrap();
            assert_eq!(payment.interest, 0u128.into());
            assert_eq!(payment.principal, 500u128.into());
            assert_eq!(payment.excess, 100u128.into());
            assert_eq!(loan.principal_due, Coin::ZERO);
            Loan::save(deps.as_mut().storage, addr.clone(), loan).unwrap();

            // is it cleaned up?
            let is_none = Loan::<Lpn>::query(deps.as_ref().storage, addr)
                .expect("should query loan")
                .is_none();
            assert!(is_none);
        }
    }
}
