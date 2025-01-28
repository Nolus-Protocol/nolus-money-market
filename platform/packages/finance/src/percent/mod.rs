use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult, Write},
    ops::{Add, Div, Rem, Sub},
};

use serde::{Deserialize, Serialize};

use sdk::{
    cosmwasm_std::{OverflowError, OverflowOperation},
    schemars::{self, JsonSchema},
};

use crate::{
    error::{Error, Result as FinanceResult},
    fractionable::Fractionable,
    ratio::{CheckedAdd, CheckedMul, Ratio, Rational},
    zero::Zero,
};

pub mod bound;

pub type Units = u32;

impl CheckedMul for Units {
    type Output = Self;

    fn checked_mul(self, rhs: Self) -> Option<Self::Output> {
        self.checked_mul(rhs)
    }
}

impl CheckedAdd for Units {
    type Output = Self;

    fn checked_add(self, rhs: Self) -> Option<Self::Output> {
        self.checked_add(rhs)
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[serde(transparent)]
pub struct Percent(Units); //value in permille

impl Percent {
    pub const ZERO: Self = Self::from_permille(0);
    pub const HUNDRED: Self = Self::from_permille(1000);

    const UNITS_TO_PERCENT_RATIO: Units = 10;

    pub fn from_percent(percent: u16) -> Self {
        debug_assert!(percent < 100);
        Self::from_permille(Units::from(percent) * Self::UNITS_TO_PERCENT_RATIO)
    }

    pub const fn from_permille(permille: Units) -> Self {
        Self(permille)
    }

    // pub fn from_ratio<FractionUnit>(nominator: FractionUnit, denominator: FractionUnit) -> Self
    // where
    //     FractionUnit: Copy + Debug + PartialEq + Zero + PartialOrd,
    //     Self: Fractionable<FractionUnit>,
    // {
    //     Rational::new(nominator, denominator).of(Percent::HUNDRED)
    // }

    pub const fn units(&self) -> Units {
        self.0
    }

    pub fn of<A>(&self, whole: A) -> A
    where
        A: Fractionable<Units>,
    {
        whole.safe_mul(&(*self).into())
    }

    pub fn is_zero(&self) -> bool {
        self == &Self::ZERO
    }

    pub fn checked_add(self, other: Self) -> FinanceResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self::from_permille)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Add).into())
    }

    pub fn checked_sub(self, other: Self) -> FinanceResult<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self::from_permille)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Sub).into())
    }
}

impl Zero for Percent {
    const ZERO: Self = Self::ZERO;
}

impl From<Percent> for Ratio<Units> {
    fn from(percent: Percent) -> Self {
        Self::new(percent.units(), Percent::HUNDRED.units())
    }
}

impl From<Percent> for Rational<Units> {
    fn from(percent: Percent) -> Self {
        Self::new(percent.units(), Percent::HUNDRED.units())
    }
}

impl TryFrom<Rational<Units>> for Percent {
    type Error = Error;

    fn try_from(rational: Rational<Units>) -> Result<Self, Self::Error> {
        // debug_assert_ne!(rational.denominator(), Zero::ZERO);

        rational
            .nominator()
            .checked_mul(Percent::HUNDRED.units())
            .and_then(|nominator| nominator.checked_div(rational.denominator()))
            .ok_or(Error::Overflow("Overflow occurred during conversion."))
            .and_then(|permille| {
                if permille > Percent::HUNDRED.units() {
                    Err(Error::UpperBoundCrossed {
                        bound: Percent::HUNDRED.units(),
                        value: permille,
                    })
                } else {
                    Ok(Percent::from_permille(permille))
                }
            })
    }
}

impl CheckedAdd for Percent {
    type Output = Self;

    fn checked_add(self, rhs: Self) -> Option<Self::Output> {
        let lhs: Rational<Units> = self.into();
        let rhs: Rational<Units> = rhs.into();

        lhs.checked_add(rhs).and_then(|res| res.try_into().ok())
    }
}

