use serde::{Deserialize, Serialize};

use currency::{CurrencyDef, Group, MemberOf};

use crate::{
    coin::{Coin, CoinDTO},
    error::Error,
    price::base::BasePrice as ValidatedBasePrice,
};

/// Brings invariant checking as a step in deserializing and serializing a BasePrice
#[derive(Deserialize, Serialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "snake_case",
    bound(serialize = "", deserialize = "")
)]
pub(super) struct BasePrice<BaseG, QuoteG>
where
    BaseG: Group,
    QuoteG: Group,
{
    amount: CoinDTO<BaseG>,
    amount_quote: CoinDTO<QuoteG>,
}

impl<BaseG, QuoteC, QuoteG> TryFrom<BasePrice<BaseG, QuoteG>>
    for ValidatedBasePrice<BaseG, QuoteC, QuoteG>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
    QuoteG: Group,
{
    type Error = Error;

    fn try_from(unchecked: BasePrice<BaseG, QuoteG>) -> Result<Self, Self::Error> {
        Coin::<QuoteC>::try_from(unchecked.amount_quote).and_then(|amount_quote| {
            ValidatedBasePrice::new_checked(unchecked.amount, amount_quote)
        })
    }
}

impl<BaseG, QuoteC, QuoteG> From<ValidatedBasePrice<BaseG, QuoteC, QuoteG>>
    for BasePrice<BaseG, QuoteG>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
    QuoteG: Group,
{
    fn from(checked: ValidatedBasePrice<BaseG, QuoteC, QuoteG>) -> Self {
        Self {
            amount: checked.amount,
            amount_quote: checked.amount_quote.into(),
        }
    }
}
