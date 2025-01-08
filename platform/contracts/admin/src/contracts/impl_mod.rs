use std::collections::BTreeMap;

use platform::{batch::Batch, message::Response as MessageResponse};
use sdk::cosmwasm_std::{Addr, Binary, Storage, WasmMsg};
use versioning::Id;

use crate::{
    error::Error,
    msg::ExecuteMsg,
    result::Result,
    state::{contract::Contract as ContractState, contracts as state_contracts},
    validate::{Validate, ValidateValues},
};

use super::{
    Contracts, ContractsExecute, ContractsMigration, ContractsTemplate, Granularity,
    HigherOrderType, MigrationSpec, Protocol, ProtocolContracts,
};

pub(crate) fn migrate(
    storage: &mut dyn Storage,
    admin_contract_addr: Addr,
    release: Id,
    migration_spec: ContractsMigration,
) -> Result<MessageResponse> {
    ContractState::AwaitContractsMigrationReply { release }.store(storage)?;

    load_and_run(storage, |contracts| {
        contracts.migrate(migration_spec).and_then(
            |Batches {
                 mut migration_batch,
                 post_migration_execute_batch,
             }| {
                migration_batch
                    .schedule_execute_wasm_no_reply_no_funds(
                        admin_contract_addr,
                        &ExecuteMsg::EndOfMigration {},
                    )
                    .map(|()| {
                        MessageResponse::messages_only(
                            migration_batch.merge(post_migration_execute_batch),
                        )
                    })
                    .map_err(Into::into)
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

pub(super) fn migrate_contract(
    migration_batch: &mut Batch,
    post_migration_execute_batch: &mut Batch,
    address: Addr,
    migrate: MigrationSpec,
) {
    if let Some(post_migrate_execute_msg) = migrate.post_migrate_execute_msg {
        execute_contract(
            post_migration_execute_batch,
            address.clone(),
            post_migrate_execute_msg,
        )
    }

    migration_batch.schedule_execute_reply_on_success(
        WasmMsg::Migrate {
            contract_addr: address.into_string(),
            new_code_id: migrate.code_id.u64(),
            msg: Binary::new(migrate.migrate_msg.into_bytes()),
        },
        0,
    );
}

impl Contracts {
    fn migrate(self, migration_msgs: ContractsMigration) -> Result<Batches> {
        let mut migration_batch: Batch = Batch::default();

        let mut post_migration_execute_batch: Batch = Batch::default();

        match migration_msgs.platform {
            Granularity::Some { some: platform } => self.platform.maybe_migrate(
                &mut migration_batch,
                &mut post_migration_execute_batch,
                platform,
            ),
            Granularity::All(Some(platform)) => self.platform.migrate(
                &mut migration_batch,
                &mut post_migration_execute_batch,
                platform,
            ),
            Granularity::All(None) => {}
        }

        Self::try_for_each_protocol(
            self.protocol,
            migration_msgs.protocol,
            |contracts, protocol| match protocol {
                Granularity::Some { some: protocol } => contracts.maybe_migrate(
                    &mut migration_batch,
                    &mut post_migration_execute_batch,
                    protocol,
                ),
                Granularity::All(Some(protocol)) => contracts.migrate(
                    &mut migration_batch,
                    &mut post_migration_execute_batch,
                    protocol,
                ),
                Granularity::All(None) => {}
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
            Granularity::Some { some: platform } => {
                self.platform.maybe_execute(&mut batch, platform)
            }
            Granularity::All(Some(platform)) => self.platform.execute(&mut batch, platform),
            Granularity::All(None) => {}
        }

        Self::try_for_each_protocol(
            self.protocol,
            execute_msgs.protocol,
            |contracts, protocol| match protocol {
                Granularity::Some { some: protocol } => {
                    contracts.maybe_execute(&mut batch, protocol)
                }
                Granularity::All(Some(protocol)) => contracts.execute(&mut batch, protocol),
                Granularity::All(None) => {}
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
        F: FnMut(ProtocolContracts<Addr>, T),
    {
        protocols
            .into_iter()
            .try_for_each(|(name, Protocol { contracts, .. })| {
                counterparts
                    .remove(&name)
                    .ok_or_else(|| Error::MissingProtocol(name))
                    .map(|protocol| f(contracts, protocol))
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

    fn validate(&self, ctx: Self::Context<'_>) -> ::std::result::Result<(), Self::Error> {
        self.platform
            .validate(ctx)
            .and_then(|()| ValidateValues::new(&self.protocol).validate(ctx))
    }
}

pub(super) trait AsRef {
    type Item;

    type HigherOrderType: HigherOrderType<Of<Self::Item> = Self>;

    fn as_ref(&self) -> <Self::HigherOrderType as HigherOrderType>::Of<&Self::Item>;
}

pub(super) trait TryForEach {
    type Item;

    fn try_for_each<U, F, E>(self, accumulator: U, functor: F) -> std::result::Result<U, E>
    where
        F: FnMut(Self::Item, U) -> Result<U, E>;
}

pub(super) trait ForEachPair {
    type Item;

    type HigherOrderType: HigherOrderType<Of<Self::Item> = Self>;

    fn for_each_pair<U, V, F>(
        self,
        counter_part: <Self::HigherOrderType as HigherOrderType>::Of<U>,
        accumulator: V,
        functor: F,
    ) -> V
    where
        F: FnMut(Self::Item, U, V) -> V;
}

trait MigrateContracts: ForEachPair<Item = Addr> + Sized {
    fn migrate(
        self,
        migration_batch: &mut Batch,
        post_migration_execute_batch: &mut Batch,
        migration_msgs: <Self::HigherOrderType as HigherOrderType>::Of<MigrationSpec>,
    ) {
        () = self.for_each_pair(migration_msgs, (), |address, migration_spec, ()| {
            () = migrate_contract(
                migration_batch,
                post_migration_execute_batch,
                address,
                migration_spec,
            );
        });
    }

    fn maybe_migrate(
        self,
        migration_batch: &mut Batch,
        post_migration_execute_batch: &mut Batch,
        migration_msgs: <Self::HigherOrderType as HigherOrderType>::Of<Option<MigrationSpec>>,
    ) {
        () = self.for_each_pair(migration_msgs, (), |address, migration_spec, ()| {
            if let Some(migration_spec) = migration_spec {
                () = migrate_contract(
                    migration_batch,
                    post_migration_execute_batch,
                    address,
                    migration_spec,
                )
            }
        });
    }
}

trait ExecuteContracts: ForEachPair<Item = Addr> + Sized {
    fn execute(
        self,
        batch: &mut Batch,
        execute_messages: <Self::HigherOrderType as HigherOrderType>::Of<String>,
    ) {
        () = self.for_each_pair(execute_messages, (), |address, migration_spec, ()| {
            () = execute_contract(batch, address, migration_spec);
        });
    }

    fn maybe_execute(
        self,
        batch: &mut Batch,
        execute_messages: <Self::HigherOrderType as HigherOrderType>::Of<Option<String>>,
    ) {
        () = self.for_each_pair(execute_messages, (), |address, migration_spec, ()| {
            if let Some(migration_spec) = migration_spec {
                () = execute_contract(batch, address, migration_spec);
            }
        });
    }
}

impl<T: ForEachPair<Item = Addr>> MigrateContracts for T {}

impl<T: ForEachPair<Item = Addr>> ExecuteContracts for T {}

fn load_and_run<F, R>(storage: &mut dyn Storage, f: F) -> Result<R>
where
    F: FnOnce(Contracts) -> Result<R>,
{
    state_contracts::load_all(storage).and_then(f)
}

fn execute_contract(batch: &mut Batch, address: Addr, execute_message: String) {
    batch.schedule_execute_no_reply(WasmMsg::Execute {
        contract_addr: address.into_string(),
        msg: Binary::new(execute_message.into_bytes()),
        funds: vec![],
    });
}

struct Batches {
    migration_batch: Batch,
    post_migration_execute_batch: Batch,
}
