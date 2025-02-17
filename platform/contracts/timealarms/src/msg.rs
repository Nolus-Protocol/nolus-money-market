use serde::{Deserialize, Serialize};

use sdk::cosmwasm_std::Timestamp;

pub type AlarmsCount = platform::dispatcher::AlarmsCount;

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddAlarm {
        time: Timestamp,
    },
    /// Returns [`DispatchAlarmsResponse`] as response data.
    DispatchAlarms {
        max_count: AlarmsCount,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SudoMsg {}

#[derive(Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryMsg {
    ContractVersion {},
    AlarmsStatus {},
    /// Implementation of [versioning::query::PlatformPackage::Release]
    PlatformPackageRelease {},
}

#[derive(Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteAlarmMsg {
    TimeAlarm {},
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "testing", derive(PartialEq, Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
/// number of sent alarms
pub struct DispatchAlarmsResponse(pub AlarmsCount);

#[derive(Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AlarmsStatusResponse {
    pub remaining_alarms: bool,
}

#[cfg(test)]
mod test {
    use platform::tests as platform_tests;

    use super::QueryMsg;

    #[test]
    fn release() {
        assert_eq!(
            Ok(QueryMsg::PlatformPackageRelease {}),
            platform_tests::ser_de(&versioning::query::PlatformPackage::Release {}),
        );
    }
}
