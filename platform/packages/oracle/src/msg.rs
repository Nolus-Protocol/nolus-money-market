use serde::Serialize;

use currency::{CurrencyDTO, Group};

#[derive(Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", bound(serialize = ""))]
pub enum BaseCurrencyQueryMsg<G>
where
    G: Group,
{
    /// Report the base currency as [CurrencyDTO<G>]
    BaseCurrency {},

    /// Provide the price of a currency against the base one
    ///
    /// Return [BasePrice<G, <BaseCurrency>, <BaseCurrencyGroup>>]
    BasePrice { currency: CurrencyDTO<G> },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", bound(serialize = ""))]
pub enum StableCurrencyQueryMsg<G>
where
    G: Group,
{
    /// Report the stable currency as [CurrencyDTO<G>]
    StableCurrency {},

    /// Provide the price of a currency against the stable one
    ///
    /// Return [BasePrice<G, <BaseCurrency>, <BaseCurrencyGroup>>]
    StablePrice { currency: CurrencyDTO<G> },
}
