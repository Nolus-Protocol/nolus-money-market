use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use ::platform::contract::CodeId;
use json_value::JsonValue;
use sdk::cosmwasm_std::{Addr, Uint64};
use versioning::ReleaseId;

#[cfg(feature = "contract")]
pub(crate) use self::impl_mod::{execute, migrate};
pub use self::{
    granular::{Granularity, HigherOrderType as HigherOrderGranularity},
    higher_order_type::{
        Compose as HigherOrderCompose, HigherOrderTuple, HigherOrderType,
        Option as HigherOrderOption,
    },
    platform::{
        higher_order_type::{
            Contracts as HigherOrderPlatformContracts,
            ContractsWithoutAdmin as HigherOrderPlatformContractsWithoutAdmin,
        },
        Contracts as PlatformContracts, ContractsWithoutAdmin as PlatformContractsWithoutAdmin,
    },
    protocol::{
        higher_order_type::{
            Contracts as HigherOrderProtocolContracts, Protocol as HigherOrderProtocol,
        },
        Contracts as ProtocolContracts, Dex, Network, Protocol,
    },
};

mod granular;
mod higher_order_type;
#[cfg(feature = "contract")]
mod impl_mod;
mod platform;
mod protocol;

pub type PlatformContractAddressesWithoutAdmin = PlatformContractsWithoutAdmin<Addr>;

#[cfg(feature = "contract")]
pub type PlatformContractAddresses = PlatformContracts<Addr>;

pub type ProtocolContractAddresses = ProtocolContracts<Addr>;

pub type HigherOrderGranularOptional<T> = HigherOrderGranularity<T, HigherOrderOption>;

pub type HigherOrderGranularOptionalPlatformContractsWithoutAdmin =
    HigherOrderGranularOptional<HigherOrderPlatformContractsWithoutAdmin>;

pub type HigherOrderGranularOptionalPlatformContracts =
    HigherOrderGranularOptional<HigherOrderPlatformContracts>;

pub type HigherOrderGranularOptionalProtocolContracts =
    HigherOrderGranularOptional<HigherOrderProtocolContracts>;

pub type HigherOrderPlatformMigration = HigherOrderGranularOptionalPlatformContracts;

#[cfg(feature = "contract")]
pub type PlatformMigration = <HigherOrderPlatformMigration as HigherOrderType>::Of<MigrationSpec>;

pub type HigherOrderProtocolMigration = HigherOrderCompose<
    HigherOrderTuple<false, ReleaseId>,
    HigherOrderGranularOptionalProtocolContracts,
>;

#[cfg(feature = "contract")]
pub type ProtocolMigration = <HigherOrderProtocolMigration as HigherOrderType>::Of<MigrationSpec>;

pub type HigherOrderPlatformExecute = HigherOrderGranularOptionalPlatformContractsWithoutAdmin;

pub type PlatformExecute = <HigherOrderPlatformExecute as HigherOrderType>::Of<ExecuteSpec>;

pub type HigherOrderProtocolExecute = HigherOrderGranularOptionalProtocolContracts;

pub type ProtocolExecute = <HigherOrderProtocolExecute as HigherOrderType>::Of<ExecuteSpec>;

pub type Protocols<Protocol> = BTreeMap<String, Protocol>;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    deny_unknown_fields,
    bound(
        serialize = "Platform::Of<Unit>: Serialize, \
            Protocol::Of<Unit>: Serialize",
        deserialize = "Platform::Of<Unit>: Deserialize<'de>, \
            Protocol::Of<Unit>: Deserialize<'de>",
    )
)]
pub struct ContractsTemplate<Platform, Protocol, Unit>
where
    Platform: HigherOrderType,
    Protocol: HigherOrderType,
{
    pub platform: Platform::Of<Unit>,
    pub protocol: Protocols<Protocol::Of<Unit>>,
}

pub type Contracts =
    ContractsTemplate<HigherOrderPlatformContractsWithoutAdmin, HigherOrderProtocol, Addr>;

pub type ContractsMigration =
    ContractsTemplate<HigherOrderPlatformMigration, HigherOrderProtocolMigration, MigrationSpec>;

pub type ContractsExecute =
    ContractsTemplate<HigherOrderPlatformExecute, HigherOrderProtocolExecute, ExecuteSpec>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrationSpec
where
    Uint64: Into<CodeId>,
    CodeId: Into<Uint64>,
{
    pub code_id: Uint64,
    pub migrate_message: JsonValue,
    pub post_migrate_execute: Option<ExecuteSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExecuteSpec {
    pub message: JsonValue,
}

#[cfg(test)]
const _: fn() = || {
    let _: ContractsExecute = ContractsExecute {
        platform: Granularity::All(Some(PlatformContractsWithoutAdmin {
            timealarms: ExecuteSpec {
                message: JsonValue::Null,
            },
            treasury: ExecuteSpec {
                message: JsonValue::Null,
            },
        })),
        protocol: BTreeMap::from([
            (
                String::new(),
                Granularity::Some {
                    some: ProtocolContracts {
                        leaser: Some(ExecuteSpec {
                            message: JsonValue::Null,
                        }),
                        lpp: None,
                        oracle: Some(ExecuteSpec {
                            message: JsonValue::Null,
                        }),
                        profit: Some(ExecuteSpec {
                            message: JsonValue::Null,
                        }),
                        reserve: Some(ExecuteSpec {
                            message: JsonValue::Null,
                        }),
                    },
                },
            ),
            (
                String::new(),
                Granularity::All(Some(ProtocolContracts {
                    leaser: ExecuteSpec {
                        message: JsonValue::Null,
                    },
                    lpp: ExecuteSpec {
                        message: JsonValue::Null,
                    },
                    oracle: ExecuteSpec {
                        message: JsonValue::Null,
                    },
                    profit: ExecuteSpec {
                        message: JsonValue::Null,
                    },
                    reserve: ExecuteSpec {
                        message: JsonValue::Null,
                    },
                })),
            ),
            (String::new(), Granularity::All(None)),
        ]),
    };
};
