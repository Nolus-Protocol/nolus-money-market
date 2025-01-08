use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    fraction::Fraction,
    fractionable::Fractionable,
    percent::{Percent, Units},
    zero::Zero,
};

// TODO review whether it may gets simpler if extend Fraction
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Ratio<U> {
    parts: U,
    total: U,
}

impl<U> Ratio<U>
where
    U: Copy + PartialEq + PartialOrd<U>,
{
    pub fn new(parts: U, total: U) -> Self {
        debug_assert!(parts < total);

        Self { parts, total }
    }

    pub fn parts(&self) -> U {
        self.parts
    }

    pub fn total(&self) -> U {
        self.total
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Rational<U> {
    nominator: U,
    denominator: U,
}

impl<U> Rational<U>
where
    U: Zero + Debug + PartialEq<U>,
{
    #[track_caller]
    pub fn new(nominator: U, denominator: U) -> Self {
        debug_assert_ne!(denominator, Zero::ZERO);

        Self {
            nominator,
            denominator,
        }
    }
}

impl<U, T> Fraction<U> for Rational<T>
where
    U: Copy + PartialOrd,
    T: Copy + Into<U>,
{
    #[track_caller]
    fn of<A>(&self, whole: A) -> A
    where
        A: Fractionable<U>,
    {
        whole.safe_mul(&(*self).into())
    }
}

impl<U, T> From<Rational<T>> for Ratio<U>
where
    U: Copy + PartialOrd,
    T: Into<U>,
{
    fn from(ratio: Rational<T>) -> Self {
        Ratio::new(ratio.nominator.into(), ratio.denominator.into())
    }
}

impl From<Rational<Percent>> for Ratio<Units> {
    fn from(rational: Rational<Percent>) -> Self {
        Ratio::new(rational.nominator.units(), rational.denominator.units())
    }
}
