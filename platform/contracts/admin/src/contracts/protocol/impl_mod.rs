use platform::batch::Batch;
use sdk::cosmwasm_std::Addr;
use versioning::{ProtocolPackageRelease, ProtocolPackageReleaseId};

use crate::{result::Result, validate::Validate};

use super::{
    super::{
        impl_mod::migrate_contract, HigherOrderType, Map, MigrationSpec, TryForEach, TryForEachPair,
    },
    higher_order_type, Contracts, Protocol,
};

impl Contracts<Addr> {
    pub(crate) fn migrate_standalone(
        self,
        release: ProtocolPackageReleaseId,
        migration_msgs: Contracts<MigrationSpec>,
    ) -> Result<Batch> {
        let mut migration_batch = Batch::default();

        let mut post_migration_execute_batch = Batch::default();

        self.try_for_each_pair(migration_msgs, |address, migration_spec| {
            migrate_contract::<ProtocolPackageRelease>(
                &mut migration_batch,
                &mut post_migration_execute_batch,
                address,
                release.clone(),
                migration_spec,
            )
        })
        .map(|()| migration_batch.merge(post_migration_execute_batch))
    }
}

impl<T> Map for Contracts<T> {
    type Unit = T;

    type HigherOrderType = higher_order_type::Contracts;

    fn try_map<Unit, Err, F>(
        self,
        mut f: F,
    ) -> Result<<Self::HigherOrderType as HigherOrderType>::Of<Unit>, Err>
    where
        F: FnMut(Self::Unit) -> Result<Unit, Err>,
    {
        Ok(Contracts {
            leaser: f(self.leaser)?,
            lpp: f(self.lpp)?,
            oracle: f(self.oracle)?,
            profit: f(self.profit)?,
            reserve: f(self.reserve)?,
        })
    }

    fn as_ref(&self) -> Contracts<&T> {
        Contracts {
            leaser: &self.leaser,
            lpp: &self.lpp,
            oracle: &self.oracle,
            profit: &self.profit,
            reserve: &self.reserve,
        }
    }
}

impl<T> TryForEach for Contracts<T> {
    type Item = T;

    fn try_for_each<F, Err>(self, f: F) -> Result<(), Err>
    where
        F: FnMut(Self::Item) -> Result<(), Err>,
    {
        [
            self.leaser,
            self.lpp,
            self.oracle,
            self.profit,
            self.reserve,
        ]
        .into_iter()
        .try_for_each(f)
    }
}

impl<T> TryForEachPair for Contracts<T> {
    type Item = T;

    type HigherOrderType = higher_order_type::Contracts;

    fn try_for_each_pair<CounterpartUnit, F, Err>(
        self,
        counterpart: Contracts<CounterpartUnit>,
        mut f: F,
    ) -> Result<(), Err>
    where
        F: FnMut(T, CounterpartUnit) -> Result<(), Err>,
    {
        [
            (self.leaser, counterpart.leaser),
            (self.lpp, counterpart.lpp),
            (self.oracle, counterpart.oracle),
            (self.profit, counterpart.profit),
            (self.reserve, counterpart.reserve),
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

impl<T> Validate for Protocol<T>
where
    T: Validate,
{
    type Context<'r> = T::Context<'r>;

    type Error = T::Error;

    #[inline]
    fn validate(&self, ctx: Self::Context<'_>) -> Result<(), Self::Error> {
        self.contracts.validate(ctx)
    }
}
