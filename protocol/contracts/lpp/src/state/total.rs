use serde::{Deserialize, Serialize};

use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    error::Error as FinanceError,
    fraction::Fraction,
    interest,
    percent::Percent,
    ratio::Rational,
    zero::Zero,
};
use sdk::{
    cosmwasm_std::{StdResult, Storage, Timestamp},
    cw_storage_plus::Item,
    schemars::{self, JsonSchema},
};

use crate::error::{ContractError, Result};

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
#[serde(bound(serialize = "", deserialize = ""))]
pub struct Total<Lpn> {
    total_principal_due: Coin<Lpn>,
    total_interest_due: Coin<Lpn>,
    annual_interest_rate: Rational<Coin<Lpn>>,
    last_update_time: Timestamp,
}

impl<Lpn> Default for Total<Lpn> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Lpn> Total<Lpn> {
    const STORAGE: Item<'static, Total<Lpn>> = Item::new("total");

    pub fn new() -> Self {
        Total {
            total_principal_due: Coin::ZERO,
            total_interest_due: Coin::ZERO,
            annual_interest_rate: zero_interest_rate(),
            last_update_time: Timestamp::default(),
        }
    }

    pub fn total_principal_due(&self) -> Coin<Lpn> {
        self.total_principal_due
    }

    pub fn store(&self, storage: &mut dyn Storage) -> StdResult<()> {
        Self::STORAGE.save(storage, self)
    }

    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        Self::STORAGE.load(storage)
    }

    pub fn total_interest_due_by_now(&self, ctime: &Timestamp) -> Option<Coin<Lpn>> {
        interest::interest::<Coin<Lpn>, _, _>(
            self.annual_interest_rate,
            self.total_principal_due,
            Duration::between(&self.last_update_time, ctime),
        )
        .map(|interest| interest + self.total_interest_due)
    }

    pub fn borrow(
        &mut self,
        ctime: Timestamp,
        amount: Coin<Lpn>,
        loan_interest_rate: Percent,
    ) -> Result<&Self> {
        self.total_interest_due =
            self.total_interest_due_by_now(&ctime)
                .ok_or(ContractError::Finance(FinanceError::Overflow(format!(
                    "Oveflow while calculating the total interest due by now: {:?}",
                    &ctime
                ))))?;

        let new_total_principal_due =
            self.total_principal_due
                .checked_add(amount)
                .ok_or(ContractError::Finance(FinanceError::overflow_err(
                    "while calculating the total principal due",
                    self.total_principal_due,
                    amount,
                )))?;

        // TODO: get rid of fully qualified syntax
        let new_annual_interest =
            Fraction::<Coin<Lpn>>::of(&self.annual_interest_rate, self.total_principal_due)
                .ok_or(ContractError::Finance(FinanceError::overflow_err(
                    "in fraction calculation",
                    self.annual_interest_rate,
                    self.total_principal_due,
                )))
                .and_then(|current_annual_interest| {
                    loan_interest_rate
                        .of(amount)
                        .ok_or(ContractError::Finance(FinanceError::overflow_err(
                            "in fraction calculation",
                            loan_interest_rate,
                            amount,
                        )))
                        .and_then(|loan_interest| {
                            current_annual_interest.checked_add(loan_interest).ok_or(
                                ContractError::Finance(FinanceError::overflow_err(
                                    "while calculating the annual interest",
                                    current_annual_interest,
                                    loan_interest,
                                )),
                            )
                        })
                })?;

        self.annual_interest_rate = Rational::new(new_annual_interest, new_total_principal_due);

        self.total_principal_due = new_total_principal_due;

        self.last_update_time = ctime;

        Ok(self)
    }

    pub fn repay(
        &mut self,
        ctime: Timestamp,
        loan_interest_payment: Coin<Lpn>,
        loan_principal_payment: Coin<Lpn>,
        loan_interest_rate: Percent,
    ) -> Option<&Self> {
        // The interest payment calculation of loans is the source of truth.
        // Therefore, it is possible for the rounded-down total interest due from `total_interest_due_by_now`
        // to become less than the sum of loans' interests. Taking 0 when subtracting a loan's interest from the total is a safe solution.

        self.total_interest_due =
            self.total_interest_due_by_now(&ctime)
                .map(|total_interst_due_by_now| {
                    total_interst_due_by_now.saturating_sub(loan_interest_payment)
                })?;

        let new_total_principal_due = self
            .total_principal_due
            .checked_sub(loan_principal_payment)?;

        self.annual_interest_rate = if new_total_principal_due.is_zero() {
            Some(zero_interest_rate())
        } else {
            // Please refer to the comment above for more detailed information on why using `saturating_sub` is a safe solution
            // for updating the annual interest
            Fraction::<Coin<Lpn>>::of(&self.annual_interest_rate, self.total_principal_due)
                .and_then(|annual_interest| {
                    loan_interest_rate
                        .of(loan_principal_payment)
                        .map(|loan_interest| {
                            Rational::new(
                                annual_interest.saturating_sub(loan_interest),
                                new_total_principal_due,
                            )
                        })
                })
        }?;

        self.total_principal_due = new_total_principal_due;
        self.last_update_time = ctime;

        Some(self)
    }
}

