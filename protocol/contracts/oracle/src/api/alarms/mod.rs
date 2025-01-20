use std::result::Result as StdResult;

use serde::{Deserialize, Serialize};

use currency::{CurrencyDef, Group, MemberOf};
use finance::{
    error,
    price::{
        base::{
            with_price::{self, WithPrice},
            BasePrice,
        },
        Price,
    },
};
use thiserror::Error;

use sdk::cosmwasm_std::StdError as CosmWasmError;

mod unchecked;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, Clone))]
#[serde(deny_unknown_fields, rename_all = "snake_case", bound(serialize = ""))]
pub enum ExecuteMsg<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    AddPriceAlarm { alarm: Alarm<G, Lpn, Lpns> },
}

pub type Result<T> = StdResult<T, Error>;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("[Oracle; Stub] Failed to add alarm! Cause: {0}")]
    StubAddAlarm(CosmWasmError),

    #[error("[PriceAlarms] {0}")]
    FinanceError(#[from] error::Error),

    #[error("[PriceAlarms] {0}")]
    InvariantBroken(&'static str),
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(
    try_from = "unchecked::Alarm<G, Lpn, Lpns>",
    into = "unchecked::Alarm<G, Lpn, Lpns>",
    bound(serialize = "", deserialize = "")
)]
pub struct Alarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    below: BasePrice<G, Lpn, Lpns>,
    above: Option<BasePrice<G, Lpn, Lpns>>,
}

impl<G, Lpn, Lpns> Alarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    pub fn new<P>(below: P, above_or_equal: Option<P>) -> Alarm<G, Lpn, Lpns>
    where
        P: Into<BasePrice<G, Lpn, Lpns>>,
    {
        let below = below.into();
        let above_or_equal = above_or_equal.map(Into::into);
        let alarm = Self {
            below,
            above: above_or_equal,
        };
        debug_assert_eq!(Ok(()), alarm.invariant_held());
        alarm
    }

    fn invariant_held(&self) -> Result<()> {
        self.above.map_or(Ok(()), |ref above_or_equal|{
            struct BaseCurrencyType<'a, BaseG, QuoteC, QuoteG>
            where
                BaseG: Group,
                QuoteC: CurrencyDef,
                QuoteC::Group: MemberOf<QuoteG>+ MemberOf<BaseG::TopG>,
                QuoteG: Group,
            {
                below_price: &'a BasePrice<BaseG, QuoteC, QuoteG>,
            }

            impl<BaseG, QuoteC, QuoteG> WithPrice<QuoteC> for BaseCurrencyType<'_, BaseG, QuoteC, QuoteG>
            where
                BaseG: Group,
                QuoteC: CurrencyDef,
                QuoteC::Group: MemberOf<QuoteG>+ MemberOf<BaseG::TopG>,
                QuoteG: Group,
            {
                type PriceG = BaseG;

                type Output = ();

                type Error = Error;

                fn exec<C>(
                    self,
                    above_or_equal: Price<C, QuoteC>,
                ) -> StdResult<Self::Output, Self::Error>
                where
                    C: CurrencyDef,
                    C::Group: MemberOf<BaseG>,
                {
                    Price::<C, QuoteC>::try_from(self.below_price).map_err(Error::FinanceError).and_then(|below_price| {
                            if below_price > above_or_equal {
                                Err(Error::InvariantBroken("The below alarm price should be less than or equal to the above_or_equal alarm price"))
                            } else {
                                Ok(())
                            }
                        })
                }
            }
            with_price::execute(
                above_or_equal,
                BaseCurrencyType {
                    below_price: &self.below,
                },
            )
        })
    }
}

impl<G, Lpn, Lpns> From<Alarm<G, Lpn, Lpns>>
    for (BasePrice<G, Lpn, Lpns>, Option<BasePrice<G, Lpn, Lpns>>)
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    fn from(value: Alarm<G, Lpn, Lpns>) -> Self {
        (value.below, value.above)
    }
}

impl<G, Lpn, Lpns> Clone for Alarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    fn clone(&self) -> Self {
        Self {
            below: self.below,
            above: self.above,
        }
    }
}

#[cfg(test)]
mod test {
    use std::fmt::{Display, Formatter, Result as FmtResult};

    use serde::Serialize;

    use currencies::{
        testing::{LeaseC1, LeaseC2, LeaseC3},
        LeaseGroup, Lpn, Lpns, PaymentGroup,
    };
    use currency::{CurrencyDef, Group, MemberOf};
    use finance::{
        coin::{Coin, CoinDTO},
        price::base::BasePrice,
    };
    use sdk::cosmwasm_std::{from_json, to_json_vec, StdError};

    use super::Alarm;

    type BasePriceTest = BasePrice<PaymentGroup, Lpn, Lpns>;

