use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    marker::PhantomData,
    result::Result as StdResult,
};

#[cfg(any(test, feature = "testing"))]
use currency::CurrencyDef;
use currency::{Currency, CurrencyDTO, Group, InPoolWith, MemberOf};

use crate::{
    coin::CoinDTO,
    error::{Error, Result},
    price::Price,
};

mod unchecked;
pub mod with_price;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "unchecked::PriceDTO<G>",
    bound(serialize = "", deserialize = "G: Group<TopG = G>")
)]
pub struct PriceDTO<G>
where
    G: Group,
{
    amount: CoinDTO<G>,
    amount_quote: CoinDTO<G>,
}

impl<G> PriceDTO<G>
where
    G: Group<TopG = G>,
{
    pub fn from_price<C, Q>(
        price: Price<C, Q>,
        base_c: CurrencyDTO<G>,
        quote_c: CurrencyDTO<G>,
    ) -> Self
    where
        C: Currency + MemberOf<G>,
        Q: Currency + MemberOf<G>,
    {
        Self::new_unchecked(
            CoinDTO::from_coin(price.amount, base_c),
            CoinDTO::from_coin(price.amount_quote, quote_c),
        )
    }

    pub(super) fn new_unchecked(base: CoinDTO<G>, quote: CoinDTO<G>) -> Self {
        let res = Self {
            amount: base,
            amount_quote: quote,
        };
        debug_assert!(
            res.invariant_held().is_ok(),
            "Invariant result = {:?}",
            res.invariant_held()
        );
        res
    }

    fn try_new(base: CoinDTO<G>, quote: CoinDTO<G>) -> Result<Self> {
        Self {
            amount: base,
            amount_quote: quote,
        }
        .invariant_held()
    }

    pub const fn base(&self) -> &CoinDTO<G> {
        &self.amount
    }

    pub const fn quote(&self) -> &CoinDTO<G> {
        &self.amount_quote
    }

    fn invariant_held(self) -> Result<Self> {
        struct InvariantCheck<G> {
            g: PhantomData<G>,
        }

        impl<G> WithPrice for InvariantCheck<G>
        where
            G: Group,
        {
            type G = G;

            type Output = ();

            type Error = Error;

            fn exec<C, QuoteC>(self, converted: Price<C, QuoteC>) -> Result<Self::Output>
            where
                C: Currency + MemberOf<G>,
                QuoteC: Currency + MemberOf<G>,
            {
                converted.invariant_held()
            }
        }

        with_price::execute_with_coins(
            self.amount,
            self.amount_quote,
            InvariantCheck {
                g: PhantomData::<G>,
            },
        )
        .map(|()| self)
    }

    /// Intended in scenarios when the currencies are known in advance.
    #[cfg(any(test, feature = "testing"))]
    pub fn as_specific<C, QuoteC>(
        &self,
        amount_c: &CurrencyDTO<G>,
        quote_c: &CurrencyDTO<G>,
    ) -> Price<C, QuoteC>
    where
        C: Currency + MemberOf<G>,
        QuoteC: Currency + MemberOf<G>,
    {
        debug_assert!(self.of_currencies(amount_c, quote_c).is_ok());
        super::total_of(self.amount.as_specific(amount_c))
            .is(self.amount_quote.as_specific(quote_c))
    }

    #[cfg(any(test, feature = "testing"))]
    fn of_currencies(&self, amount_c: &CurrencyDTO<G>, quote_c: &CurrencyDTO<G>) -> Result<()> {
        self.amount
            .of_currency_dto(amount_c)
            .and_then(|()| self.amount_quote.of_currency_dto(quote_c))
            .map_err(Into::into)
    }
}

#[cfg(any(test, feature = "testing"))]
impl<G, C, QuoteC> From<Price<C, QuoteC>> for PriceDTO<G>
where
    G: Group<TopG = G>,
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<G>,
{
    fn from(price: Price<C, QuoteC>) -> Self {
        Self::new_unchecked(price.amount.into(), price.amount_quote.into())
    }
}

#[cfg(any(test, feature = "testing"))]
impl<G, C, QuoteC> TryFrom<PriceDTO<G>> for Price<C, QuoteC>
where
    G: Group<TopG = G>,
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<G>,
{
    type Error = Error;

    fn try_from(price: PriceDTO<G>) -> StdResult<Self, Self::Error> {
        Self::try_from(&price)
    }
}

#[cfg(any(test, feature = "testing"))]
impl<G, C, QuoteC> TryFrom<&PriceDTO<G>> for Price<C, QuoteC>
where
    G: Group<TopG = G>,
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<G>,
{
    type Error = Error;

    fn try_from(price: &PriceDTO<G>) -> StdResult<Self, Self::Error> {
        let dto_c = currency::dto::<C, G>();
        let dto_quote = currency::dto::<QuoteC, G>();
        price
            .of_currencies(&dto_c, &dto_quote)
            .map(|()| price.as_specific(&dto_c, &dto_quote))
    }
}

impl<G> Display for PriceDTO<G>
where
    G: Group,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Price({}/{})", self.amount, self.amount_quote))
    }
}

pub trait WithPrice {
    type G: Group;

    type Output;
    type Error;

    fn exec<C, QuoteC>(self, _: Price<C, QuoteC>) -> StdResult<Self::Output, Self::Error>
    where
        C: Currency + MemberOf<Self::G>,
        QuoteC: Currency + MemberOf<Self::G> + InPoolWith<C>;
}

