use sdk::cosmwasm_std::Storage;

#[cfg(feature = "schema")]
pub use crate::software::SemVer;
pub use crate::{
    error::Error,
    migration::MigrationMessage,
    protocol::Release as ProtocolRelease,
    release::{
        query, Id as ReleaseId, PlatformPackageRelease, ProtocolPackageRelease,
        ProtocolPackageReleaseId, ProtocolReleaseId, SoftwareReleaseId, UpdatablePackage,
    },
    software::{PackageRelease as SoftwarePackageRelease, VersionSegment},
};

mod error;
mod migration;
mod protocol;
mod release;
mod software;

pub type PlatformMigrationMessage<ContractMsg> =
    MigrationMessage<PlatformPackageRelease, ContractMsg>;

pub type ProtocolMigrationMessage<ContractMsg> =
    MigrationMessage<ProtocolPackageRelease, ContractMsg>;

pub struct FullUpdateOutput<MigrateStorageOutput> {
    pub to: SoftwareReleaseId,
    pub storage_migration_output: MigrateStorageOutput,
}

pub fn update_software_and_storage<
    PackageRelease,
    ContractError,
    MigrateStorageFunctor,
    StorageMigrationOutput,
    MapErrorFunctor,
>(
    storage: &mut dyn Storage,
    previous: PackageRelease,
    current: PackageRelease,
    current_release: &PackageRelease::ReleaseId,
    migrate_storage: MigrateStorageFunctor,
    map_error: MapErrorFunctor,
) -> Result<FullUpdateOutput<StorageMigrationOutput>, ContractError>
where
    PackageRelease: UpdatablePackage,
    MigrateStorageFunctor:
        FnOnce(&mut dyn Storage) -> Result<StorageMigrationOutput, ContractError>,
    MapErrorFunctor: FnOnce(Error) -> ContractError,
{
    previous
        .update_software_and_storage(&current, current_release)
        .map_err(map_error)
        .and_then(|()| {
            migrate_storage(storage).map(|storage_migration_output| FullUpdateOutput {
                to: SoftwareReleaseId(ReleaseId::VOID), //TODO remove the release return value!!!
                storage_migration_output,
            })
        })
}
