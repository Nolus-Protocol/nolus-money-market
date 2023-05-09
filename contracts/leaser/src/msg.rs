use serde::{Deserialize, Serialize};

use finance::{currency::SymbolOwned, liability::Liability, percent::Percent};
use lease::api::{ConnectionParams, DownpaymentCoin, InterestPaymentSpec, LeaseCoin};
use lpp::msg::LpnCoin;
use sdk::{
    cosmwasm_std::{Addr, Uint64},
    schemars::{self, JsonSchema},
};

use crate::state::config::Config;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub lease_code_id: Uint64,
    pub lpp_ust_addr: Addr,
    pub lease_interest_rate_margin: Percent,
    pub liability: Liability,
    pub lease_interest_payment: InterestPaymentSpec,
    pub time_alarms: Addr,
    pub market_price_oracle: Addr,
    pub profit: Addr,
}

#[derive(Serialize, Deserialize)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    OpenLease {
        currency: SymbolOwned,
        #[serde(default)]
        max_ltd: Option<Percent>,
    },
    MigrateLeases {
        new_code_id: Uint64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SudoMsg {
    SetupDex(ConnectionParams),
    Config {
        lease_interest_rate_margin: Percent,
        liability: Liability,
        lease_interest_payment: InterestPaymentSpec,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Quote {
        downpayment: DownpaymentCoin,
        lease_asset: SymbolOwned,
        #[serde(default)]
        max_ltd: Option<Percent>,
    },
    Leases {
        owner: Addr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ConfigResponse {
    pub config: Config,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Clone, Debug))]
pub struct QuoteResponse {
    pub total: LeaseCoin,
    pub borrow: LpnCoin,
    pub annual_interest_rate: Percent,
    pub annual_interest_rate_margin: Percent,
}