impl Display for Percent {
    #[track_caller]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let whole = (self.0) / Self::UNITS_TO_PERCENT_RATIO;
        let (no_fraction, overflow) = whole.overflowing_mul(Self::UNITS_TO_PERCENT_RATIO);
        debug_assert!(!overflow);
        let (fractional, overflow) = (self.0).overflowing_sub(no_fraction);
        debug_assert!(!overflow);

        f.write_fmt(format_args!("{}", whole))?;
        if fractional != Units::default() {
            f.write_fmt(format_args!(".{}", fractional))?;
        }
        f.write_char('%')?;
        Ok(())
    }
}

impl Add<Percent> for Percent {
    type Output = Self;

    #[track_caller]
    fn add(self, rhs: Self) -> Self {
        Self(
            self.0
                .checked_add(rhs.0)
                .expect("attempt to add with overflow"),
        )
    }
}

impl<'a> Add<&'a Percent> for Percent {
    type Output = Self;

    #[track_caller]
    fn add(self, rhs: &'a Percent) -> Self {
        self + *rhs
    }
}

impl Sub<Percent> for Percent {
    type Output = Self;

    #[track_caller]
    fn sub(self, rhs: Self) -> Self {
        Self(
            self.0
                .checked_sub(rhs.0)
                .expect("attempt to subtract with overflow"),
        )
    }
}

impl<'a> Sub<&'a Percent> for Percent {
    type Output = Self;

    #[track_caller]
    fn sub(self, rhs: &'a Percent) -> Self {
        self - *rhs
    }
}

impl Div for Percent {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Percent(self.0 / rhs.0)
    }
}

impl Rem for Percent {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        Percent(self.0 % rhs.0)
    }
}

#[cfg(test)]
pub(super) mod test {
    use std::{
        fmt::{Debug, Display},
        ops::{Div, Mul},
    };

    use currency::test::SubGroupTestC10;

    use crate::{coin::Coin, fractionable::Percentable, percent::Percent, ratio::Rational};

    use super::Units;

    #[test]
    fn from_percent() {
        assert_eq!(Percent::from_percent(0), Percent(0));
        assert_eq!(Percent::from_percent(10), Percent(100));
    }

    #[test]
    fn from_permille() {
        assert_eq!(Percent::from_permille(0), Percent(0));
        assert_eq!(Percent::from_permille(10), Percent(10));
    }

    // #[test]
    // fn from_ratio() {
    //     let a1 = 0;
    //     let a2 = 5000;
    //     let a3 = 1352;
    //     let c1 = Coin::<SubGroupTestC10>::new(a1);
    //     let c2 = Coin::<SubGroupTestC10>::new(a2);
    //     let c3 = Coin::<SubGroupTestC10>::new(a3);
    //     assert_eq!(Percent::ZERO, Percent::from_ratio(c1, c2));

    //     assert_eq!(from_parts(a3, a2), Percent::from_ratio(c3, c2));

    //     assert_eq!(Percent::HUNDRED, Percent::from_ratio(c3, c3));

    //     assert_eq!(from_parts(a2, a3), Percent::from_ratio(c2, c3));
    // }

    #[test]
    fn test_zero() {
        assert_eq!(
            Coin::<SubGroupTestC10>::new(0),
            Percent::ZERO.of(Coin::<SubGroupTestC10>::new(10))
        )
    }

    #[test]
    fn test_hundred() {
        let amount = 123;
        assert_eq!(
            Coin::<SubGroupTestC10>::new(amount),
            Percent::HUNDRED.of(Coin::<SubGroupTestC10>::new(amount))
        )
    }

    #[test]
    fn add() {
        assert_eq!(from(40), from(25) + from(15));
        assert_eq!(from(39), from(0) + from(39));
        assert_eq!(from(39), from(39) + from(0));
        assert_eq!(from(1001), Percent::HUNDRED + from(1));
        assert_eq!(from(1) + Percent::HUNDRED, Percent::HUNDRED + from(1));
        assert_eq!(from(Units::MAX), from(Units::MAX) + from(0));
    }

