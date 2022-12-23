use serde::{Deserialize, Serialize};

use crate::state::{
    config::Config,
    supported_pairs::{SubTree, SwapLeg, TreeStore},
};
use finance::currency::SymbolOwned;
use marketprice::config::Config as PriceConfig;
use marketprice::{alarms::Alarm, SpotPrice};
use sdk::{
    cosmwasm_std::Addr,
    schemars::{self, JsonSchema},
};

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
pub struct InstantiateMsg {
    pub config: Config,
    #[schemars(with = "Vec<SubTree>")]
    pub swap_tree: TreeStore,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    RegisterFeeder {
        feeder_address: String,
    },
    RemoveFeeder {
        feeder_address: String,
    },
    FeedPrices {
        prices: Vec<SpotPrice>,
    },
    UpdateConfig(PriceConfig),
    SwapTree {
        #[schemars(with = "Vec<SubTree>")]
        tree: TreeStore,
    },
    AddPriceAlarm {
        alarm: Alarm,
    },
    RemovePriceAlarm {},
    /// Returns [`Status`] as response data.
    DispatchAlarms {
        max_count: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // returns the contract configuration
    Config {},
    // returns the supported currencies tree
    SwapTree {},
    // returns all registered feeders
    Feeders {},
    // check if an address belongs to a registered feeder
    IsFeeder {
        address: Addr,
    },
    // returns the price of the denom against the base asset
    Prices {
        currencies: Vec<SymbolOwned>,
    },
    Price {
        currency: SymbolOwned,
    },
    // returns a list of supported denom pairs
    SupportedCurrencyPairs {},
    SwapPath {
        from: SymbolOwned,
        to: SymbolOwned,
    },
    /// Returns [`Status`] as response data.
    AlarmsStatus {},
}

pub type SupportedCurrencyPairsResponse = Vec<SwapLeg>;

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
pub struct ConfigResponse {
    pub owner: Addr,
    pub config: Config,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct SwapTreeResponse {
    #[schemars(with = "Vec<SubTree>")]
    pub tree: TreeStore,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct PricesResponse {
    pub prices: Vec<SpotPrice>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteAlarmMsg {
    PriceAlarm(),
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct DispatchAlarmsResponse(pub u32);

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AlarmsStatusResponse {
    pub remaining_alarms: bool,
}
