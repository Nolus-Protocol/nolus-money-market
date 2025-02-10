use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::result::Result as StdResult;

use thiserror::Error;

use currency::{CurrencyDTO, Group};
use sdk::cosmwasm_std::StdError;

pub type PoolId = u64;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub enum QueryMsg<PriceCurrencies>
where
    PriceCurrencies: Group,
{
    /// Provides a path in the swap tree between two arbitrary currencies
    ///
    /// Returns `self::SwapPath`
    SwapPath {
        from: CurrencyDTO<PriceCurrencies>,
        to: CurrencyDTO<PriceCurrencies>,
    },
}

pub type SwapPath<G> = Vec<SwapTarget<G>>;

pub type Result<T> = StdResult<T, Error>;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("[Oracle; Stub] Failed to query swap path! Cause: {0}")]
    StubSwapPathQuery(StdError),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SwapTarget<G>
where
    G: Group,
{
    pub pool_id: PoolId,
    pub target: CurrencyDTO<G>,
}

impl<G> Serialize for SwapTarget<G>
where
    G: Group,
{
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (self.pool_id, self.target).serialize(serializer)
    }
}

impl<'de, G> Deserialize<'de> for SwapTarget<G>
where
    G: Group,
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|(pool_id, target)| Self { pool_id, target })
    }
}
