use crate::validate::Validate;

use super::{
    super::{AsRef, ForEachPair, TryForEach},
    PlatformContracts,
};

impl<T> AsRef for PlatformContracts<T> {
    type Item = T;

    type HigherOrderType = super::HigherOrderType;

    fn as_ref(&self) -> PlatformContracts<&Self::Item> {
        PlatformContracts {
            timealarms: &self.timealarms,
            treasury: &self.treasury,
        }
    }
}

impl<T> TryForEach for PlatformContracts<T> {
    type Item = T;

    fn try_for_each<F, Err>(self, f: F) -> Result<(), Err>
    where
        F: FnMut(Self::Item) -> Result<(), Err>,
    {
        [self.timealarms, self.treasury].into_iter().try_for_each(f)
    }
}

impl<T> ForEachPair for PlatformContracts<T> {
    type Item = T;

    type HigherOrderType = super::HigherOrderType;

    fn for_each_pair<CounterpartUnit, F>(
        self,
        counterpart: PlatformContracts<CounterpartUnit>,
        mut f: F,
    ) where
        F: FnMut(Self::Item, CounterpartUnit),
    {
        [
            (self.timealarms, counterpart.timealarms),
            (self.treasury, counterpart.treasury),
        ]
        .into_iter()
        .for_each(|(unit, counter_part)| f(unit, counter_part))
    }
}

impl<T> Validate for PlatformContracts<T>
where
    T: Validate,
{
    type Context<'r> = T::Context<'r>;

    type Error = T::Error;

    fn validate(&self, ctx: Self::Context<'_>) -> Result<(), Self::Error> {
        self.as_ref()
            .try_for_each(|contract| contract.validate(ctx))
    }
}
