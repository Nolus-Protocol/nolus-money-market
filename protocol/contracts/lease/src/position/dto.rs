use std::result::Result as StdResult;

use currency::{CurrencyDef, MemberOf};
use finance::{
    coin::{Coin, CoinDTO, WithCoin, WithCoinResult},
    error::Error as FinanceError,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::{open::PositionSpecDTO, LeaseAssetCurrencies, LeasePaymentCurrencies},
    error::ContractError,
};

use super::{Position, Spec};

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PositionDTO {
    amount: CoinDTO<LeaseAssetCurrencies>,
    spec: PositionSpecDTO,
}

pub type WithPositionResult<V> = Result<<V as WithPosition>::Output, <V as WithPosition>::Error>;

pub trait WithPosition {
    type Output;
    type Error;

    fn on<Asset>(self, position: Position<Asset>) -> WithPositionResult<Self>
    where
        Asset: CurrencyDef,
        Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>;
}

impl PositionDTO {
    pub(crate) fn new(amount: CoinDTO<LeaseAssetCurrencies>, spec: PositionSpecDTO) -> Self {
        Self { amount, spec }
    }

    pub fn amount(&self) -> &CoinDTO<LeaseAssetCurrencies> {
        &self.amount
    }

    pub fn with_position<V>(self, cmd: V) -> StdResult<V::Output, V::Error>
    where
        V: WithPosition,
        FinanceError: Into<V::Error>,
        ContractError: Into<V::Error>,
    {
        struct WithAmount<V> {
            cmd: V,
            spec: PositionSpecDTO,
        }

        impl<V> WithCoin<LeaseAssetCurrencies> for WithAmount<V>
        where
            V: WithPosition,
            ContractError: Into<V::Error>,
        {
            type VisitorG = LeasePaymentCurrencies;

            type Output = V::Output;

            type Error = V::Error;

            fn on<Asset>(self, amount: Coin<Asset>) -> WithCoinResult<LeaseAssetCurrencies, Self>
            where
                Asset: CurrencyDef,
                Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<Self::VisitorG>,
            {
                Spec::try_from(self.spec)
                    .map(|spec| Position::<Asset>::new(amount, spec))
                    .map_err(Into::into)
                    .and_then(|position| self.cmd.on(position))
            }
        }
        self.amount.with_super_coin(WithAmount {
            cmd,
            spec: self.spec,
        })
    }
}

impl<Asset> From<Position<Asset>> for PositionDTO
where
    Asset: CurrencyDef,
    Asset::Group: MemberOf<LeaseAssetCurrencies>,
{
    fn from(value: Position<Asset>) -> Self {
        Self {
            amount: value.amount.into(),
            spec: value.spec.into(),
        }
    }
}

impl From<PositionDTO> for CoinDTO<LeaseAssetCurrencies> {
    fn from(value: PositionDTO) -> Self {
        value.amount
    }
}
