use currency::{Currency, CurrencyDef};
use finance::coin::Coin;
use platform::{
    bank::{self, BankAccount},
    batch::Batch,
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Storage};

use crate::{
    error::Result,
    loan::Loan,
    lpp::LiquidityPool,
    msg::{LoanResponse, QueryLoanResponse, QueryQuoteResponse},
};

pub(super) fn try_open_loan<Lpn>(
    mut deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    amount: Coin<Lpn>,
) -> Result<(LoanResponse<Lpn>, MessageResponse)>
where
    Lpn: 'static + CurrencyDef,
{
    let lease_addr = info.sender;
    let mut lpp = LiquidityPool::<Lpn>::load(deps.storage)?;
    lpp.validate_lease_addr(&deps.as_ref(), &lease_addr)?;

    let loan = lpp.try_open_loan(&mut deps, &env, lease_addr.clone(), amount)?;

    let messages: Batch = bank::account(&env.contract.address, deps.querier)
        .send(amount, lease_addr)
        .into();

    Ok((loan, messages.into()))
}

pub(super) fn try_repay_loan<Lpn>(
    mut deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
) -> Result<(Coin<Lpn>, MessageResponse)>
where
    Lpn: CurrencyDef,
{
    let lease_addr = info.sender;
    let repay_amount = bank::received_one(info.funds)?;

    let mut lpp = LiquidityPool::<Lpn>::load(deps.storage)?;
    lpp.validate_lease_addr(&deps.as_ref(), &lease_addr)?;
    let excess_received = lpp.try_repay_loan(&mut deps, &env, lease_addr.clone(), repay_amount)?;

    let batch = if excess_received.is_zero() {
        Batch::default()
    } else {
        bank::account(&env.contract.address, deps.querier)
            .send(excess_received, lease_addr)
            .into()
    };
    Ok((excess_received, batch.into()))
}

pub(super) fn query_quote<Lpn>(
    deps: &Deps<'_>,
    env: &Env,
    quote: Coin<Lpn>,
) -> Result<QueryQuoteResponse>
where
    Lpn: CurrencyDef,
{
    let lpp = LiquidityPool::<Lpn>::load(deps.storage)?;

    match lpp.query_quote(quote, &env.contract.address, deps.querier, &env.block.time)? {
        Some(quote) => Ok(QueryQuoteResponse::QuoteInterestRate(quote)),
        None => Ok(QueryQuoteResponse::NoLiquidity),
    }
}

pub fn query_loan<Lpn>(storage: &dyn Storage, lease_addr: Addr) -> Result<QueryLoanResponse<Lpn>>
where
    Lpn: 'static + Currency,
{
    Loan::query(storage, lease_addr)
}
