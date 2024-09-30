use std::{marker::PhantomData, result::Result as StdResult};

use currency::{CurrencyDef, Group, MemberOf};
use finance::{coin::Coin, percent::Percent};
use platform::batch::Batch;
use sdk::cosmwasm_std::Timestamp;

use crate::{
    error::ContractError,
    loan::{Loan, RepayShares},
    msg::ExecuteMsg,
};

use super::{LppBatch, LppRef};

pub trait LppLoan<Lpn, Lpns>
where
    Lpns: Group,
    Self: TryInto<LppBatch<LppRef<Lpn, Lpns>>, Error = ContractError>,
{
    fn principal_due(&self) -> Coin<Lpn>;
    fn interest_due(&self, by: &Timestamp) -> Option<Coin<Lpn>>;
    /// Repay the due interest and principal by the specified time
    ///
    /// First, the provided 'repayment' is used to repay the due interest,
    /// and then, if there is any remaining amount, to repay the principal.
    /// Amount 0 is acceptable although does not change the loan.
    fn repay(&mut self, by: &Timestamp, repayment: Coin<Lpn>) -> Option<RepayShares<Lpn>>;
    fn annual_interest_rate(&self) -> Percent;
}

pub trait WithLppLoan<Lpn, Lpns>
where
    Lpns: Group,
{
    type Output;
    type Error;

    fn exec<Loan>(self, loan: Loan) -> StdResult<Self::Output, Self::Error>
    where
        Loan: LppLoan<Lpn, Lpns>;
}

pub(super) struct LppLoanImpl<Lpn, Lpns> {
    lpp_ref: LppRef<Lpn, Lpns>,
    lpn: PhantomData<Lpn>,
    loan: Loan<Lpn>,
    repayment: Coin<Lpn>,
}

impl<Lpn, Lpns> LppLoanImpl<Lpn, Lpns>
where
    Lpns: Group,
{
    pub(super) fn new(lpp_ref: LppRef<Lpn, Lpns>, loan: Loan<Lpn>) -> Self {
        Self {
            lpp_ref,
            lpn: PhantomData,
            loan,
            repayment: Default::default(),
        }
    }
}
impl<Lpn, Lpns> LppLoan<Lpn, Lpns> for LppLoanImpl<Lpn, Lpns>
where
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns>,
    Lpns: Group,
{
    fn principal_due(&self) -> Coin<Lpn> {
        self.loan.principal_due
    }

    fn interest_due(&self, by: &Timestamp) -> Option<Coin<Lpn>> {
        self.loan.interest_due(by)
    }

    fn repay(&mut self, by: &Timestamp, repayment: Coin<Lpn>) -> Option<RepayShares<Lpn>> {
        self.repayment += repayment;
        self.loan.repay(by, repayment)
    }

    fn annual_interest_rate(&self) -> Percent {
        self.loan.annual_interest_rate
    }
}

impl<Lpn, Lpns> TryFrom<LppLoanImpl<Lpn, Lpns>> for LppBatch<LppRef<Lpn, Lpns>>
where
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns>,
    Lpns: Group,
{
    type Error = ContractError;

    fn try_from(stub: LppLoanImpl<Lpn, Lpns>) -> StdResult<Self, Self::Error> {
        let mut batch = Batch::default();
        if !stub.repayment.is_zero() {
            batch.schedule_execute_wasm_no_reply(
                stub.lpp_ref.addr().clone(),
                &ExecuteMsg::<Lpns>::RepayLoan(),
                Some(stub.repayment),
            )?;
        }
        Ok(Self {
            lpp_ref: stub.lpp_ref,
            batch,
        })
    }
}

#[cfg(test)]
mod test {
    use currencies::{Lpn, Lpns};
    use finance::{coin::Coin, duration::Duration, percent::Percent, zero::Zero};
    use platform::batch::Batch;
    use sdk::cosmwasm_std::Timestamp;

    use crate::{
        loan::Loan,
        msg::ExecuteMsg,
        stub::{loan::LppLoan, LppBatch, LppRef},
    };

    use super::LppLoanImpl;

    #[test]
    fn try_from_no_payments() {
        let lpp_ref = LppRef::<Lpn, _>::unchecked("lpp_address");
        let start = Timestamp::from_seconds(10);
        let mut loan = LppLoanImpl::new(
            lpp_ref.clone(),
            Loan {
                principal_due: Coin::<Lpn>::new(100),
                annual_interest_rate: Percent::from_percent(12),
                interest_paid: start,
            },
        );
        loan.repay(&(start + Duration::YEAR), Coin::ZERO).unwrap();
        let batch: LppBatch<LppRef<Lpn, Lpns>> = loan.try_into().unwrap();

        assert_eq!(lpp_ref, batch.lpp_ref);
        assert_eq!(Batch::default(), batch.batch);
    }

    #[test]
    fn try_from_a_few_payments() {
        let lpp_ref = LppRef::<Lpn, _>::unchecked("lpp_address");
        let start = Timestamp::from_seconds(0);
        let mut loan = LppLoanImpl::new(
            lpp_ref.clone(),
            Loan {
                principal_due: Coin::<Lpn>::new(100),
                annual_interest_rate: Percent::from_percent(12),
                interest_paid: start,
            },
        );
        let payment1 = 8.into();
        let payment2 = 4.into();
        loan.repay(&(start + Duration::YEAR), payment1).unwrap();
        loan.repay(&(start + Duration::YEAR), payment2).unwrap();
        let batch: LppBatch<LppRef<Lpn, Lpns>> = loan.try_into().unwrap();

        assert_eq!(lpp_ref, batch.lpp_ref);
        {
            let mut exp = Batch::default();
            exp.schedule_execute_wasm_no_reply(
                lpp_ref.addr().clone(),
                &ExecuteMsg::<Lpns>::RepayLoan(),
                Some(payment1 + payment2),
            )
            .unwrap();
            assert_eq!(exp, batch.batch);
        }
    }
}