    #[test]
    fn new_valid() {
        let below = BasePriceTest::new(Coin::<LeaseC2>::new(2).into(), Coin::<Lpn>::new(10));
        let above = BasePriceTest::new(Coin::<LeaseC2>::new(1).into(), Coin::<Lpn>::new(12));
        let exp = Alarm::new(below, Some(above));

        let below_json =
            alarm_half_to_json(AlarmPrice::Below, below).expect("Serialization failed");
        let above_json =
            alarm_half_to_json(AlarmPrice::Above, above).expect("Serialization failed");

        let deserialized =
            from_both_str_impl(below_json, Some(&above_json)).expect("Deserialization failed");

        assert_eq!(exp, deserialized);
    }

    #[test]
    fn below_price_ok() {
        let exp_price = BasePriceTest::new(Coin::<LeaseC2>::new(10).into(), Coin::<Lpn>::new(10));
        let exp_res = Ok(Alarm::new(exp_price, None));
        assert_eq!(exp_res, from_below(exp_price));
    }

    #[test]
    fn below_price_err() {
        assert_err::<PaymentGroup, Lpn, Lpns>(
            alarm_half_coins_to_json(
                AlarmPrice::Below,
                Coin::<LeaseC1>::new(5),
                Coin::<Lpn>::new(0),
            )
            .and_then(|json| from_both_str_impl(json, None::<&str>)),
            "The quote amount should not be zero",
        );
        assert_err::<PaymentGroup, Lpn, Lpns>(
            alarm_half_coins_to_json(
                AlarmPrice::Below,
                Coin::<LeaseC2>::new(0),
                Coin::<Lpn>::new(5),
            )
            .and_then(|json| from_both_str_impl(json, None::<&str>)),
            "The amount should not be zero",
        );
    }

    #[test]
    fn above_price_err() {
        let below = alarm_half_coins_to_json(
            AlarmPrice::Below,
            Coin::<LeaseC2>::new(13),
            Coin::<Lpn>::new(15),
        )
        .unwrap();

        assert_err::<PaymentGroup, Lpn, Lpns>(
            alarm_half_coins_to_json(
                AlarmPrice::Above,
                Coin::<LeaseC1>::new(5),
                Coin::<Lpn>::new(0),
            )
            .and_then(|json| from_both_str_impl(&below, Some(&json))),
            "The quote amount should not be zero",
        );
        assert_err::<PaymentGroup, Lpn, Lpns>(
            alarm_half_coins_to_json(
                AlarmPrice::Above,
                Coin::<LeaseC3>::new(0),
                Coin::<Lpn>::new(5),
            )
            .and_then(|json| from_both_str_impl(&below, Some(&json))),
            "The amount should not be zero",
        );
    }

    #[test]
    fn currencies_mismatch() {
        let below = BasePriceTest::new(Coin::<LeaseC1>::new(2).into(), Coin::<Lpn>::new(10));
        let above = BasePriceTest::new(Coin::<LeaseC2>::new(2).into(), Coin::<Lpn>::new(10));

        let msg = "Expected currency";

        assert_err(from_both(below, above), msg);

        let full_json = format!(
            r#"{{"below": {{"amount": {{"amount": "2", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}, "above": {{"amount": {{"amount": "2", "ticker": "{}"}}, "amount_quote": {{"amount": "5", "ticker": "{}"}}}}}}"#,
            LeaseC3::ticker(),
            Lpn::ticker(),
            LeaseC1::ticker(),
            Lpn::ticker()
        );

        assert_err::<PaymentGroup, Lpn, Lpns>(from_json(full_json.into_bytes()), msg);
    }

    #[test]
    fn below_not_less_than_above() {
        let below = BasePriceTest::new(Coin::<LeaseC2>::new(2).into(), Coin::<Lpn>::new(10));
        let above = BasePriceTest::new(Coin::<LeaseC2>::new(2).into(), Coin::<Lpn>::new(9));

        assert_err(
            from_both(below, above),
            "should be less than or equal to the above",
        );
    }

    #[test]
    fn below_price_eq_above() {
        let price = BasePriceTest::new(Coin::<LeaseC3>::new(1).into(), Coin::<Lpn>::new(10));
        let alarm = Alarm::new(price, Some(price));
        let msg = "valid alarm with equal above_or_equal and below prices";

        assert_eq!(alarm, from_both(price, price).expect(msg));
    }

    #[test]
    fn below_price_less_than_above() {
        let price_below = BasePriceTest::new(Coin::<LeaseC3>::new(1).into(), Coin::<Lpn>::new(10));
        let price_above_or_equal =
            BasePriceTest::new(Coin::<LeaseC3>::new(1).into(), Coin::<Lpn>::new(11));
        let alarm = Alarm::new(price_below, Some(price_above_or_equal));
        let msg = "valid alarm";

        assert_eq!(
            alarm,
            from_both(price_below, price_above_or_equal).expect(msg)
        );
    }

