use serde::{Deserialize, Serialize};

use platform::contract::CodeId;
use sdk::cosmwasm_std::{Addr, Uint64};
use versioning::ReleaseId;

pub use crate::contracts::{
    Contracts, ContractsExecute, ContractsMigration, Dex, Granularity, HigherOrderGranularity,
    HigherOrderOption, HigherOrderPlatformContracts, HigherOrderProtocol,
    HigherOrderProtocolContracts, HigherOrderType, MigrationSpec, Network, PlatformContracts,
    Protocol, ProtocolContracts,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct InstantiateMsg {
    pub dex_admin: Addr,
    pub contracts: Contracts,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MigrateMsg {
    pub contracts_migration: ContractsMigration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteMsg
where
    Uint64: Into<CodeId>,
    CodeId: Into<Uint64>,
{
    Instantiate {
        code_id: Uint64,
        expected_address: Addr,
        protocol: String,
        label: String,
        message: String,
    },
    RegisterProtocol {
        name: String,
        protocol: Protocol<Addr>,
    },
    DeregisterProtocol(ProtocolContracts<MigrationSpec>),
    /// A message for **internal purposes only**.
    ///
    /// It is meant to clean up any temporary storage changes.
    ///
    /// Manual execution by an outside sender is considered an
    /// error, thus execution has to fail.
    EndOfMigration {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SudoMsg {
    ChangeDexAdmin {
        new_dex_admin: Addr,
    },
    RegisterProtocol {
        name: String,
        protocol: Protocol<Addr>,
    },
    /// Trigger a migration of contracts
    ///
    /// This message is meant to be used when the Admin contract
    /// itself does not need a migration. If one is needed then
    /// it should start as Admin contract migration which would then
    /// continue with the migration of the other contracts.
    MigrateContracts(MigrateContracts),
    ExecuteContracts(ContractsExecute),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrateContracts {
    pub release: ReleaseId,
    pub migration_spec: ContractsMigration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum QueryMsg
where
    Uint64: Into<CodeId>,
    CodeId: Into<Uint64>,
{
    InstantiateAddress {
        code_id: Uint64,
        protocol: String,
    },
    Protocols {},
    Platform {},
    Protocol(String),
    /// Implementation of [versioning::query::PlatformPackage::Release]
    PlatformPackageRelease {},
}

pub type ProtocolsQueryResponse = Vec<String>;

pub type PlatformQueryResponse = PlatformContracts<Addr>;

pub type ProtocolQueryResponse = Protocol<Addr>;

pub type ProtocolContractAddresses = ProtocolContracts<Addr>;
