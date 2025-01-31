use std::{collections::BTreeMap, convert::Infallible};

use platform::{batch::Batch, message::Response as MessageResponse};
use sdk::cosmwasm_std::{Addr, Binary, Storage, WasmMsg};
use versioning::{
    MigrationMessage, PlatformPackageRelease, ProtocolPackageRelease, ProtocolPackageReleaseId,
    SoftwareReleaseId, UpdatablePackage,
};

use crate::{
    error::Error,
    result::Result,
    state::contracts as state_contracts,
    validate::{Validate, ValidateValues},
};

use super::{
    Contracts, ContractsExecute, ContractsMigration, ContractsTemplate, ExecuteSpec, Granularity,
    HigherOrderType, MigrationSpec, Protocol, ProtocolContracts,
};

pub(crate) fn migrate(
    storage: &mut dyn Storage,
    release: SoftwareReleaseId,
    migration_spec: ContractsMigration,
) -> Result<MessageResponse> {
    load_and_run(storage, |contracts| {
        contracts.migrate(release, migration_spec).map(
            |Batches {
                 migration_batch,
                 post_migration_execute_batch,
             }| {
                MessageResponse::messages_only(migration_batch.merge(post_migration_execute_batch))
            },
        )
    })
}

pub(crate) fn execute(
    storage: &mut dyn Storage,
    execute_messages: ContractsExecute,
) -> Result<MessageResponse> {
    load_and_run(storage, |contracts| contracts.execute(execute_messages))
        .map(MessageResponse::messages_only)
}

pub(super) fn migrate_contract<Package>(
    migration_batch: &mut Batch,
    post_migration_execute_batch: &mut Batch,
    address: Addr,
    /* TODO Add field once deployed contracts can be queried about their version
        and release information.
    migrate_from: Package,
    */
    release: Package::ReleaseId,
    migration: MigrationSpec,
) -> Result<()>
where
    Package: UpdatablePackage,
{
    migration
        .post_migrate_execute_msg
        .map_or(const { Ok(()) }, |message| {
            execute_contract(post_migration_execute_batch, address.clone(), message)
        })
        .and_then(|()| {
            sdk::cosmwasm_std::to_json_vec(&MigrationMessage::<Package, _>::new(
                release,
                migration.migrate_msg,
            ))
            .map(|message| {
                migration_batch.schedule_execute_no_reply(WasmMsg::Migrate {
                    contract_addr: address.into_string(),
                    new_code_id: migration.code_id.u64(),
                    msg: Binary::new(message),
                })
            })
            .map_err(Into::into)
        })
}

impl Contracts {
    fn migrate(
        self,
        software_release: SoftwareReleaseId,
        migration_msgs: ContractsMigration,
    ) -> Result<Batches> {
        let mut migration_batch: Batch = Batch::default();

        let mut post_migration_execute_batch: Batch = Batch::default();

        match migration_msgs.platform {
            Granularity::Some { some: platform } => Some(platform),
            Granularity::All(platform) => platform.map(|platform| platform.map(Some)),
        }
        .map_or(const { Ok(()) }, |platform| {
            self.platform
                .try_for_each_pair(platform, |address, migration_spec| {
                    migration_spec.map_or(const { Ok(()) }, |migration_spec| {
                        migrate_contract::<PlatformPackageRelease>(
                            &mut migration_batch,
                            &mut post_migration_execute_batch,
                            address,
                            software_release.clone(),
                            migration_spec,
                        )
                    })
                })
        })?;

        Self::try_for_each_protocol(
            self.protocol,
            migration_msgs.protocol,
            |contracts, (protocol_release, protocol)| {
                match protocol {
                    Granularity::Some { some: protocol } => Some(protocol),
                    Granularity::All(protocol) => protocol.map(|protocol| protocol.map(Some)),
                }
                .map_or(const { Ok(()) }, |protocol| {
                    contracts.try_for_each_pair(protocol, |address, migration_spec| {
                        migration_spec.map_or(const { Ok(()) }, |migration_spec| {
                            migrate_contract::<ProtocolPackageRelease>(
                                &mut migration_batch,
                                &mut post_migration_execute_batch,
                                address,
                                ProtocolPackageReleaseId::new(
                                    software_release.clone(),
                                    protocol_release.clone(),
                                ),
                                migration_spec,
                            )
                        })
                    })
                })
            },
        )
        .map(|()| Batches {
            migration_batch,
            post_migration_execute_batch,
        })
    }

