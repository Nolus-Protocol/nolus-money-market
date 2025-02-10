use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use ::platform::contract::CodeId;
use sdk::cosmwasm_std::{Addr, Uint64};

#[cfg(feature = "contract")]
pub(crate) use self::impl_mod::{execute, migrate};
#[cfg(feature = "contract")]
use self::impl_mod::{AsRef, TryForEach, TryForEachPair};
pub use self::{
    granular::{Granularity, HigherOrderType as HigherOrderGranularity},
    higher_order_type::{HigherOrderType, Option as HigherOrderOption},
    platform::{HigherOrderType as HigherOrderPlatformContracts, PlatformContracts},
    protocol::{
        higher_order_type::{
            Protocol as HigherOrderProtocol, ProtocolContracts as HigherOrderProtocolContracts,
        },
        Dex, Network, Protocol, ProtocolContracts,
    },
};

mod granular;
mod higher_order_type;
#[cfg(feature = "contract")]
mod impl_mod;
mod platform;
mod protocol;

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
    pub protocol: BTreeMap<String, Protocol::Of<Unit>>,
}

pub type Contracts = ContractsTemplate<HigherOrderPlatformContracts, HigherOrderProtocol, Addr>;

pub type ContractsMigration = ContractsTemplate<
    HigherOrderGranularity<HigherOrderPlatformContracts, HigherOrderOption>,
    HigherOrderGranularity<HigherOrderProtocolContracts, HigherOrderOption>,
    MigrationSpec,
>;

pub type ContractsExecute = ContractsTemplate<
    HigherOrderGranularity<HigherOrderPlatformContracts, HigherOrderOption>,
    HigherOrderGranularity<HigherOrderProtocolContracts, HigherOrderOption>,
    String,
>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrationSpec
where
    Uint64: Into<CodeId>,
    CodeId: Into<Uint64>,
{
    pub code_id: Uint64,
    pub migrate_msg: String,
    pub post_migrate_execute_msg: Option<String>,
}

#[cfg(test)]
const _: fn() = || {
    let _: ContractsExecute = ContractsExecute {
        platform: Granularity::All(Some(PlatformContracts {
            timealarms: String::new(),
            treasury: String::new(),
        })),
        protocol: BTreeMap::from([
            (
                String::new(),
                Granularity::Some {
                    some: ProtocolContracts {
                        leaser: Some(String::new()),
                        lpp: None,
                        oracle: Some(String::new()),
                        profit: Some(String::new()),
                        reserve: Some(String::new()),
                    },
                },
            ),
            (
                String::new(),
                Granularity::All(Some(ProtocolContracts {
                    leaser: String::new(),
                    lpp: String::new(),
                    oracle: String::new(),
                    profit: String::new(),
                    reserve: String::new(),
                })),
            ),
            (String::new(), Granularity::All(None)),
        ]),
    };
};
