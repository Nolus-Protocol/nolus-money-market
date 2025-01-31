use std::fmt::{Debug, Formatter};

use serde::{Deserialize, Serialize};

use super::UpdatablePackage;

#[derive(Serialize, Deserialize)]
#[serde(
    bound(
        serialize = "Package::ReleaseId: Serialize,
            ContractMsg: Serialize",
        deserialize = "Package::ReleaseId: Deserialize<'de>,
            ContractMsg: Deserialize<'de>",
    ),
    deny_unknown_fields,
    rename_all = "snake_case"
)]
pub struct MigrationMessage<Package, ContractMsg>
where
    Package: UpdatablePackage,
{
    /* TODO Add field once deployed contracts can be queried about their version
        and release information.
    pub migrate_from: Package,
    */
    pub to_release: Package::ReleaseId,
    pub message: ContractMsg,
}

impl<Package, ContractMsg> MigrationMessage<Package, ContractMsg>
where
    Package: UpdatablePackage,
{
    pub const fn new(
        /* TODO Add field once deployed contracts can be queried about their version
            and release information.
        migrate_from: Package,
        */
        to_release: Package::ReleaseId,
        message: ContractMsg,
    ) -> Self {
        Self {
            /* TODO Add field once deployed contracts can be queried about their version
                and release information.
            migrate_from,
            */
            to_release,
            message,
        }
    }
}

impl<Package, ContractMsg> Debug for MigrationMessage<Package, ContractMsg>
where
    Package: UpdatablePackage, /* + Debug */
    Package::ReleaseId: Debug,
    ContractMsg: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MigrationMessage")
            /* TODO Add field once deployed contracts can be queried about their version
                and release information.
            .field("migrate_from", &self.migrate_from)
            */
            .field("to_release", &self.to_release)
            .field("message", &self.message)
            .finish()
    }
}