    fn execute(self, execute_msgs: ContractsExecute) -> Result<Batch> {
        let mut batch: Batch = Batch::default();

        match execute_msgs.platform {
            Granularity::Some { some: platform } => Some(platform),
            Granularity::All(platform) => platform.map(|platform| platform.map(Some)),
        }
        .map_or(const { Ok(()) }, |execute_spec| {
            self.platform
                .try_for_each_pair(execute_spec, |address, execute_spec| {
                    execute_spec.map_or(const { Ok(()) }, |migration_spec| {
                        execute_contract(&mut batch, address, migration_spec)
                    })
                })
        })?;

        Self::try_for_each_protocol(
            self.protocol,
            execute_msgs.protocol,
            |contracts, protocol| {
                match protocol {
                    Granularity::Some { some: protocol } => Some(protocol),
                    Granularity::All(protocol) => protocol.map(|protocol| protocol.map(Some)),
                }
                .map_or(const { Ok(()) }, |f| {
                    contracts.try_for_each_pair(f, |address, migration_spec| {
                        migration_spec.map_or(const { Ok(()) }, |migration_spec| {
                            execute_contract(&mut batch, address, migration_spec)
                        })
                    })
                })
            },
        )
        .map(|()| batch)
    }

    fn try_for_each_protocol<T, F>(
        protocols: BTreeMap<String, Protocol<Addr>>,
        mut counterparts: BTreeMap<String, T>,
        mut f: F,
    ) -> Result<()>
    where
        F: FnMut(ProtocolContracts<Addr>, T) -> Result<()>,
    {
        protocols
            .into_iter()
            .try_for_each(|(name, Protocol { contracts, .. })| {
                counterparts
                    .remove(&name)
                    .ok_or_else(|| Error::MissingProtocol(name))
                    .and_then(|protocol| f(contracts, protocol))
            })
    }
}

impl<Platform, Protocol, Unit> Validate for ContractsTemplate<Platform, Protocol, Unit>
where
    Platform: HigherOrderType,
    Platform::Of<Unit>: Validate,
    Protocol: HigherOrderType,
    Protocol::Of<Unit>: for<'r> Validate<
        Context<'r> = <Platform::Of<Unit> as Validate>::Context<'r>,
        Error = <Platform::Of<Unit> as Validate>::Error,
    >,
{
    type Context<'r> = <Platform::Of<Unit> as Validate>::Context<'r>;

    type Error = <Platform::Of<Unit> as Validate>::Error;

    fn validate(&self, ctx: Self::Context<'_>) -> Result<(), Self::Error> {
        self.platform
            .validate(ctx)
            .and_then(|()| ValidateValues::new(&self.protocol).validate(ctx))
    }
}

pub(super) trait Map
where
    Self: Sized,
{
    type Unit;

    type HigherOrderType: HigherOrderType<Of<Self::Unit> = Self>;

    #[inline]
    fn map<Unit, F>(self, mut f: F) -> <Self::HigherOrderType as HigherOrderType>::Of<Unit>
    where
        F: FnMut(Self::Unit) -> Unit,
    {
        match self.try_map(|unit| -> Result<Unit, Infallible> { Ok(f(unit)) }) {
            Ok(mapped) => mapped,
            Err(infallible) => match infallible {},
        }
    }

    fn try_map<Unit, Err, F>(
        self,
        f: F,
    ) -> Result<<Self::HigherOrderType as HigherOrderType>::Of<Unit>, Err>
    where
        F: FnMut(Self::Unit) -> Result<Unit, Err>;

    fn as_ref(&self) -> <Self::HigherOrderType as HigherOrderType>::Of<&Self::Unit>;
}

pub(super) trait TryForEach {
    type Item;

    fn try_for_each<F, Err>(self, f: F) -> Result<(), Err>
    where
        F: FnMut(Self::Item) -> Result<(), Err>;
}

pub(super) trait TryForEachPair {
    type Item;

    type HigherOrderType: HigherOrderType<Of<Self::Item> = Self>;

    fn try_for_each_pair<CounterpartUnit, F, Err>(
        self,
        counterpart: <Self::HigherOrderType as HigherOrderType>::Of<CounterpartUnit>,
        f: F,
    ) -> Result<(), Err>
    where
        F: FnMut(Self::Item, CounterpartUnit) -> Result<(), Err>;
}

fn load_and_run<F, R>(storage: &mut dyn Storage, f: F) -> Result<R>
where
    F: FnOnce(Contracts) -> Result<R>,
{
    state_contracts::load_all(storage).and_then(f)
}

fn execute_contract(
    batch: &mut Batch,
    address: Addr,
    ExecuteSpec { message }: ExecuteSpec,
) -> Result<()> {
    sdk::cosmwasm_std::to_json_vec(&message)
        .map(|message| {
            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: address.into_string(),
                msg: Binary::new(message),
                funds: vec![],
            })
        })
        .map_err(Into::into)
}

struct Batches {
    migration_batch: Batch,
    post_migration_execute_batch: Batch,
}
