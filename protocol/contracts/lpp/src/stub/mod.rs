use std::{marker::PhantomData, result::Result as StdResult};

use serde::{Deserialize, Serialize};

use currency::{self, error::CmdError, Currency, Group, MemberOf, SymbolSlice};
use platform::batch::Batch;
use sdk::cosmwasm_std::{Addr, QuerierWrapper};

use crate::{
    error::{ContractError, Result},
    msg::{LoanResponse, LpnResponse, QueryLoanResponse, QueryMsg},
};

use self::{
    lender::{LppLenderStub, WithLppLender},
    loan::{LppLoanImpl, WithLppLoan},
};

pub mod lender;
pub mod loan;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Eq, PartialEq))]
pub struct LppRef<Lpn, Lpns> {
    addr: Addr,
    #[serde(skip)]
    _lpn: PhantomData<Lpn>,
    #[serde(skip)]
    _lpns: PhantomData<Lpns>,
}

impl<Lpn, Lpns> LppRef<Lpn, Lpns>
where
    Lpn: Currency + MemberOf<Lpns>,
    Lpns: Group,
{
    pub fn try_new(addr: Addr, querier: QuerierWrapper<'_>) -> Result<Self> {
        querier
            .query_wasm_smart(addr.clone(), &QueryMsg::<Lpns>::Lpn())
            .map_err(ContractError::from)
            .and_then(|lpn: LpnResponse| {
                currency::validate_ticker(lpn, Lpn::TICKER).map_err(Into::into)
            })
            .map(|()| Self {
                addr,
                _lpn: PhantomData,
                _lpns: PhantomData,
            })
    }

    pub fn lpn(&self) -> &SymbolSlice {
        Lpn::TICKER
    }

    pub fn addr(&self) -> &Addr {
        &self.addr
    }

    pub fn execute_loan<Cmd>(
        self,
        cmd: Cmd,
        lease: impl Into<Addr>,
        querier: QuerierWrapper<'_>,
    ) -> StdResult<Cmd::Output, Cmd::Error>
    where
        Cmd: WithLppLoan<Lpn, Lpns>,
        ContractError: Into<Cmd::Error>,
    {
        self.into_loan(lease, querier)
            .map_err(CmdError::from_api_err)
            .and_then(|lpp_loan| cmd.exec(lpp_loan).map_err(CmdError::from_customer_err))
            .map_err(CmdError::into_customer_err)
    }

    pub fn execute_lender<Cmd>(
        self,
        cmd: Cmd,
        querier: QuerierWrapper<'_>,
    ) -> StdResult<Cmd::Output, Cmd::Error>
    where
        Cmd: WithLppLender<Lpn, Lpns>,
        ContractError: Into<Cmd::Error>,
    {
        cmd.exec(self.into_lender(querier))
    }

    fn into_loan<A>(self, lease: A, querier: QuerierWrapper<'_>) -> Result<LppLoanImpl<Lpn, Lpns>>
    where
        A: Into<Addr>,
    {
        querier
            .query_wasm_smart(
                self.addr().clone(),
                &QueryMsg::<Lpns>::Loan {
                    lease_addr: lease.into(),
                },
            )
            .map_err(Into::into)
            .and_then(|may_loan: QueryLoanResponse<Lpn>| may_loan.ok_or(ContractError::NoLoan {}))
            .map(|loan: LoanResponse<Lpn>| LppLoanImpl::new(self, loan))
    }

    fn into_lender(self, querier: QuerierWrapper<'_>) -> LppLenderStub<'_, Lpn, Lpns> {
        LppLenderStub::new(self, querier)
    }
}

#[cfg(any(test, feature = "testing"))]
impl<Lpn, Lpns> LppRef<Lpn, Lpns>
where
    Lpns: Group,
{
    pub fn unchecked<A>(addr: A) -> Self
    where
        A: Into<String>,
    {
        Self {
            addr: Addr::unchecked(addr),
            _lpn: PhantomData,
            _lpns: PhantomData,
        }
    }
}

pub struct LppBatch<Ref> {
    pub lpp_ref: Ref,
    pub batch: Batch,
}
