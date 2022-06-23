use std::{
    fmt::{Debug, Display, Formatter, Result, Write},
    ops::{Add, Sub},
};

use cosmwasm_std::{OverflowError, OverflowOperation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{error::Result as FinanceResult, fractionable::Percentable, ratio::Ratio};

pub type Units = u32;

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
        Self::from_permille(Units::from(percent) * Self::UNITS_TO_PERCENT_RATIO)
    }

    pub const fn from_permille(permille: Units) -> Self {
        Self(permille)
    }

    pub(crate) fn units(&self) -> Units {
        self.0
    }

    pub fn of<P>(&self, amount: P) -> P
    where
        P: Percentable,
    {
        Ratio::from(*self).of(amount)
    }

    /// the inverse of `Percent::of`
    /// If %.of(X) -> Y, then %.are(Y) -> X
    /// :pre self != 0
    pub fn are<P>(&self, amount: P) -> P
    where
        P: Percentable,
    {
        use cosmwasm_std::Fraction;
        debug_assert!(self != &Self::ZERO);
        Ratio::from(*self)
            .inv()
            .expect("precondition not respected")
            .of(amount)
    }

    pub fn checked_add(self, other: Self) -> FinanceResult<Self> {
        self.0
            .checked_add(other.0)
            .map(Self::from_permille)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Add, self, other).into())
    }

    pub fn checked_sub(self, other: Self) -> FinanceResult<Self> {
        self.0
            .checked_sub(other.0)
            .map(Self::from_permille)
            .ok_or_else(|| OverflowError::new(OverflowOperation::Sub, self, other).into())
    }
}

impl From<Percent> for Ratio<Units> {
    fn from(p: Percent) -> Self {
        Self::new(p.units(), Percent::HUNDRED.units())
    }
}

impl Display for Percent {
    fn fmt(&self, f: &mut Formatter) -> Result {
        let whole = (self.0) / Self::UNITS_TO_PERCENT_RATIO;
        let fractional = (self.0)
            .checked_rem(Self::UNITS_TO_PERCENT_RATIO)
            .expect("zero divider");

        f.write_str(&whole.to_string())?;
        if fractional != Units::default() {
            f.write_char('.')?;
            f.write_str(&fractional.to_string())?;
        }
        f.write_char('%')?;
        Ok(())
    }
}

impl Add<Percent> for Percent {
    type Output = Self;

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

    fn add(self, rhs: &'a Percent) -> Self {
        self + *rhs
    }
}

impl Sub<Percent> for Percent {
    type Output = Self;

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

    fn sub(self, rhs: &'a Percent) -> Self {
        self - *rhs
    }
}

#[cfg(test)]
pub(super) mod test {
    use std::fmt::{Debug, Display};

    use crate::{coin::Coin, currency::Nls, percent::Percent, fractionable::Percentable};

    use super::Units;

    fn from(permille: Units) -> Percent {
        Percent::from_permille(permille)
    }

    #[test]
    fn from_percent() {
        assert_eq!(Percent(0), Percent::from_percent(0));
        assert_eq!(Percent(100), Percent::from_percent(10));
    }

    #[test]
    fn from_permille() {
        assert_eq!(Percent(0), Percent::from_permille(0));
        assert_eq!(Percent(10), Percent::from_permille(10));
    }

    #[test]
    fn test_zero() {
        assert_eq!(Coin::<Nls>::new(0), Percent::ZERO.of(Coin::<Nls>::new(10)))
    }

    #[test]
    fn test_hundred() {
        let amount = 123;
        assert_eq!(
            Coin::<Nls>::new(amount),
            Percent::HUNDRED.of(Coin::<Nls>::new(amount))
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

    pub(crate) fn test_of_are<P>(permille: Units, quantity: P, exp: P)
    where
        P: Percentable + PartialEq + Debug + Clone + Display,
    {
        test_of(permille, quantity.clone(), exp.clone());
        test_are(permille, exp, quantity);
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

    pub(crate) fn test_are<P>(permille: Units, quantity: P, exp: P)
    where
        P: Percentable + PartialEq + Debug + Clone + Display,
    {
        let perm = Percent::from_permille(permille);

        assert_eq!(
            exp,
            perm.are(quantity.clone()),
            "Calculating {} of X are {}",
            perm,
            quantity
        );
    }

    fn test_display(exp: &str, permilles: Units) {
        assert_eq!(exp, format!("{}", Percent::from_permille(permilles)));
    }
}