pub trait WithBase<C>
where
    C: Currency,
{
    type Output;
    type Error;

    fn exec<QuoteC>(self, _: Price<C, QuoteC>) -> StdResult<Self::Output, Self::Error>
    where
        QuoteC: Currency;
}

#[cfg(test)]
mod test_invariant {

    use currency::test::{
        SuperGroup, SuperGroupTestC1, SuperGroupTestC2, SuperGroupTestC4, SuperGroupTestC5,
    };
    use currency::{error::Error as CurrencyError, CurrencyDef, Group, MemberOf};
    use sdk::cosmwasm_std::{from_json, StdError as CWError, StdResult as CWResult};

    use crate::{
        coin::{Coin, CoinDTO},
        error::{Error, Result},
    };

    use super::PriceDTO;

    type TC = SuperGroup;

    #[test]
    fn base_zero() {
        assert_err(
            new_invalid(
                Coin::<SuperGroupTestC1>::new(0),
                Coin::<SuperGroupTestC2>::new(5),
            ),
            "zero",
        );
    }

    #[test]
    fn base_zero_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "0", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}"#,
            SuperGroupTestC1::ticker(),
            SuperGroupTestC2::ticker()
        );
        assert_load_err(load(&json.into_bytes()), "not be zero");
    }

    #[test]
    fn quote_zero() {
        assert_err(
            new_invalid(
                Coin::<SuperGroupTestC1>::new(10),
                Coin::<SuperGroupTestC2>::new(0),
            ),
            "zero",
        )
    }

    #[test]
    fn quote_zero_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "10", "ticker": "{}"}}, "amount_quote": {{"amount": "0", "ticker": "{}"}}}}"#,
            SuperGroupTestC1::ticker(),
            SuperGroupTestC2::ticker()
        );
        assert_load_err(load(&json.into_bytes()), "not be zero");
    }

    #[test]
    fn currencies_match() {
        assert_err(
            new_invalid(
                Coin::<SuperGroupTestC5>::new(4),
                Coin::<SuperGroupTestC5>::new(5),
            ),
            "should be equal to the identity if the currencies match",
        );
    }

    #[test]
    fn currencies_match_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "10", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}"#,
            SuperGroupTestC5::ticker(),
            SuperGroupTestC5::ticker()
        );
        assert_load_err(
            load(&json.into_bytes()),
            "should be equal to the identity if the currencies match",
        );
    }

    #[test]
    fn currencies_match_ok() {
        let p = PriceDTO::<TC>::new_unchecked(
            Coin::<SuperGroupTestC5>::new(4).into(),
            Coin::<SuperGroupTestC5>::new(4).into(),
        );
        assert_eq!(
            &CoinDTO::<TC>::from(Coin::<SuperGroupTestC5>::new(4)),
            p.base()
        );
    }

    #[test]
    fn currencies_match_ok_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "4", "ticker": "{}"}}, "amount_quote": {{"amount": "4", "ticker": "{}"}}}}"#,
            SuperGroupTestC5::ticker(),
            SuperGroupTestC5::ticker()
        );
        assert_eq!(
            load(&json.into_bytes()).unwrap().base(),
            &CoinDTO::<TC>::from(Coin::<SuperGroupTestC5>::new(4)),
        );
    }

    #[test]
    fn unknown_ticker_json() {
        let r = load_with_groups::<TC>(&format!(
            r#"{{"amount": {{"amount": "4", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}"#,
            "UNKNOWN",
            SuperGroupTestC2::ticker()
        ).into_bytes());
        assert_load_err(r, "pretending to be ticker of a currency pertaining to");
    }

    #[test]
    fn invalid_pair() {
        let p = PriceDTO::<TC>::try_new(
            Coin::<SuperGroupTestC2>::new(4).into(),
            Coin::<SuperGroupTestC4>::new(5).into(),
        );
        assert_eq!(
            Error::CurrencyError(CurrencyError::NotInPoolWith {
                buddy1: SuperGroupTestC2::ticker(),
                buddy2: SuperGroupTestC4::ticker()
            }),
            p.unwrap_err()
        );
    }

    #[test]
    fn invalid_pair_json() {
        let json = format!(
            r#"{{"amount": {{"amount": "4", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}"#,
            SuperGroupTestC4::ticker(),
            SuperGroupTestC2::ticker()
        );
        assert_load_err(load(&json.into_bytes()), "No records for a pool with");
    }

    fn new_invalid<C, QuoteC>(base: Coin<C>, quote: Coin<QuoteC>) -> Result<PriceDTO<TC>>
    where
        C: CurrencyDef,
        C::Group: MemberOf<TC>,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<TC>,
    {
        PriceDTO::<TC>::try_new(base.into(), quote.into())
    }

    fn load(json: &[u8]) -> CWResult<PriceDTO<TC>> {
        load_with_groups::<TC>(json)
    }

    fn load_with_groups<G>(json: &[u8]) -> CWResult<PriceDTO<G>>
    where
        G: Group<TopG = G>,
    {
        from_json::<PriceDTO<G>>(json)
    }

    fn assert_load_err<G>(r: CWResult<PriceDTO<G>>, msg: &str)
    where
        G: Group,
    {
        assert!(matches!(
            r,
            Err(CWError::ParseErr {
                target_type,
                msg: real_msg,
                backtrace: _,
            }) if target_type.contains("PriceDTO") && real_msg.contains(msg)
        ));
    }

    fn assert_err<G>(r: Result<PriceDTO<G>>, msg: &str)
    where
        G: Group,
    {
        assert!(r.expect_err("expected an error").to_string().contains(msg));
    }
}
