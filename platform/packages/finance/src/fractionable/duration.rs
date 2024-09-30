use sdk::cosmwasm_std::{Uint128, Uint256};

use crate::{coin::Coin, duration::Duration, ratio::Ratio};

use super::{Fractionable, HigherRank};

impl<T> HigherRank<T> for u128
where
    T: Into<Self>,
{
    type Type = Uint256;
    type Intermediate = Uint128;
}

impl<C> Fractionable<Coin<C>> for Duration {
    #[track_caller]
    fn checked_mul<F>(self, fraction: &F) -> Option<Self>
    where
        F: Ratio<Coin<C>>,
    {
        // TODO re-assess the design of Ratio ... and whether it could be > 1
        let d128: u128 = self.into();
        Fractionable::<Coin<C>>::checked_mul(d128, fraction)
            .and_then(|may_product| may_product.try_into().ok())
    }
}

#[cfg(test)]
mod tests {
    use currency::test::SuperGroupTestC1;

    use crate::{coin::Coin, duration::Duration, fractionable::Fractionable, ratio::Rational};

    #[test]
    fn checked_mul() {
        let d = Duration::from_secs(10);
        let res = d
            .checked_mul(&Rational::new(
                Coin::<SuperGroupTestC1>::new(10),
                Coin::<SuperGroupTestC1>::new(20),
            ))
            .unwrap();
        assert_eq!(Duration::from_secs(5), res);
    }

    #[test]
    fn checked_mul_max() {
        let d = Duration::from_secs(10);
        let res = d
            .checked_mul(&Rational::new(
                Coin::<SuperGroupTestC1>::new(u128::MAX),
                Coin::<SuperGroupTestC1>::new(u128::MAX / 2),
            ))
            .unwrap();
        assert_eq!(Duration::from_secs(20), res);
    }

    #[test]
    fn checked_mul_overflow() {
        let d = Duration::from_secs(10);
        let res = d.checked_mul(&Rational::new(
            Coin::<SuperGroupTestC1>::new(u128::MAX / 2),
            Coin::<SuperGroupTestC1>::new(1),
        ));

        assert_eq!(None, res);
    }
}
