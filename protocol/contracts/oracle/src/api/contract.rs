use serde::{Deserialize, Deserializer, Serialize, Serializer};

use currency::{CurrencyDTO, CurrencyDef, Definition, Group, MemberOf, SymbolOwned};
use finance::price::{base::BasePrice, dto::PriceDTO};
use marketprice::config::Config as PriceConfig;
use sdk::{
    cosmwasm_std::Addr,
    schemars::{self, JsonSchema},
};
use tree::HumanReadableTree;
use versioning::ProtocolPackageReleaseId;

pub use super::alarms::Alarm;
use super::swap::SwapTarget;

pub type AlarmsCount = platform::dispatcher::AlarmsCount;

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg<PriceCurrencies>
where
    PriceCurrencies: Group,
{
    pub config: Config,
    pub swap_tree: HumanReadableTree<SwapTarget<PriceCurrencies>>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {
    pub to_release: ProtocolPackageReleaseId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub enum ExecuteMsg<BaseCurrency, BaseCurrencies, AlarmCurrencies, PriceCurrencies>
where
    BaseCurrency: CurrencyDef,
    BaseCurrency::Group: MemberOf<BaseCurrencies> + MemberOf<AlarmCurrencies::TopG>,
    BaseCurrencies: Group,
    AlarmCurrencies: Group,
    PriceCurrencies: Group<TopG = PriceCurrencies>,
{
    FeedPrices {
        prices: Vec<PriceDTO<PriceCurrencies>>,
    },
    AddPriceAlarm {
        alarm: Alarm<AlarmCurrencies, BaseCurrency, BaseCurrencies>,
    },
    /// Returns [`DispatchAlarmsResponse`] as response data.
    DispatchAlarms { max_count: AlarmsCount },
}

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SudoMsg<PriceCurrencies>
where
    PriceCurrencies: Group,
{
    RegisterFeeder {
        feeder_address: String,
    },
    RemoveFeeder {
        feeder_address: String,
    },
    UpdateConfig(PriceConfig),
    SwapTree {
        tree: HumanReadableTree<SwapTarget<PriceCurrencies>>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryMsg<PriceCurrencies>
where
    PriceCurrencies: Group,
{
    // Returns contract's semantic version as a package, which is set in `Cargo.toml`.
    ContractVersion {},

    /// Implementation of [versioning::query::ProtocolPackage::Release]
    ProtocolPackageRelease {},

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

    /// Provides all supported prices
    ///
    /// Returns `oracle::api::PricesResponse`
    Prices {},

    /// Report the base currency as [SymbolOwned]
    ///
    /// Implementation of [crate::api::price::QueryMsg::BaseCurrency]
    BaseCurrency {},

    /// Provides the price of a currency against the base currency, i.e. serving as its quote currency
    ///
    /// Implementation of [crate::api::price::QueryMsg::BasePrice]
    BasePrice {
        currency: CurrencyDTO<PriceCurrencies>,
    },

    /// Implementation of [oracle_platform::msg::QueryMsg::StableCurrency]
    StableCurrency {},

    /// Implementation of [oracle_platform::msg::QueryMsg::StablePrice]
    StablePrice {
        currency: CurrencyDTO<PriceCurrencies>,
    },

    /// Lists configured swap pairs
    ///
    /// Return `oracle;:api::SupportedCurrencyPairsResponse`
    SupportedCurrencyPairs {},

    /// Lists configured currencies
    ///
    /// Return a `Vec<oracle::api::Currency>`
    Currencies {},

    /// Provides a path in the swap tree between two arbitrary currencies
    ///
    /// Returns `oracle::api::swap::SwapPath`
    /// Implementation of [crate::api::swap::QueryMsg::SwapPath]
    SwapPath {
        from: CurrencyDTO<PriceCurrencies>,
        to: CurrencyDTO<PriceCurrencies>,
    },
    /// Returns [`Status`] as response data.
    AlarmsStatus {},
}

#[derive(Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Config {
    pub price_config: PriceConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteAlarmMsg {
    PriceAlarm(),
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "testing", derive(PartialEq, Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct DispatchAlarmsResponse(pub AlarmsCount);

pub type SupportedCurrencyPairsResponse<PriceCurrencies> = Vec<SwapLeg<PriceCurrencies>>;

pub type CurrenciesResponse = Vec<Currency>;

#[derive(Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Currency {
    pub ticker: SymbolOwned,
    pub bank_symbol: SymbolOwned,
    pub dex_symbol: SymbolOwned,
    pub decimal_digits: u8,
    pub group: CurrencyGroup,
}

impl Currency {
    pub fn new(def: &Definition, group: CurrencyGroup) -> Self
where {
        Self {
            ticker: def.ticker.into(),
            bank_symbol: def.bank_symbol.into(),
            dex_symbol: def.dex_symbol.into(),
            decimal_digits: def.decimal_digits,
            group,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CurrencyGroup {
    Native,
    Lpn,
    Lease,
    PaymentOnly,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SwapTreeResponse<G>
where
    G: Group,
{
    pub tree: HumanReadableTree<SwapTarget<G>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub struct PricesResponse<PriceCurrencies, BaseC, BaseCurrencies>
where
    PriceCurrencies: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseCurrencies> + MemberOf<PriceCurrencies::TopG>,
    BaseCurrencies: Group + MemberOf<PriceCurrencies>,
{
    pub prices: Vec<BasePrice<PriceCurrencies, BaseC, BaseCurrencies>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AlarmsStatusResponse {
    pub remaining_alarms: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SwapLeg<G>
where
    G: Group,
{
    pub from: CurrencyDTO<G>,
    pub to: SwapTarget<G>,
}

impl<G> Serialize for SwapLeg<G>
where
    G: Group,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (&self.from, &self.to).serialize(serializer)
    }
}

impl<'de, G> Deserialize<'de> for SwapLeg<G>
where
    G: Group,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|(from, to)| Self { from, to })
    }
}

#[cfg(test)]
mod test {
    use super::QueryMsg;
    use currencies::Lpns;
    use platform::tests as platform_tests;

    #[test]
    fn release() {
        assert_eq!(
            Ok(QueryMsg::<Lpns>::ProtocolPackageRelease {}),
            platform_tests::ser_de(&versioning::query::ProtocolPackage::Release {}),
        );

        platform_tests::ser_de::<_, QueryMsg<Lpns>>(
            &versioning::query::PlatformPackage::Release {},
        )
        .unwrap_err();
    }
}
