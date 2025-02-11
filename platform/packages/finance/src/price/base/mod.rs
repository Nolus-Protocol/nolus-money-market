use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use currency::{Currency, CurrencyDTO, CurrencyDef, Group, MemberOf};
use with_price::WithPrice;

use crate::{
    coin::{Coin, CoinDTO},
    error::{Error, Result as FinanceResult},
};

use super::Price;

mod unchecked;
pub mod with_price;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(
    try_from = "unchecked::BasePrice<BaseG, QuoteG>",
    into = "unchecked::BasePrice<BaseG, QuoteG>",
    bound(serialize = "", deserialize = "",)
)]
pub struct BasePrice<BaseG, QuoteC, QuoteG>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
    QuoteG: Group,
{
    amount: CoinDTO<BaseG>,
    // decouples this field representation on the wire from the `Coin<>`-s one
    amount_quote: Coin<QuoteC>,
    #[serde(skip)]
    _quote_group: PhantomData<QuoteG>,
}

impl<BaseG, QuoteC, QuoteG> BasePrice<BaseG, QuoteC, QuoteG>
where
    BaseG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<BaseG::TopG>,
    QuoteG: Group,
{
    pub fn from_price<C>(price: &Price<C, QuoteC>, c_dto: CurrencyDTO<BaseG>) -> Self
    where
        C: Currency + MemberOf<BaseG>,
    {
        Self::new_unchecked(CoinDTO::from_coin(price.amount, c_dto), price.amount_quote)
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn new(amount: CoinDTO<BaseG>, amount_quote: Coin<QuoteC>) -> Self {
        Self::new_unchecked(amount, amount_quote)
    }

    fn new_checked(amount: CoinDTO<BaseG>, amount_quote: Coin<QuoteC>) -> FinanceResult<Self> {
        let res = Self::new_raw(amount, amount_quote);
        res.invariant_held().map(|()| res)
    }

    fn new_unchecked(amount: CoinDTO<BaseG>, amount_quote: Coin<QuoteC>) -> Self {
        let res = Self::new_raw(amount, amount_quote);

        debug_assert_eq!(Ok(()), res.invariant_held());
        res
    }

    fn new_raw(amount: CoinDTO<BaseG>, amount_quote: Coin<QuoteC>) -> Self {
        Self {
            amount,
            amount_quote,
            _quote_group: PhantomData,
        }
    }

    pub fn try_as_specific<C, SubG>(
        &self,
        amount_c: &CurrencyDTO<SubG>,
    ) -> Result<Price<C, QuoteC>, Error>
    where
        C: Currency + MemberOf<SubG>,
        SubG: Group + MemberOf<BaseG>,
    {
        self.of_currency(amount_c)
            .map(|()| super::total_of(self.amount.as_specific(amount_c)).is(self.amount_quote))
    }

    fn of_currency<SubG>(&self, amount_c: &CurrencyDTO<SubG>) -> Result<(), Error>
    where
        SubG: Group + MemberOf<BaseG>,
    {
        self.amount.of_currency_dto(amount_c).map_err(Into::into)
    }

    fn invariant_held(&self) -> FinanceResult<()> {
        struct InvariantCheck<PriceG> {
            price_g: PhantomData<PriceG>,
        }

        impl<PriceG, QuoteC> WithPrice<QuoteC> for InvariantCheck<PriceG>
        where
            PriceG: Group,
            QuoteC: Currency,
        {
            type PriceG = PriceG;

            type Output = ();

            type Error = Error;

            fn exec<C>(self, converted: Price<C, QuoteC>) -> Result<Self::Output, Self::Error>
            where
                C: 'static,
            {
                converted.invariant_held()
            }
        }

        with_price::execute_with_coins(
            self.amount,
            self.amount_quote,
            InvariantCheck {
                price_g: PhantomData::<BaseG>,
            },
        )
    }
}

//
// Price related transformations
//
impl<C, G, QuoteC, QuoteG> From<Price<C, QuoteC>> for BasePrice<G, QuoteC, QuoteG>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
    QuoteG: Group,
{
    fn from(price: Price<C, QuoteC>) -> Self {
        Self::from(&price)
    }
}

impl<C, G, QuoteC, QuoteG> From<&Price<C, QuoteC>> for BasePrice<G, QuoteC, QuoteG>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
    QuoteG: Group,
{
    fn from(price: &Price<C, QuoteC>) -> Self {
        Self::from_price(price, C::dto().into_super_group())
    }
}

impl<C, G, QuoteC, QuoteG> TryFrom<BasePrice<G, QuoteC, QuoteG>> for Price<C, QuoteC>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
    QuoteG: Group,
{
    type Error = Error;

    fn try_from(base: BasePrice<G, QuoteC, QuoteG>) -> Result<Self, Self::Error> {
        Self::try_from(&base)
    }
}

impl<C, G, QuoteC, QuoteG> TryFrom<&BasePrice<G, QuoteC, QuoteG>> for Price<C, QuoteC>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
    QuoteG: Group,
{
    type Error = Error;

    fn try_from(base: &BasePrice<G, QuoteC, QuoteG>) -> Result<Self, Self::Error> {
        base.try_as_specific(C::dto())
    }
}

#[cfg(test)]
mod test_invariant {
    use currency::{
        test::{SubGroup, SubGroupTestC10, SuperGroup, SuperGroupTestC1, SuperGroupTestC2},
        CurrencyDef, Group, MemberOf, SymbolStatic,
    };
    use sdk::cosmwasm_std::{from_json, to_json_string, StdError, StdResult};

    use crate::coin::Coin;

    use super::BasePrice;

    #[test]
    #[should_panic = "zero"]
    fn base_zero() {
        new_invalid(
            Coin::<SuperGroupTestC1>::new(0),
            Coin::<SuperGroupTestC2>::new(3),
        )
    }

    #[test]
    fn base_zero_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "0", "ticker": "{}"}}, "amount_quote": {{"amount": "3", "ticker": "{}"}}}}"#,
            SuperGroupTestC1::dto().definition().ticker,
            ticker::<SuperGroupTestC2>()
        );

        assert_load_err(
            load::<SuperGroup, SuperGroupTestC2, SuperGroup>(&json.into_bytes()),
            "zero",
        );
    }

    #[test]
    #[should_panic = "zero"]
    fn quote_zero() {
        new_invalid(
            Coin::<SuperGroupTestC1>::new(6),
            Coin::<SuperGroupTestC2>::new(0),
        );
    }

    #[test]
    fn quote_zero_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "6", "ticker": "{}"}}, "amount_quote": {{"amount": "0", "ticker": "{}"}}}}"#,
            ticker::<SuperGroupTestC1>(),
            ticker::<SuperGroupTestC2>()
        );

        assert_load_err(
            load::<SuperGroup, SuperGroupTestC2, SuperGroup>(&json.into_bytes()),
            "zero",
        );
    }

    #[test]
    fn test_serialize_deserialize() {
        let base_price = BasePrice::<SuperGroup, SubGroupTestC10, SubGroup>::new(
            Coin::<SuperGroupTestC2>::new(2).into(),
            Coin::<SubGroupTestC10>::new(10),
        );

        let serialized = to_json_string(&base_price).expect("Failed to serialize");
        let loaded = load::<SuperGroup, SubGroupTestC10, SubGroup>(&serialized.into_bytes())
            .expect("Failed to deserialize");
        assert_eq!(base_price, loaded);
    }

    fn new_invalid<BaseC, QuoteC>(amount: Coin<BaseC>, amount_quote: Coin<QuoteC>)
    where
        BaseC: CurrencyDef,
        BaseC::Group: MemberOf<SuperGroup>,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<SuperGroup>,
    {
        assert!(BasePrice::new_checked(amount.into(), amount_quote).is_err());

        let _base_price: BasePrice<SuperGroup, QuoteC, SuperGroup> =
            BasePrice::new_unchecked(amount.into(), amount_quote);

        #[cfg(not(debug_assertions))]
        {
            _base_price
                .invariant_held()
                .expect("should have returned an error");
        }
    }

    fn assert_load_err<G, QuoteC, QuoteG>(r: StdResult<BasePrice<G, QuoteC, QuoteG>>, msg: &str)
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group,
    {
        assert!(matches!(
            r,
            Err(StdError::ParseErr {
                target_type,
                msg: real_msg,
                backtrace: _,
            }) if target_type.contains("BasePrice") && real_msg.contains(msg)
        ));
    }

    fn load<G, QuoteC, QuoteG>(json: &[u8]) -> StdResult<BasePrice<G, QuoteC, QuoteG>>
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group + MemberOf<G>,
    {
        load_with_group::<G, QuoteC, QuoteG>(json)
    }

    fn load_with_group<G, QuoteC, QuoteG>(json: &[u8]) -> StdResult<BasePrice<G, QuoteC, QuoteG>>
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group + MemberOf<G>,
    {
        from_json::<BasePrice<G, QuoteC, QuoteG>>(json)
    }

    fn ticker<C>() -> SymbolStatic
    where
        C: CurrencyDef,
    {
        C::dto().definition().ticker
    }
}
