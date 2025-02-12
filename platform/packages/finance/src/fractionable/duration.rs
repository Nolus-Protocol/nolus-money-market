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
    fn safe_mul(self, fraction: &Ratio<Coin<C>>) -> Self {
        let d128: u128 = self.into();
        d128.safe_mul(fraction)
            .try_into()
            .expect("overflow computing a fraction of duration")
    }
}

#[cfg(test)]
mod tests {
    use currency::test::SuperGroupTestC1;

    use crate::{coin::Coin, duration::Duration, fractionable::Fractionable, ratio::Ratio};

    #[test]
    fn safe_mul() {
        let d = Duration::from_secs(10);
        let res = d.safe_mul(&Ratio::new(
            Coin::<SuperGroupTestC1>::new(10),
            Coin::<SuperGroupTestC1>::new(20),
        ));
        assert_eq!(Duration::from_secs(5), res);
    }

    #[test]
    fn safe_mul_max() {
        let d = Duration::from_secs(10);
        let res = d.safe_mul(&Ratio::new(
            Coin::<SuperGroupTestC1>::new(u128::MAX / 2),
            Coin::<SuperGroupTestC1>::new(u128::MAX),
        ));
        assert_eq!(Duration::from_secs(5) - Duration::from_nanos(1), res);
    }
}
