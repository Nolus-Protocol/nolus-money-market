use cosmwasm_std::{Addr, Coin, QuerierWrapper, StdResult, Storage, SubMsg, Timestamp};
use cw_storage_plus::Item;
use finance::liability::Liability;
use lpp::stub::Lpp;
use serde::{Deserialize, Serialize};

use crate::{
    bank::BankAccount,
    error::{ContractError, ContractResult},
    loan::Loan,
    msg::Denom,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lease<L> {
    customer: Addr,
    currency: Denom,
    liability: Liability,
    loan: Loan<L>,
}

impl<L> Lease<L>
where
    L: Lpp,
{
    const DB_ITEM: Item<'static, Lease<L>> = Item::new("lease");

    pub(crate) fn new(
        customer: Addr,
        currency: Denom,
        liability: Liability,
        loan: Loan<L>,
    ) -> Self {
        Self {
            customer,
            currency,
            liability,
            loan,
        }
    }

    pub(crate) fn close<B>(
        &self,
        lease: Addr,
        querier: &QuerierWrapper,
        account: B,
    ) -> ContractResult<SubMsg>
    where
        B: BankAccount,
    {
        if !self.loan.closed(querier, lease)? {
            return ContractResult::Err(ContractError::LoanNotPaid {});
        }
        let balance = account.balance(&self.currency)?;
        account
            .send(&self.customer, balance)
            .map_err(|err| err.into())
    }

    pub(crate) fn repay(
        &mut self,
        payment: Coin,
        by: Timestamp,
        querier: &QuerierWrapper,
        lease: Addr,
    ) -> ContractResult<Option<SubMsg>> {
        debug_assert_eq!(self.currency, payment.denom);
        self.loan.repay(payment, by, querier, lease)
    }

    pub(crate) fn store(self, storage: &mut dyn Storage) -> StdResult<()> {
        Lease::DB_ITEM.save(storage, &self)
    }

    pub(crate) fn load(storage: &dyn Storage) -> StdResult<Self> {
        Lease::DB_ITEM.load(storage)
    }

    pub(crate) fn owned_by(&self, addr: &Addr) -> bool {
        &self.customer == addr
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::MockStorage, Addr, QuerierWrapper, StdResult, SubMsg, Timestamp};
    use finance::{liability::Liability, percent::Percent};
    use lpp::{msg::QueryLoanResponse, stub::Lpp};
    use serde::{Deserialize, Serialize};

    use crate::loan::Loan;

    use super::Lease;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct LppLocalStub {}
    impl Lpp for LppLocalStub {
        fn open_loan_req(&self, _amount: cosmwasm_std::Coin) -> StdResult<SubMsg> {
            unimplemented!()
        }

        fn open_loan_resp(&self, _resp: cosmwasm_std::Reply) -> Result<(), String> {
            unimplemented!()
        }

        fn repay_loan_req(&self, _repayment: cosmwasm_std::Coin) -> StdResult<SubMsg> {
            todo!()
        }
        fn loan(
            &self,
            _querier: &QuerierWrapper,
            _lease: impl Into<Addr>,
        ) -> StdResult<QueryLoanResponse> {
            todo!()
        }

        fn loan_outstanding_interest(
            &self,
            _querier: &QuerierWrapper,
            _lease: impl Into<Addr>,
            _by: Timestamp,
        ) -> StdResult<lpp::msg::QueryLoanOutstandingInterestResponse> {
            todo!()
        }
    }

    #[test]
    fn persist_ok() {
        let mut storage = MockStorage::default();
        let obj = Lease {
            customer: Addr::unchecked("test"),
            currency: "UST".to_owned(),
            liability: Liability::new(
                Percent::from_percent(65),
                Percent::from_percent(5),
                Percent::from_percent(10),
                10 * 24,
            ),
            loan: Loan::open(
                Timestamp::default(),
                LppLocalStub {},
                Percent::from_percent(23),
                100,
                10,
            )
            .unwrap(),
        };
        let obj_exp = obj.clone();
        obj.store(&mut storage).expect("storing failed");
        let obj_loaded: Lease<LppLocalStub> = Lease::load(&storage).expect("loading failed");
        assert_eq!(obj_exp.customer, obj_loaded.customer);
    }
}