    #[test]
    #[should_panic]
    fn add_overflow() {
        let _ = from(Units::MAX) + from(1);
    }

    #[test]
    fn sub() {
        assert_eq!(from(67), from(79) - from(12));
        assert_eq!(from(0), from(34) - from(34));
        assert_eq!(from(39), from(39) - from(0));
        assert_eq!(from(990), Percent::HUNDRED - from(10));
        assert_eq!(from(0), from(Units::MAX) - from(Units::MAX));
    }

    #[test]
    #[should_panic]
    fn sub_overflow() {
        let _ = from(34) - from(35);
    }

    #[test]
    fn display() {
        test_display("0%", 0);
        test_display("0.1%", 1);
        test_display("0.4%", 4);
        test_display("1%", 10);
        test_display("1.9%", 19);
        test_display("9%", 90);
        test_display("10.1%", 101);
        test_display("100%", 1000);
        test_display("1234567.8%", 12345678);
    }

    #[test]
    fn of() {
        test_of(100, Percent::from_percent(40), Percent::from_percent(4));
        test_of(100, Percent::from_percent(40), Percent::from_permille(40));
        test_of(10, Percent::from_percent(800), Percent::from_percent(8));
        test_of(10, Percent::from_permille(8900), Percent::from_permille(89));
        test_of(1, Percent::from_percent(12300), Percent::from_permille(123));
        test_of(1, Percent::from_percent(12345), Percent::from_permille(123));
        test_of(0, Percent::from_percent(123), Percent::from_percent(0));
        test_of(
            1000,
            Percent::from_permille(Units::MAX),
            Percent::from_permille(Units::MAX),
        );
        test_of(
            2000,
            Percent::from_permille(Units::MAX / 2),
            Percent::from_permille(Units::MAX - 1),
        );
        test_of(1000, Percent::HUNDRED, Percent::HUNDRED);
        test_of(100, Percent::ZERO, Percent::ZERO);
    }

    #[test]
    #[should_panic]
    fn of_overflow() {
        Percent::from_permille(1001).of(Percent::from_permille(Units::MAX));
    }

    #[test]
    fn rational_of_percents() {
        let v = 14u32;
        let r = Rational::new(Percent::HUNDRED, Percent::HUNDRED);
        assert_eq!(v, r.checked_mul(v).unwrap());
        // assert_eq!(v, Ratio::<Units>::of(&r, v));
    }

    #[test]
    fn rational_to_percents() {
        let n: Units = 189;
        let d: Units = 1890;
        let r = Rational::new(n, d);
        let res: Percent = r.checked_mul(Percent::HUNDRED).unwrap();
        assert_eq!(Percent::from_permille(n * 1000 / d), res);
    }

    pub(crate) fn test_of<P>(permille: Units, quantity: P, exp: P)
    where
        P: Percentable + PartialEq + Debug + Clone + Display,
    {
        let perm = Percent::from_permille(permille);
        assert_eq!(
            exp,
            perm.of(quantity.clone()),
            "Calculating {} of {}",
            perm,
            quantity
        );
    }

    fn from(permille: Units) -> Percent {
        Percent::from_permille(permille)
    }

    fn from_parts<U>(nom: U, denom: U) -> Percent
    where
        U: TryInto<Units>,
        Units: Into<U>,
        U: Mul<U, Output = U> + Div<U, Output = U>,
        <U as TryInto<Units>>::Error: Debug,
    {
        let exp = nom * Percent::HUNDRED.units().into() / denom;
        from(exp.try_into().expect("valid test data"))
    }

    fn test_display(exp: &str, permilles: Units) {
        assert_eq!(exp, format!("{}", Percent::from_permille(permilles)));
    }
}
