use std::marker::PhantomData;

use currency::{Currency, CurrencyDef, Group, MemberOf};

use crate::{
    coin::{Coin, CoinDTO, WithCoin, WithCoinResult},
    error::Error,
    price::{base::BasePrice, Price},
};

pub trait WithPrice<QuoteC>
where
    QuoteC: Currency,
{
    type PriceG: Group;

    type Output;

    type Error;

    fn exec<C>(self, _: Price<C, QuoteC>) -> Result<Self::Output, Self::Error>
    where
        C: CurrencyDef,
        C::Group: MemberOf<Self::PriceG>;
}

/// Execute the provided price command on a valid base price
pub fn execute<BaseG, QuoteC, QuoteG, Cmd>(
    price: &BasePrice<BaseG, QuoteC, QuoteG>,
    cmd: Cmd,
) -> Result<Cmd::Output, Cmd::Error>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
    QuoteG: Group,
    Cmd: WithPrice<QuoteC, PriceG = BaseG>,
    Cmd::Error: From<Error>,
{
    price.amount.with_coin(CoinResolve {
        price: UncheckedConversion::<BaseG, QuoteC>(PhantomData, price.amount_quote),
        cmd,
    })
}

/// Execute the provided price command on a non-validated price
/// Intended mainly for invariant validation purposes.
pub(super) fn execute_with_coins<BaseG, QuoteC, Cmd>(
    amount: CoinDTO<BaseG>,
    amount_quote: Coin<QuoteC>,
    cmd: Cmd,
) -> Result<Cmd::Output, Cmd::Error>
where
    BaseG: Group,
    QuoteC: Currency,
    Cmd: WithPrice<QuoteC, PriceG = BaseG>,
    Cmd::Error: From<Error>,
{
    amount.with_coin(CoinResolve {
        price: CheckedConversion::<BaseG, QuoteC>(PhantomData, amount_quote),
        cmd,
    })
}

trait PriceFactory {
    type G: Group;
    type QuoteC: Currency;

    fn try_obtain_price<C>(self, amount: Coin<C>) -> Result<Price<C, Self::QuoteC>, Error>
    where
        C: Currency + MemberOf<Self::G>;
}

struct UncheckedConversion<BaseG, QuoteC>(PhantomData<BaseG>, Coin<QuoteC>)
where
    // BaseG: Group,
    QuoteC: CurrencyDef;
// QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
// QuoteG: Group;

impl<BaseG, QuoteC> PriceFactory for UncheckedConversion<BaseG, QuoteC>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<BaseG::TopG>,
{
    type G = BaseG;
    type QuoteC = QuoteC;

    fn try_obtain_price<C>(self, amount: Coin<C>) -> Result<Price<C, Self::QuoteC>, Error>
    where
        C: Currency + MemberOf<Self::G>,
    {
        Ok(Price::new(amount, self.1))
    }
}

struct CheckedConversion<BaseG, QuoteC>(PhantomData<BaseG>, Coin<QuoteC>)
where
    BaseG: Group,
    QuoteC: Currency;

impl<BaseG, QuoteC> PriceFactory for CheckedConversion<BaseG, QuoteC>
where
    BaseG: Group,
    QuoteC: Currency,
{
    type G = BaseG;
    type QuoteC = QuoteC;

    fn try_obtain_price<C>(self, amount: Coin<C>) -> Result<Price<C, Self::QuoteC>, Error>
    where
        C: Currency + MemberOf<Self::G>,
    {
        Price::try_new(amount, self.1)
    }
}

struct CoinResolve<Price, G, QuoteC, Cmd>
where
    Price: PriceFactory<G = G, QuoteC = QuoteC>,
    G: Group,
    QuoteC: Currency,
    Cmd: WithPrice<QuoteC, PriceG = G>,
{
    price: Price,
    cmd: Cmd,
}

impl<Price, G, QuoteC, Cmd> WithCoin<G> for CoinResolve<Price, G, QuoteC, Cmd>
where
    Price: PriceFactory<G = G, QuoteC = QuoteC>,
    G: Group,
    QuoteC: Currency,
    Cmd: WithPrice<QuoteC, PriceG = G>,
    Cmd::Error: From<Error>,
{
    type Output = Cmd::Output;

    type Error = Cmd::Error;

    fn on<C>(self, amount: Coin<C>) -> WithCoinResult<G, Self>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G> + MemberOf<G::TopG>,
    {
        self.price
            .try_obtain_price(amount)
            .map_err(Into::into)
            .and_then(|price| self.cmd.exec(price))
    }
}
