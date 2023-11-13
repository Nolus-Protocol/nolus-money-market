use serde::{Deserialize, Serialize};

use platform::contract::CodeId;
use sdk::{
    cosmwasm_std::Addr,
    schemars::{self, JsonSchema},
};

use crate::contracts::{
    ContractsGroupedByDex, ContractsMigration, ContractsPostMigrationExecute, MigrationSpec,
    Protocol,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct InstantiateMsg {
    pub dex_admin: Addr,
    pub contracts: ContractsGroupedByDex,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrateMsg {
    pub dex: String,
    pub dex_admin: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteMsg {
    Instantiate {
        code_id: CodeId,
        label: String,
        message: String,
    },
    RegisterDex {
        dex: String,
        contracts: Protocol<Addr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SudoMsg {
    ChangeDexAdmin {
        new_dex_admin: Addr,
    },
    RegisterDex {
        dex: String,
        contracts: Protocol<Addr>,
    },
    MigrateContracts(MigrateContracts),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct MigrateContracts {
    pub release: String,
    pub admin_contract: Option<MigrationSpec>,
    pub migration_spec: ContractsMigration,
    pub post_migration_execute: ContractsPostMigrationExecute,
}
