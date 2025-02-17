use serde::{Deserialize, Serialize};

use super::higher_order_type::FirstOrderType;

pub(super) mod higher_order_type;
#[cfg(feature = "contract")]
mod impl_mod;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename = "PlatformContractsWithoutAdmin",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub struct ContractsWithoutAdmin<T> {
    pub timealarms: T,
    pub treasury: T,
}

impl<T> ContractsWithoutAdmin<T> {
    pub fn with_admin(self, admin: T) -> Contracts<T> {
        let Self {
            timealarms,
            treasury,
        } = self;

        Contracts {
            admin,
            timealarms,
            treasury,
        }
    }
}

impl<T> FirstOrderType<higher_order_type::ContractsWithoutAdmin> for ContractsWithoutAdmin<T> {
    type Unit = T;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename = "PlatformContracts",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub struct Contracts<T> {
    pub admin: T,
    pub timealarms: T,
    pub treasury: T,
}

impl<T> FirstOrderType<higher_order_type::Contracts> for Contracts<T> {
    type Unit = T;
}
