use serde::{Deserialize, Serialize};

use sdk::{
    cosmwasm_std::Addr,
    schemars::{self, JsonSchema},
};

use crate::typedefs::CadenceHours;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub cadence_hours: CadenceHours,
    pub treasury: Addr,
    pub oracle: Addr,
    pub timealarms: Addr,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteMsg {
    TimeAlarm {},
    Config {
        cadence_hours: CadenceHours,
    },

    /// An entry point for safe delivery of a Dex response
    ///
    /// Invoked always by the same contract instance.
    DexCallback(),

    /// An entry point for safe delivery of a ICA Open response, error or timeout
    ///
    /// Invoked always by the same contract instance.
    DexCallbackContinue(),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ConfigResponse {
    pub cadence_hours: CadenceHours,
}