fn zero_interest_rate<Lpn>() -> Rational<Coin<Lpn>> {
    const THOUSAND: Amount = 1000;
    Rational::new(Coin::ZERO, THOUSAND.into())
}

#[cfg(test)]
mod test {
    use currencies::Lpn;
    use finance::duration::Duration;
    use sdk::cosmwasm_std::testing;

    use crate::loan::Loan;

    use super::*;

    #[test]
    fn borrow_and_repay() {
        let mut deps = testing::mock_dependencies();
        let mut block_time = Timestamp::from_nanos(1_571_797_419_879_305_533);

        let total: Total<Lpn> = Total::default();
        total.store(deps.as_mut().storage).expect("should store");

        let mut total: Total<Lpn> = Total::load(deps.as_ref().storage).expect("should load");

        assert_eq!(total.total_principal_due(), Coin::<Lpn>::new(0));

        total
            .borrow(block_time, Coin::new(10000), Percent::from_percent(20))
            .expect("should borrow");
        assert_eq!(total.total_principal_due(), Coin::new(10000));

        block_time = block_time.plus_nanos(Duration::YEAR.nanos() / 2);
        let interest_due = total.total_interest_due_by_now(&block_time).unwrap();
        assert_eq!(interest_due, Coin::new(1000));

        total
            .repay(
                block_time,
                Coin::new(1000),
                Coin::new(5000),
                Percent::from_percent(20),
            )
            .unwrap();
        assert_eq!(total.total_principal_due(), Coin::new(5000));

        block_time = block_time.plus_nanos(Duration::YEAR.nanos() / 2);
        let interest_due = total.total_interest_due_by_now(&block_time).unwrap();
        assert_eq!(interest_due, 500u128.into());
    }

    #[test]
    fn borrow_and_repay_with_overflow() {
        let mut block_time = Timestamp::from_nanos(0);

        let mut total: Total<Lpn> = Total::default();
        assert_eq!(total.total_principal_due(), Coin::<Lpn>::new(0));

        let borrow_loan1 = Coin::<Lpn>::new(5_458_329);
        let loan1_annual_interest_rate = Percent::from_permille(137);
        let loan1 = Loan {
            principal_due: borrow_loan1,
            annual_interest_rate: loan1_annual_interest_rate,
            interest_paid: block_time,
        };

        total
            .borrow(block_time, borrow_loan1, loan1_annual_interest_rate)
            .unwrap();
        assert_eq!(total.total_principal_due(), borrow_loan1);
        assert_eq!(
            total.total_interest_due_by_now(&block_time).unwrap(),
            Coin::ZERO
        );

        block_time = block_time.plus_days(59);

        // Open loan2 after 59 days
        let borrow_loan2 = Coin::<Lpn>::new(3_543_118);
        let loan2_annual_interest_rate = Percent::from_permille(133);
        let loan2 = Loan {
            principal_due: borrow_loan2,
            annual_interest_rate: loan2_annual_interest_rate,
            interest_paid: block_time,
        };

        let total_interest_due = total.total_interest_due_by_now(&block_time).unwrap();
        assert_eq!(total_interest_due, loan1.interest_due(&block_time).unwrap());

        total
            .borrow(block_time, borrow_loan2, loan2_annual_interest_rate)
            .unwrap();
        assert_eq!(total.total_principal_due(), borrow_loan1 + borrow_loan2);
        assert_eq!(
            total.total_interest_due_by_now(&block_time).unwrap(),
            total_interest_due
        );

        block_time = block_time.plus_days(147);

        // Fully repay loan1 after 147 days
        total
            .repay(
                block_time,
                loan1.interest_due(&block_time).unwrap(),
                loan1.principal_due,
                loan1.annual_interest_rate,
            )
            .unwrap();
        assert_eq!(total.total_principal_due(), borrow_loan2);

        block_time = block_time.plus_days(67);

        // Fully repay loan2 after 67 days
        total
            .repay(
                block_time,
                loan2.interest_due(&block_time).unwrap(),
                loan2.principal_due,
                loan2.annual_interest_rate,
            )
            .unwrap();

        assert!(total.total_interest_due.is_zero());
        assert!(total.total_principal_due.is_zero());
    }
}