    #[track_caller]
    fn assert_err<G, QuoteC, QuoteG>(r: Result<Alarm<G, QuoteC, QuoteG>, StdError>, msg: &str)
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group,
    {
        assert!(r.is_err());
        assert!(matches!(
            r,
            Err(StdError::ParseErr{
                target_type,
                msg: real_msg,
                backtrace:_,
            }) if target_type.contains("Alarm") && real_msg.contains(msg)
        ));
    }

    fn from_below<G, QuoteC, QuoteG>(
        below: BasePrice<G, QuoteC, QuoteG>,
    ) -> Result<Alarm<G, QuoteC, QuoteG>, StdError>
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group,
    {
        from_both_impl::<G, QuoteC, QuoteG, QuoteC, QuoteG>(below, None)
    }

    fn from_both<G, QuoteC1, QuoteG1, QuoteC2, QuoteG2>(
        below: BasePrice<G, QuoteC1, QuoteG1>,
        above: BasePrice<G, QuoteC2, QuoteG2>,
    ) -> Result<Alarm<G, QuoteC1, QuoteG1>, StdError>
    where
        G: Group,
        QuoteC1: CurrencyDef,
        QuoteC1::Group: MemberOf<QuoteG1> + MemberOf<G::TopG>,
        QuoteG1: Group,
        QuoteC2: CurrencyDef,
        QuoteC2::Group: MemberOf<QuoteG2> + MemberOf<G::TopG>,
        QuoteG2: Group,
    {
        from_both_impl(below, Some(above))
    }

    fn from_both_impl<G, QuoteC1, QuoteG1, QuoteC2, QuoteG2>(
        below: BasePrice<G, QuoteC1, QuoteG1>,
        above: Option<BasePrice<G, QuoteC2, QuoteG2>>,
    ) -> Result<Alarm<G, QuoteC1, QuoteG1>, StdError>
    where
        G: Group,
        QuoteC1: CurrencyDef,
        QuoteC1::Group: MemberOf<QuoteG1> + MemberOf<G::TopG>,
        QuoteG1: Group,
        QuoteC2: CurrencyDef,
        QuoteC2::Group: MemberOf<QuoteG2> + MemberOf<G::TopG>,
        QuoteG2: Group,
    {
        let above_str = above
            .map(|above| alarm_half_to_json(AlarmPrice::Above, above))
            .transpose()?;
        let below_str = alarm_half_to_json(AlarmPrice::Below, below)?;
        from_both_str_impl(below_str, above_str)
    }

    fn from_both_str_impl<Str1, Str2, G, QuoteC, QuoteG>(
        below: Str1,
        above: Option<Str2>,
    ) -> Result<Alarm<G, QuoteC, QuoteG>, StdError>
    where
        Str1: AsRef<str>,
        Str2: AsRef<str>,
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group,
    {
        let full_json = above.map_or_else(
            || format!(r#"{{{}}}"#, below.as_ref()),
            |above| format!(r#"{{{}, {}}}"#, below.as_ref(), above.as_ref()),
        );
        from_json(full_json.into_bytes())
    }

    enum AlarmPrice {
        Above,
        Below,
    }
    impl Display for AlarmPrice {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str(match self {
                AlarmPrice::Below => "below",
                AlarmPrice::Above => "above",
            })
        }
    }

    fn alarm_half_to_json<G, QuoteC, QuoteG>(
        price_type: AlarmPrice,
        price: BasePrice<G, QuoteC, QuoteG>,
    ) -> Result<String, StdError>
    where
        G: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG> + MemberOf<G::TopG>,
        QuoteG: Group,
    {
        as_json(&price).map(|string_price| alarm_half_to_json_str(price_type, &string_price))
    }

    fn alarm_half_coins_to_json<C, Q>(
        price_type: AlarmPrice,
        amount: Coin<C>,
        amount_quote: Coin<Q>,
    ) -> Result<String, StdError>
    where
        C: CurrencyDef,
        C::Group: MemberOf<LeaseGroup>,
        Q: CurrencyDef,
        Q::Group: MemberOf<Lpns>,
    {
        as_json(&CoinDTO::<LeaseGroup>::from(amount)).and_then(|amount_str| {
            as_json(&CoinDTO::<Lpns>::from(amount_quote)).map(|amount_quote_str| {
                let price = format!(
                    r#"{{"amount": {},"amount_quote": {}}}"#,
                    amount_str, amount_quote_str
                );
                alarm_half_to_json_str(price_type, &price)
            })
        })
    }

    fn alarm_half_to_json_str(price_type: AlarmPrice, price: &str) -> String {
        format!(r#""{}": {}"#, price_type, price)
    }

    fn as_json<S>(to_serialize: &S) -> Result<String, StdError>
    where
        S: Serialize,
    {
        to_json_vec(to_serialize)
            .and_then(|json_bytes| String::from_utf8(json_bytes).map_err(Into::into))
    }
}
