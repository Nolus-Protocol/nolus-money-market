use crate::{contracts::HigherOrderType, validate::Validate};

use super::{
    super::{Map, TryForEach, TryForEachPair},
    Contracts,
};

impl<T> Map for Contracts<T> {
    type Unit = T;

    type HigherOrderType = super::HigherOrderType;

    fn try_map<Unit, Err, F>(
        self,
        mut f: F,
    ) -> Result<<Self::HigherOrderType as HigherOrderType>::Of<Unit>, Err>
    where
        F: FnMut(Self::Unit) -> Result<Unit, Err>,
    {
        Ok(Contracts {
            timealarms: f(self.timealarms)?,
            treasury: f(self.treasury)?,
        })
    }

    fn as_ref(&self) -> Contracts<&Self::Unit> {
        Contracts {
            timealarms: &self.timealarms,
            treasury: &self.treasury,
        }
    }
}

impl<T> TryForEach for Contracts<T> {
    type Item = T;

    fn try_for_each<F, Err>(self, f: F) -> Result<(), Err>
    where
        F: FnMut(Self::Item) -> Result<(), Err>,
    {
        [self.timealarms, self.treasury].into_iter().try_for_each(f)
    }
}

impl<T> TryForEachPair for Contracts<T> {
    type Item = T;

    type HigherOrderType = super::HigherOrderType;

    fn try_for_each_pair<CounterpartUnit, F, Err>(
        self,
        counterpart: Contracts<CounterpartUnit>,
        mut f: F,
    ) -> Result<(), Err>
    where
        F: FnMut(Self::Item, CounterpartUnit) -> Result<(), Err>,
    {
        [
            (self.timealarms, counterpart.timealarms),
            (self.treasury, counterpart.treasury),
        ]
        .into_iter()
        .try_for_each(|(unit, counter_part)| f(unit, counter_part))
    }
}

impl<T> Validate for Contracts<T>
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
