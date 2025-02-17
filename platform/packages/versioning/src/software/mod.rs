use serde::{Deserialize, Serialize};

use sdk::cw_storage_plus::Item;

pub use self::{
    package::Package,
    version::{SemVer, VersionSegment},
};

use crate::{
    release::{Id, UpdatablePackage},
    Error,
};

mod package;
mod version;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct PackageRelease {
    id: Id,
    code: Package,
}

impl PackageRelease {
    pub fn pull_prev(
        name: &'static str,
        storage: &mut dyn sdk::cosmwasm_std::Storage,
    ) -> Result<Self, Error> {
        #[derive(Deserialize)]
        pub struct LegacyPackage {
            storage: VersionSegment,
            software: SemVer,
        }

        const VERSION_STORAGE_KEY: Item<LegacyPackage> = Item::new("contract_version");
        const PREV_ID: Id = Id::new_static("v0.7.6");

        impl LegacyPackage {
            fn migrate_to(self, name: &'static str) -> Package {
                Package::new(name, self.software, self.storage)
            }
        }
        impl Serialize for LegacyPackage {
            fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                unimplemented!("LegacyPackage is not meant to be serialized")
            }
        }

        VERSION_STORAGE_KEY
            .load(storage)
            .map_err(Error::loading)
            .inspect(|_| VERSION_STORAGE_KEY.remove(storage))
            .map(|code| Self::instance(PREV_ID, code.migrate_to(name)))
    }

    pub const fn current(name: &'static str, version: &str, storage: VersionSegment) -> Self {
        const ID: &str = env!(
            "SOFTWARE_RELEASE_ID",
            "No software release identifier provided as an environment variable! Please set \
            \"SOFTWARE_RELEASE_ID\" environment variable!",
        );

        Self::instance(
            Id::new_static(ID),
            Package::new(name, SemVer::parse(version), storage),
        )
    }

    const fn instance(id: Id, code: Package) -> Self {
        Self { id, code }
    }

    pub const fn version(&self) -> SemVer {
        self.code.version()
    }

    fn check_software_update_allowed<F>(&self, to: &Self, storage_check: F) -> Result<(), Error>
    where
        F: FnOnce(&Self, &Package) -> Result<(), Error>,
    {
        self.check_name_match(&to.code)
            .and_then(|()| self.check_code_same_or_newer(&to.code))
            .and_then(|()| storage_check(self, &to.code))
    }

    fn check_release_match(&self, target: &Id) -> Result<(), Error> {
        if self.id == *target {
            Ok(())
        } else {
            Err(Error::software_release_mismatch(
                self.id.clone(),
                target.clone(),
            ))
        }
    }

    fn check_name_match(&self, other: &Package) -> Result<(), Error> {
        if self.code.same_name(other) {
            Ok(())
        } else {
            Err(Error::package_names_mismatch(&self.code, other))
        }
    }
    fn check_code_same_or_newer(&self, other: &Package) -> Result<(), Error> {
        if &self.code <= other {
            Ok(())
        } else {
            Err(Error::older_package_code(&self.code, other))
        }
    }

    fn check_storage_match(&self, other: &Package) -> Result<(), Error> {
        if self.code.same_storage(other) {
            Ok(())
        } else {
            Err(Error::package_storage_versions_mismatch(&self.code, other))
        }
    }

    fn check_storage_adjacent(&self, next: &Package) -> Result<(), Error> {
        if self.code.next_storage(next) {
            Ok(())
        } else {
            Err(Error::package_storage_version_not_adjacent(
                &self.code, next,
            ))
        }
    }
}

impl UpdatablePackage for PackageRelease {
    type ReleaseId = Id;

    fn update_software(&self, to: &Self, to_release: &Self::ReleaseId) -> Result<(), Error> {
        to.check_release_match(to_release)
            .and_then(|()| self.check_software_update_allowed(to, Self::check_storage_match))
    }

    fn update_software_and_storage(
        &self,
        to: &Self,
        to_release: &Self::ReleaseId,
    ) -> Result<(), Error> {
        to.check_release_match(to_release)
            .and_then(|()| self.check_software_update_allowed(to, Self::check_storage_adjacent))
    }
}

