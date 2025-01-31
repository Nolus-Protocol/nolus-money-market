use std::{collections::BTreeMap, marker::PhantomData};

use serde::{Deserialize, Serialize};

use ::platform::contract::CodeId;
use json_value::JsonValue;
use sdk::{
    cosmwasm_std::{Addr, Uint64},
    schemars::{self, JsonSchema},
};
use versioning::ProtocolReleaseId;

#[cfg(feature = "contract")]
pub(crate) use self::impl_mod::{execute, migrate};
#[cfg(feature = "contract")]
use self::impl_mod::{Map, TryForEach, TryForEachPair};
pub use self::{
    granular::{Granularity, HigherOrderType as HigherOrderGranularity},
    higher_order_type::{
        Compose as HigherOrderCompose, HigherOrderType, Option as HigherOrderOption,
    },
    platform::{Contracts as PlatformContracts, HigherOrderType as HigherOrderPlatformContracts},
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
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
#[schemars(bound = "Platform: JsonSchema, \
    Platform::Of<Unit>: JsonSchema, \
    Protocol: JsonSchema, \
    Protocol::Of<Unit>: JsonSchema, \
    Unit: JsonSchema")]
pub struct ContractsTemplate<Platform, Protocol, Unit>
where
    Platform: HigherOrderType,
    Protocol: HigherOrderType,
{
    pub platform: Platform::Of<Unit>,
    pub protocol: BTreeMap<String, Protocol::Of<Unit>>,
}

pub type Contracts = ContractsTemplate<HigherOrderPlatformContracts, HigherOrderProtocol, Addr>;

pub type ContractsMigration = ContractsTemplate<
    HigherOrderGranularity<HigherOrderPlatformContracts, HigherOrderOption>,
    HigherOrderCompose<
        HigherOrderLeftBoundTuple<ProtocolReleaseId>,
        HigherOrderGranularity<HigherOrderProtocolContracts, HigherOrderOption>,
    >,
    MigrationSpec,
>;

pub type ContractsExecute = ContractsTemplate<
    HigherOrderGranularity<HigherOrderPlatformContracts, HigherOrderOption>,
    HigherOrderGranularity<HigherOrderProtocolContracts, HigherOrderOption>,
    ExecuteSpec,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct HigherOrderLeftBoundTuple<Left> {
    _left: PhantomData<Left>,
}

impl<Left> HigherOrderType for HigherOrderLeftBoundTuple<Left> {
    type Of<T> = (Left, T);
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrationSpec
where
    Uint64: Into<CodeId>,
    CodeId: Into<Uint64>,
{
    pub code_id: Uint64,
    pub migrate_msg: JsonValue,
    pub post_migrate_execute_msg: Option<ExecuteSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExecuteSpec {
    pub message: JsonValue,
}

#[cfg(test)]
const _: fn() = || {
    let _: ContractsExecute = ContractsExecute {
        platform: Granularity::All(Some(PlatformContracts {
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
