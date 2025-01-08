use std::ops::DerefMut as _;

use currency::CurrencyDef;
use finance::coin::{Coin, CoinDTO};
use oracle::stub::convert;
use oracle_platform::OracleRef;
use serde::Serialize;

use access_control::SingleUserAccess;
use currencies::{
    Lpn as LpnCurrency, Lpns as LpnCurrencies, PaymentGroup, Stable as StableCurrency,
};

use platform::{
    contract::Code, error as platform_error, message::Response as PlatformResponse, response,
};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, QuerierWrapper},
};
use versioning::{package_version, SemVer, Version, VersionSegment};

use crate::{
    error::{ContractError, Result},
    lpp::{LiquidityPool, LppBalances},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SudoMsg},
    state::Config,
};

mod borrow;
mod lender;
mod rewards;

// const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 1;
const CONTRACT_STORAGE_VERSION: VersionSegment = 2;
const PACKAGE_VERSION: SemVer = package_version!();
const CONTRACT_VERSION: Version = Version::new(CONTRACT_STORAGE_VERSION, PACKAGE_VERSION);

#[entry_point]
pub fn instantiate(
    mut deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<CwResponse> {
    deps.api.addr_validate(msg.lease_code_admin.as_str())?;

    versioning::initialize(deps.storage, CONTRACT_VERSION)?;

    SingleUserAccess::new(
        deps.storage.deref_mut(),
        crate::access_control::LEASE_CODE_ADMIN_KEY,
    )
    .grant_to(&msg.lease_code_admin)?;

    Code::try_new(msg.lease_code.into(), &deps.querier)
        .map_err(Into::into)
        .and_then(|lease_code| {
            LiquidityPool::<LpnCurrency>::store(
                deps.storage,
                Config::new::<LpnCurrency>(msg, lease_code),
            )
        })
        .map(|()| response::empty_response())
        .inspect_err(platform_error::log(deps.api))
}

#[entry_point]
pub fn migrate(deps: DepsMut<'_>, _env: Env, MigrateMsg {}: MigrateMsg) -> Result<CwResponse> {
    versioning::update_legacy_software(deps.storage, CONTRACT_VERSION, Into::into)
        .and_then(response::response)
        .inspect_err(platform_error::log(deps.api))
}

#[entry_point]
pub fn execute(
    mut deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<LpnCurrencies>,
) -> Result<CwResponse> {
    let api = deps.api;
    match msg {
        ExecuteMsg::NewLeaseCode {
            lease_code: new_lease_code,
        } => {
            SingleUserAccess::new(
                deps.storage.deref_mut(),
                crate::access_control::LEASE_CODE_ADMIN_KEY,
            )
            .check(&info.sender)?;

            Config::update_lease_code(deps.storage, new_lease_code)
                .map(|()| PlatformResponse::default())
                .map(response::response_only_messages)
        }
        ExecuteMsg::DistributeRewards() => {
            rewards::try_distribute_rewards(deps, info).map(response::response_only_messages)
        }
        ExecuteMsg::ClaimRewards { other_recipient } => {
            rewards::try_claim_rewards(deps, env, info, other_recipient)
                .map(response::response_only_messages)
        }
        ExecuteMsg::OpenLoan { amount } => amount
            .try_into()
            .map_err(Into::into)
            .and_then(|amount_lpn| {
                borrow::try_open_loan::<LpnCurrency>(deps, env, info, amount_lpn)
            })
            .and_then(|(loan_resp, message_response)| {
                response::response_with_messages::<_, _, ContractError>(loan_resp, message_response)
            }),
        ExecuteMsg::RepayLoan() => borrow::try_repay_loan::<LpnCurrency>(deps, env, info).and_then(
            |(excess_amount, message_response)| {
                response::response_with_messages::<_, _, ContractError>(
                    excess_amount,
                    message_response,
                )
            },
        ),
        ExecuteMsg::Deposit() => lender::try_deposit::<LpnCurrency>(deps, env, info)
            .map(response::response_only_messages),
        ExecuteMsg::Burn { amount } => lender::try_withdraw::<LpnCurrency>(deps, env, info, amount)
            .map(response::response_only_messages),
    }
    .inspect_err(platform_error::log(api))
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, _env: Env, msg: SudoMsg) -> Result<CwResponse> {
    // no currency context variants
    match msg {
        SudoMsg::NewBorrowRate { borrow_rate } => {
            Config::update_borrow_rate(deps.storage, borrow_rate)
        }
        SudoMsg::MinUtilization { min_utilization } => {
            Config::update_min_utilization(deps.storage, min_utilization)
        }
    }
    .map(|()| PlatformResponse::default())
    .map(response::response_only_messages)
    .inspect_err(platform_error::log(deps.api))
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg<LpnCurrencies>) -> Result<Binary> {
    match msg {
        QueryMsg::Config() => Config::load(deps.storage).and_then(|ref resp| to_json_binary(resp)),
        QueryMsg::Lpn() => to_json_binary(LpnCurrency::definition().dto()),
        QueryMsg::Balance { address } => {
            lender::query_balance(deps.storage, address).and_then(|ref resp| to_json_binary(resp))
        }
        QueryMsg::Rewards { address } => {
            rewards::query_rewards(deps.storage, address).and_then(|ref resp| to_json_binary(resp))
        }
        QueryMsg::Quote { amount } => amount
            .try_into()
            .map_err(Into::into)
            .and_then(|quote| borrow::query_quote::<LpnCurrency>(&deps, &env, quote))
            .and_then(|ref resp| to_json_binary(resp)),
        QueryMsg::Loan { lease_addr } => {
            borrow::query_loan::<LpnCurrency>(deps.storage, lease_addr)
                .and_then(|ref resp| to_json_binary(resp))
        }
        QueryMsg::LppBalance() => rewards::query_lpp_balance::<LpnCurrency>(deps, env)
            .and_then(|lpp_balances| {
                rewards::query_total_rewards(deps.storage)
                    .map(|total_rewards| lpp_balances.into_response(total_rewards))
            })
            .and_then(|ref resp| to_json_binary(resp)),
        QueryMsg::StableBalance { oracle_addr } => {
            rewards::query_lpp_balance::<LpnCurrency>(deps, env)
                .map(LppBalances::into_total)
                .and_then(|total| {
                    OracleRef::try_from_base(oracle_addr, deps.querier)
                        .map_err(ContractError::InvalidOracleBaseCurrency)
                        .and_then(|oracle_ref| to_stable(oracle_ref, total, deps.querier))
                })
                .map(CoinDTO::<PaymentGroup>::from)
                .and_then(|ref resp| to_json_binary(resp))
        }
        QueryMsg::Price() => lender::query_ntoken_price::<LpnCurrency>(deps, env)
            .and_then(|ref resp| to_json_binary(resp)),
        QueryMsg::DepositCapacity() => {
            to_json_binary(&lender::deposit_capacity::<LpnCurrency>(deps, env)?)
        }
    }
    .inspect_err(platform_error::log(deps.api))
}

fn to_json_binary<T>(data: &T) -> Result<Binary>
where
    T: Serialize,
{
    cosmwasm_std::to_json_binary(data).map_err(ContractError::ConvertToBinary)
}

fn to_stable(
    oracle: OracleRef<LpnCurrency, LpnCurrencies>,
    total: Coin<LpnCurrency>,
    querier: QuerierWrapper<'_>,
) -> Result<Coin<StableCurrency>> {
    convert::from_quote::<_, LpnCurrencies, StableCurrency, PaymentGroup>(oracle, total, querier)
        .map_err(ContractError::ConvertFromQuote)
}

#[cfg(test)]
mod test {
    use currencies::Lpn;
    use finance::coin::Coin;
    use platform::coin_legacy;
    use sdk::cosmwasm_std::{Addr, Coin as CwCoin, MessageInfo};

    pub(super) type TheCurrency = Lpn;

    pub(super) fn lender() -> Addr {
        const LENDER: &str = "lender";

        Addr::unchecked(LENDER)
    }

    pub(super) fn lender_msg_no_funds() -> MessageInfo {
        MessageInfo {
            sender: lender(),
            funds: vec![],
        }
    }

    pub(super) fn lender_msg_with_funds<F>(funds: F) -> MessageInfo
    where
        F: Into<Coin<TheCurrency>>,
    {
        MessageInfo {
            sender: lender(),
            funds: vec![cwcoin(funds)],
        }
    }

    pub(super) fn cwcoin<A>(amount: A) -> CwCoin
    where
        A: Into<Coin<TheCurrency>>,
    {
        coin_legacy::to_cosmwasm::<TheCurrency>(amount.into())
    }
}
