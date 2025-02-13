use std::{
    fmt::Debug,
    ops::{Div, Mul},
};

use crate::{
    duration::Units as TimeUnits, percent::Units as PercentUnits, ratio::Ratio, zero::Zero,
};

mod coin;
mod duration;
mod percent;
mod price;
mod usize;

pub trait Fractionable<U> {
    #[track_caller]
    fn safe_mul<F>(self, fraction: &F) -> Self
    where
        F: Ratio<U>;
}

// TODO revisit its usability
pub trait Percentable: Fractionable<PercentUnits> {}
pub trait TimeSliceable: Fractionable<TimeUnits> {}

pub trait HigherRank<T> {
    type Type;
    // An intermediate type to handle cases when there is no TryInto<Self> for HigherRank::Type but
    // instead there is TryInto<HigherRank::Intermediate> for HigherRank::Type, and Into<Self> for HigherRank::Intermediate
    type Intermediate;
}

pub trait CheckedMultiply<U> {
    #[track_caller]
    fn checked_mul(self, parts: U, total: U) -> Option<Self>
    where
        U: Zero + Debug + PartialEq<U>,
        Self: Sized;
}

impl<T, D, DIntermediate, U> CheckedMultiply<U> for T
where
    T: HigherRank<U, Type = D, Intermediate = DIntermediate> + Into<D>,
    D: TryInto<DIntermediate>,
    DIntermediate: Into<T>,
    D: Mul<D, Output = D> + Div<D, Output = D>,
    U: Zero + PartialEq + Into<D> + Debug,
{
    fn checked_mul(self, parts: U, total: U) -> Option<Self> {
        if parts == total {
            Some(self)
        } else {
            let res_double: D = self.into() * parts.into();
            let res_double = res_double / total.into();
            res_double
                .try_into()
                .ok()
                .map(|res_intermediate: DIntermediate| res_intermediate.into())
        }
    }
}

impl<T, D, DIntermediate, U> Fractionable<U> for T
where
    T: HigherRank<U, Type = D, Intermediate = DIntermediate> + Into<D> + CheckedMultiply<U>,
    D: TryInto<DIntermediate>,
    <D as TryInto<DIntermediate>>::Error: Debug,
    DIntermediate: Into<T>,
    D: Mul<D, Output = D> + Div<D, Output = D>,
    U: Zero + PartialEq + Into<D> + Debug,
{
    #[track_caller]
    fn safe_mul<R>(self, ratio: &R) -> Self
    where
        R: Ratio<U>,
    {
        // TODO debug_assert_eq!(T::BITS * 2, D::BITS);
        self.checked_mul(ratio.parts(), ratio.total())
            .expect("unexpected overflow")
    }
}

impl<T> Percentable for T where T: Fractionable<PercentUnits> {}
impl<T> TimeSliceable for T where T: Fractionable<TimeUnits> {}
