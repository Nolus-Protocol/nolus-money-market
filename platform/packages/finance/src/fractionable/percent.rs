use crate::{
    coin::Coin,
    percent::{Percent, Units},
    ratio::Ratio,
};

use super::{Fractionable, HigherRank};

impl<T> HigherRank<T> for u32
where
    T: Into<Self>,
{
    type Type = u64;
    type Intermediate = Self;
}

impl Fractionable<Units> for Percent {
    #[track_caller]
    fn safe_mul(self, ratio: &Ratio<Units>) -> Self {
        Percent::from_permille(self.units().safe_mul(ratio))
    }
}

impl<C> Fractionable<Coin<C>> for Percent {
    #[track_caller]
    fn safe_mul(self, fraction: &Ratio<Coin<C>>) -> Self {
        let p128: u128 = self.units().into();
        // TODO re-assess the design of Ratio ... and whether it could be > 1
        let res: Units = p128
            .safe_mul(fraction)
            .try_into()
            .expect("overflow computing a fraction of permille");
        Self::from_permille(res)
    }
}

#[cfg(test)]
mod test {
    mod percent {
        use crate::{
            fractionable::HigherRank,
            percent::{Percent, Units},
        };

        #[test]
        fn of() {
            assert_eq!(
                Percent::from_permille(410 * 222 / 1000),
                Percent::from_percent(41).of(Percent::from_permille(222))
            );

            let p_units: Units = 410;
            let p64: <u32 as HigherRank<u8>>::Type = p_units.into();
            let p64_res: <u32 as HigherRank<u8>>::Type = p64 * u64::from(100 as Units) / 1000;
            let p_units_res: Units = p64_res.try_into().expect("u64 -> Units overflow");

            assert_eq!(
                Percent::from_permille(p_units_res),
                Percent::from_permille(50).of(Percent::from_percent(82))
            );
        }

        #[test]
        fn of_hundred_percent() {
            assert_eq!(
                Percent::from_permille(999),
                Percent::from_percent(100).of(Percent::from_permille(999))
            );
        }

        #[test]
        #[should_panic]
        fn of_overflow() {
            Percent::from_permille(1001).of(Percent::from_permille(Units::MAX));
        }
    }

    mod rational {
        use currency::test::SuperGroupTestC1;

        use crate::{coin::Coin, fractionable::Fractionable, percent::Percent, ratio::Ratio};

        #[test]
        fn safe_mul() {
            let ratio_one = Ratio::new(
                Coin::<SuperGroupTestC1>::new(u128::MAX),
                Coin::<SuperGroupTestC1>::new(u128::MAX),
            );
            assert_eq!(
                Percent::from_permille(899),
                Fractionable::<Coin<_>>::safe_mul(Percent::from_permille(899), &ratio_one)
            );
        }
    }
}