#[cfg(test)]
mod test {
    use crate::{
        release::{Id, UpdatablePackage},
        Error,
    };

    use super::{version::VersionSegment, Package, PackageRelease, SemVer};

    const CURRENT_NAME: &str = "package_A";
    const CURRENT_VERSION: SemVer = SemVer::parse("0.3.4");
    const CURRENT_STORAGE: VersionSegment = 1;

    const OTHER_NAME: &str = "package_B";
    const NEWER_VERSION: SemVer = SemVer::parse("0.3.5");

    fn prod1_id() -> Id {
        Id::new_static("v0.5.3")
    }

    fn prod2_id() -> Id {
        Id::new_static("v0.5.4")
    }

    #[test]
    fn prod_software() {
        let current_code = Package::new(CURRENT_NAME, CURRENT_VERSION, CURRENT_STORAGE);
        let next_code = Package::new(CURRENT_NAME, NEWER_VERSION, CURRENT_STORAGE);
        let current_release = PackageRelease::instance(prod1_id(), current_code);
        let next_release = PackageRelease::instance(prod2_id(), next_code);

        assert_eq!(
            Ok(()),
            current_release
                .clone()
                .update_software(&current_release, &prod1_id())
        );

        assert_eq!(
            Ok(()),
            current_release
                .clone()
                .update_software(&next_release, &prod2_id())
        );

        assert!(matches!(
            current_release
                .clone()
                .update_software(&current_release, &prod2_id()),
            Err(Error::SoftwareReleaseMismatch(_, _))
        ));

        assert!(matches!(
            current_release
                .clone()
                .update_software(&next_release, &prod1_id()),
            Err(Error::SoftwareReleaseMismatch(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(OTHER_NAME, CURRENT_VERSION, CURRENT_STORAGE),
                ),
                &prod1_id(),
            ),
            Err(Error::PackageNamesMismatch(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(CURRENT_NAME, SemVer::parse("0.3.3"), CURRENT_STORAGE),
                ),
                &prod1_id(),
            ),
            Err(Error::OlderPackageCode(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(CURRENT_NAME, NEWER_VERSION, CURRENT_STORAGE + 1),
                ),
                &prod1_id(),
            ),
            Err(Error::PackageStorageVersionMismatch(_, _))
        ));
    }

    #[test]
    fn prod_software_and_storage() {
        let current_code = Package::new(CURRENT_NAME, CURRENT_VERSION, CURRENT_STORAGE);
        let next_code = Package::new(CURRENT_NAME, NEWER_VERSION, CURRENT_STORAGE + 1);
        let current_release = PackageRelease::instance(prod1_id(), current_code);
        let next_release = PackageRelease::instance(prod2_id(), next_code);

        assert!(matches!(
            current_release
                .clone()
                .update_software_and_storage(&current_release, &prod1_id()),
            Err(Error::PackageStorageVersionNotAdjacent(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software_and_storage(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(CURRENT_NAME, NEWER_VERSION, CURRENT_STORAGE),
                ),
                &prod1_id(),
            ),
            Err(Error::PackageStorageVersionNotAdjacent(_, _))
        ));

        assert!(matches!(
            current_release
                .clone()
                .update_software_and_storage(&next_release, &prod1_id()),
            Err(Error::SoftwareReleaseMismatch(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software_and_storage(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(OTHER_NAME, NEWER_VERSION, CURRENT_STORAGE + 1),
                ),
                &prod1_id(),
            ),
            Err(Error::PackageNamesMismatch(_, _))
        ));

        assert!(matches!(
            current_release.clone().update_software_and_storage(
                &PackageRelease::instance(
                    prod1_id(),
                    Package::new(CURRENT_NAME, SemVer::parse("0.3.3"), CURRENT_STORAGE + 1),
                ),
                &prod1_id(),
            ),
            Err(Error::OlderPackageCode(_, _))
        ));

        assert_eq!(
            Ok(()),
            current_release
                .clone()
                .update_software_and_storage(&next_release, &prod2_id())
        );
    }
}
