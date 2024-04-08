use platform::contract::Code;
use serde::{Deserialize, Serialize};

pub use currency::SymbolOwned as LpnResponse;
use currency::{Currency, Group, NlsPlatform, SymbolOwned};
use finance::{
    coin::{Coin, CoinDTO},
    percent::{bound::BoundToHundredPercent, Percent},
    price::Price,
};
use lpp_platform::NLpn;
use sdk::{
    cosmwasm_std::{Addr, Uint128, Uint64},
    schemars::{self, JsonSchema},
};

use crate::{borrow::InterestRate, loan::Loan};

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub lpn_ticker: SymbolOwned,
    pub lease_code_admin: Addr,
    // Since this is an external system API we should not use [Code].
    pub lease_code: Uint64,
    pub borrow_rate: InterestRate,
    pub min_utilization: BoundToHundredPercent,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub enum ExecuteMsg<Lpns>
where
    Lpns: Group,
{
    NewLeaseCode {
        // This is an internal system API and we use [Code]
        lease_code: Code,
    },

    OpenLoan {
        amount: CoinDTO<Lpns>,
    },
    RepayLoan(),

    Deposit(),
    // CW20 interface, withdraw from lender deposit
    Burn {
        amount: Uint128,
    },

    /// Implementation of lpp_platform::msg::ExecuteMsg::DistributeRewards
    DistributeRewards(),
    ClaimRewards {
        other_recipient: Option<Addr>,
    },
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SudoMsg {
    NewBorrowRate {
        borrow_rate: InterestRate,
    },
    MinUtilization {
        min_utilization: BoundToHundredPercent,
    },
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub enum QueryMsg<Lpns>
where
    Lpns: Group,
{
    Config(),
    /// Return [LpnResponse]
    Lpn(),
    Quote {
        amount: CoinDTO<Lpns>,
    },
    Loan {
        lease_addr: Addr,
    },
    // Deposit
    /// CW20 interface, lender deposit balance
    Balance {
        address: Addr,
    },
    /// Return the pool's total balance in Lpn [LppBalanceResponse]
    LppBalance(),
    /// Implementation of lpp_platform::msg::QueryMsg::LppBalance
    /// TODO
    Price(),
    DepositCapacity(),

    Rewards {
        address: Addr,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryQuoteResponse {
    QuoteInterestRate(Percent),
    NoLiquidity,
}

pub type LoanResponse<Lpn> = Loan<Lpn>;

pub type QueryLoanResponse<Lpn> = Option<LoanResponse<Lpn>>;

// Deposit query responses

// CW20 interface
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub struct LppBalanceResponse<Lpns>
where
    Lpns: Group,
{
    pub balance: CoinDTO<Lpns>,
    pub total_principal_due: CoinDTO<Lpns>,
    pub total_interest_due: CoinDTO<Lpns>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PriceResponse<Lpn>(pub Price<NLpn, Lpn>)
where
    Lpn: 'static + Currency;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RewardsResponse {
    pub rewards: Coin<NlsPlatform>,
}
