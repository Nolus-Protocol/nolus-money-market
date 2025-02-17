use serde::{Deserialize, Deserializer, Serialize, Serializer};

use currency::{CurrencyDTO, CurrencyDef, DefinitionRef, Group, MemberOf};
use finance::price::{base::BasePrice, dto::PriceDTO};
use marketprice::config::Config as PriceConfig;
use sdk::cosmwasm_std::Addr;
use tree::HumanReadableTree;

pub use super::alarms::Alarm;
use super::swap::SwapTarget;

pub type AlarmsCount = platform::dispatcher::AlarmsCount;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg<PriceCurrencies>
where
    PriceCurrencies: Group,
{
    pub config: Config,
    pub swap_tree: HumanReadableTree<SwapTarget<PriceCurrencies>>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug, Clone))]
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

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug, Clone))]
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
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

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Config {
    pub price_config: PriceConfig,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteAlarmMsg {
    PriceAlarm(),
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "contract_testing", derive(PartialEq, Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct DispatchAlarmsResponse(pub AlarmsCount);

pub type SupportedCurrencyPairsResponse<PriceCurrencies> = Vec<SwapLeg<PriceCurrencies>>;

pub type CurrenciesResponse = Vec<Currency>;

#[derive(Serialize)]
#[cfg_attr(feature = "contract_testing", derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Currency {
    #[serde(flatten)]
    pub definition: DefinitionRef,
    pub group: CurrencyGroup,
}

impl Currency {
    pub fn new(definition: DefinitionRef, group: CurrencyGroup) -> Self
where {
        Self { definition, group }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "contract_testing", derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CurrencyGroup {
    Native,
    Lpn,
    Lease,
    PaymentOnly,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SwapTreeResponse<G>
where
    G: Group,
{
    pub tree: HumanReadableTree<SwapTarget<G>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
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

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
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

#[cfg(all(feature = "internal.test.contract", test))]
mod test {
    use crate::api::{Currency, CurrencyGroup};

    use super::QueryMsg;
    use currencies::{testing::LeaseC1, Lpns};
    use currency::{CurrencyDef, SymbolOwned};
    use platform::tests as platform_tests;
    use serde::Deserialize;

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

    #[test]
    fn currency() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct ExpectedCurrency {
            ticker: SymbolOwned,
            bank_symbol: SymbolOwned,
            dex_symbol: SymbolOwned,
            decimal_digits: u8,
            group: CurrencyGroup,
        }
        let definition_in = LeaseC1::dto().definition();
        let currency_in = Currency {
            definition: definition_in,
            group: CurrencyGroup::PaymentOnly,
        };

        assert_eq!(
            Ok(ExpectedCurrency {
                ticker: definition_in.ticker.into(),
                bank_symbol: definition_in.bank_symbol.into(),
                dex_symbol: definition_in.dex_symbol.into(),
                decimal_digits: definition_in.decimal_digits,
                group: currency_in.group
            }),
            platform_tests::ser_de(&currency_in),
        );

        platform_tests::ser_de::<_, QueryMsg<Lpns>>(
            &versioning::query::PlatformPackage::Release {},
        )
        .unwrap_err();
    }
}
