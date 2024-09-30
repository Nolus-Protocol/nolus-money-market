use std::marker::PhantomData;

use currency::Currency;

use crate::{coin::Amount, percent::Units as PercentUnits, price::Price, ratio::Ratio};

use super::Fractionable;

impl<C, QuoteC> Fractionable<PercentUnits> for Price<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    fn checked_mul<F>(self, fraction: &F) -> Option<Self>
    where
        F: Ratio<PercentUnits>,
    {
        Some(self.lossy_mul(&RatioUpcast(PhantomData, fraction)))
    }
}

impl<C, QuoteC> Fractionable<usize> for Price<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    fn checked_mul<F>(self, fraction: &F) -> Option<Self>
    where
        F: Ratio<usize>,
    {
        Some(self.lossy_mul(&RatioTryUpcast(fraction)))
    }
}

struct RatioUpcast<'a, U, R>(PhantomData<U>, &'a R)
where
    R: Ratio<U>;
impl<'a, U, R> Ratio<Amount> for RatioUpcast<'a, U, R>
where
    U: Into<Amount>,
    R: Ratio<U>,
{
    fn parts(&self) -> Amount {
        self.1.parts().into()
    }
    fn total(&self) -> Amount {
        self.1.total().into()
    }
}

struct RatioTryUpcast<'a, R>(&'a R)
where
    R: Ratio<usize>;

const EXPECT_MSG: &str = "usize should convert into u128";

impl<'a, R> Ratio<Amount> for RatioTryUpcast<'a, R>
where
    R: Ratio<usize>,
{
    fn parts(&self) -> Amount {
        self.0.parts().try_into().expect(EXPECT_MSG)
    }
    fn total(&self) -> Amount {
        self.0.total().try_into().expect(EXPECT_MSG)
    }
}

#[cfg(test)]
mod test {
    use currency::test::{SubGroupTestC10, SuperGroupTestC1};

    use crate::coin::{Amount, Coin};

    mod percent {
        use crate::fraction::Fraction;
        use crate::fractionable::price::test::{c, q};
        use crate::{percent::Percent, price};

        #[test]
        fn greater_than_one() {
            let price = price::total_of(c(1)).is(q(1000));
            let permille = Percent::from_permille(1);
            assert_eq!(permille.of(price).unwrap(), price::total_of(c(1)).is(q(1)));
        }

        #[test]
        fn less_than_one() {
            let price = price::total_of(c(10)).is(q(1));
            let twenty_percents = Percent::from_percent(20);
            assert_eq!(
                twenty_percents.of(price).unwrap(),
                price::total_of(c(50)).is(q(1))
            );
        }
    }
    mod usize_ratio {
        use currency::test::{SubGroupTestC10, SuperGroupTestC1};

        use crate::{
            coin::{Amount, Coin},
            fraction::Fraction,
            fractionable::price::test::{c, q},
            price,
            ratio::Rational,
        };

        #[test]
        fn greater_than_one() {
            test_impl(c(1), q(999), 2, 3, c(1), q(666));
            test_impl(c(2), q(Amount::MAX), 2, 1, c(1), q(Amount::MAX));
            // follow with rounding
            {
                let exp_q = 255211775190703847597530955573826158591; // (Amount::MAX * 3) >> 2;
                let exp_c = (2 * 4) >> 2;
                test_impl(c(2), q(Amount::MAX), 3, 4, c(exp_c), q(exp_q));
            }
            {
                let exp_q = 212676479325586539664609129644855132159; // (Amount::MAX * 5) >> 3;
                let exp_c = (2 * 4) >> 3;
                test_impl(c(2), q(Amount::MAX), 5, 4, c(exp_c), q(exp_q));
            }
        }

        #[test]
        fn less_than_one() {
            test_impl(c(150), q(1), 3, 2, c(100), q(1));
            test_impl(c(Amount::MAX), q(6), 2, 3, c(Amount::MAX), q(4));
            // follow with rounding
            let exp_c = 191408831393027885698148216680369618943; // (Amount::MAX * 9) >> 4;
            let exp_q = (8 * 4) >> 4;
            test_impl(c(Amount::MAX), q(8), 4, 9, c(exp_c), q(exp_q));
        }

        #[test]
        #[should_panic = "price overflow"]
        fn overflow() {
            test_impl(c(2), q(Amount::MAX), 9, 4, c(1), q(Amount::MAX));
        }

        #[track_caller]
        fn test_impl(
            amount1: Coin<SubGroupTestC10>,
            quote1: Coin<SuperGroupTestC1>,
            nominator: usize,
            denominator: usize,
            amount_exp: Coin<SubGroupTestC10>,
            quote_exp: Coin<SuperGroupTestC1>,
        ) {
            let price = price::total_of(amount1).is(quote1);
            let ratio = Rational::new(nominator, denominator);
            assert_eq!(
                Fraction::<usize>::of(&ratio, price).unwrap(),
                price::total_of(amount_exp).is(quote_exp)
            );
        }
    }
    fn c(a: Amount) -> Coin<SubGroupTestC10> {
        Coin::<SubGroupTestC10>::from(a)
    }

    fn q(a: Amount) -> Coin<SuperGroupTestC1> {
        Coin::<SuperGroupTestC1>::from(a)
    }
}
