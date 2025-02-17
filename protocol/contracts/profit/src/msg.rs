use serde::{Deserialize, Serialize};

use dex::ConnectionParams;
use sdk::cosmwasm_std::Addr;

use crate::typedefs::CadenceHours;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub cadence_hours: CadenceHours,
    pub treasury: Addr,
    pub oracle: Addr,
    pub timealarms: Addr,
    pub dex: ConnectionParams,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(Debug))]
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    /// Implementation of [versioning::query::ProtocolPackage::Release]
    ProtocolPackageRelease {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ConfigResponse {
    pub cadence_hours: CadenceHours,
}

#[cfg(all(feature = "internal.test.testing", test))]
mod test {
    use platform::tests as platform_tests;

    use super::QueryMsg;

    #[test]
    fn release() {
        assert_eq!(
            Ok(QueryMsg::ProtocolPackageRelease {}),
            platform_tests::ser_de(&versioning::query::ProtocolPackage::Release {}),
        );
    }
}
