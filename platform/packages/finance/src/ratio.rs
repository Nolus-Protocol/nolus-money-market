use std::{
    fmt::Debug,
    ops::{Div, Rem},
};

use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{fractionable::Fractionable, zero::Zero};

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
        debug_assert!(parts <= total);

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
    U: Zero + Debug + PartialEq<U> + Copy,
{
    #[track_caller]
    pub fn new(nominator: U, denominator: U) -> Self {
        debug_assert_ne!(denominator, Zero::ZERO);

        Self {
            nominator,
            denominator,
        }
    }

    pub fn nominator(&self) -> U {
        self.nominator
    }

    pub fn denominator(&self) -> U {
        self.denominator
    }
}

pub trait CheckedMul<Rhs = Self> {
    type Output;

    fn checked_mul(self, rhs: Rhs) -> Option<Self::Output>;
}

pub trait CheckedAdd<Rhs = Self> {
    type Output;

    fn checked_add(self, rhs: Rhs) -> Option<Self::Output>;
}

impl<U> Rational<U>
where
    U: Zero + Debug + PartialEq<U> + Copy + PartialOrd + Div<Output = U> + Rem<Output = U>,
{
    // Multiplication of Rational > 1.
    pub fn checked_mul<F>(self, rhs: F) -> Option<F>
    where
        U: CheckedMul<F, Output = F>,
        F: Fractionable<U> + CheckedAdd<F, Output = F> + Copy,
    {
        // Rational(a,b).checked_mul(c) = (a / b).checked_mul(c) + c.safe_mul(Rational(a % b, b))
        (self.nominator / self.denominator)
            .checked_mul(rhs)
            .and_then(|whole_part: F| {
                let fraction_part = rhs.safe_mul(&Ratio::new(
                    self.nominator % self.denominator,
                    self.denominator,
                ));
                whole_part.checked_add(fraction_part)
            })
    }
}

impl<U> CheckedAdd for Rational<U>
where
    U: Zero + Debug + PartialEq<U> + CheckedMul<Output = U> + CheckedAdd<Output = U> + Copy,
{
    type Output = Self;

    fn checked_add(self, rhs: Self) -> Option<Self::Output> {
        self.denominator
            .checked_mul(rhs.denominator)
            .and_then(|common_denominator| {
                self.nominator
                    .checked_mul(rhs.denominator)
                    .and_then(|scaled_left_nom| {
                        rhs.nominator
                            .checked_mul(self.denominator)
                            .and_then(|scaled_right_nom| {
                                scaled_left_nom
                                    .checked_add(scaled_right_nom)
                                    .map(|nominator| Rational::new(nominator, common_denominator))
                            })
                    })
            })
    }
}
