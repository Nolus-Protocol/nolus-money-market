use std::marker::PhantomData;

use crate::{coin::Amount, percent::Units as PercentUnits, price::Price, ratio::Ratio};

use super::Fractionable;

impl<C, QuoteC> Fractionable<PercentUnits> for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    fn safe_mul(self, fraction: &Ratio<PercentUnits>) -> Self {
        self.lossy_mul(&RatioUpcast(PhantomData, fraction).into())
    }
}

struct RatioUpcast<'a, U>(PhantomData<U>, &'a Ratio<U>);

impl<'a, U> From<RatioUpcast<'a, U>> for Ratio<Amount>
where
    U: PartialEq + PartialOrd + Copy + Into<Amount>,
{
    fn from(upcast: RatioUpcast<'a, U>) -> Self {
        Self::new(upcast.1.parts().into(), upcast.1.total().into())
    }
}

struct RatioTryUpcast<'a>(&'a Ratio<usize>);

const EXPECT_MSG: &str = "usize should convert into u128";

impl From<RatioTryUpcast<'_>> for Ratio<Amount> {
    fn from(upcast: RatioTryUpcast<'_>) -> Self {
        Self::new(
            upcast.0.parts().try_into().expect(EXPECT_MSG),
            upcast.0.total().try_into().expect(EXPECT_MSG),
        )
    }
}

#[cfg(test)]
mod test {
    use currency::test::{SubGroupTestC10, SuperGroupTestC1};

    use crate::coin::{Amount, Coin};

    mod percent {
        use crate::fractionable::price::test::{c, q};
        use crate::{percent::Percent, price};

        #[test]
        fn greater_than_one() {
            let price = price::total_of(c(1)).is(q(1000));
            let permille = Percent::from_permille(1);
            assert_eq!(permille.of(price), price::total_of(c(1)).is(q(1)));
        }

        #[test]
        fn less_than_one() {
            let price = price::total_of(c(10)).is(q(1));
            let twenty_percents = Percent::from_percent(20);
            assert_eq!(twenty_percents.of(price), price::total_of(c(50)).is(q(1)));
        }
    }
    fn c(a: Amount) -> Coin<SubGroupTestC10> {
        Coin::<SubGroupTestC10>::from(a)
    }

    fn q(a: Amount) -> Coin<SuperGroupTestC1> {
        Coin::<SuperGroupTestC1>::from(a)
    }
}
