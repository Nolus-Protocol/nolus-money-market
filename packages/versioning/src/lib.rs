use serde::{Deserialize, Serialize};

#[cfg(feature = "schema")]
use sdk::schemars::{self, JsonSchema};
use sdk::{
    cosmwasm_std::{StdError, StdResult, Storage},
    cw_storage_plus::Item,
};

use self::release::ReleaseLabel;

mod release;

pub type VersionSegment = u16;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SemVer {
    major: VersionSegment,
    minor: VersionSegment,
    patch: VersionSegment,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Version {
    storage: VersionSegment,
    software: SemVer,
}

impl Version {
    pub const fn new(storage: VersionSegment, software: SemVer) -> Self {
        Self { storage, software }
    }
}

pub fn parse_semver(version: &str) -> SemVer {
    fn parse_segment<'r, I>(
        iter: &mut I,
        lowercase_name: &str,
        pascal_case_name: &str,
    ) -> VersionSegment
    where
        I: Iterator<Item = &'r str> + ?Sized,
    {
        iter.next()
            .unwrap_or_else(|| panic!("No {} segment in version string!", lowercase_name))
            .parse()
            .unwrap_or_else(|_| {
                panic!(
                    "{} segment in version string is not a number!",
                    pascal_case_name
                )
            })
    }

    let mut iter = version.split('.');

    let major: VersionSegment = parse_segment(&mut iter, "major", "Major");
    let minor: VersionSegment = parse_segment(&mut iter, "minor", "Minor");
    let patch: VersionSegment = parse_segment(&mut iter, "patch", "Patch");

    if iter.next().is_some() {
        panic!("Unexpected fourth segment found in version string!");
    };

    SemVer {
        major,
        minor,
        patch,
    }
}

#[macro_export]
macro_rules! package_version {
    () => {{
        $crate::parse_semver(::core::env!(
            "CARGO_PKG_VERSION",
            "Cargo package version is not set as an environment variable!",
        ))
    }};
}

#[macro_export]
macro_rules! version {
    ($storage: expr) => {{
        $crate::Version::new($storage, $crate::package_version!())
    }};
}

const VERSION_STORAGE_KEY: Item<'static, Version> = Item::new("contract_version");

pub fn initialize(storage: &mut dyn Storage, version: Version) -> StdResult<()> {
    VERSION_STORAGE_KEY.save(storage, &version)
}

pub fn update_software<ContractError>(
    storage: &mut dyn Storage,
    new: Version,
) -> Result<ReleaseLabel, ContractError>
where
    StdError: Into<ContractError>,
{
    load_version(storage)
        .and_then(|current| release::allow_software_update(&current, &new))
        .and_then(|()| save_version(storage, &new))
        .map(|()| release::label())
        .map_err(Into::into)
}

pub fn update_software_and_storage<
    const FROM_STORAGE_VERSION: VersionSegment,
    MigrateStorageFunctor,
    ContractError,
>(
    storage: &mut dyn Storage,
    new: Version,
    migrate_storage: MigrateStorageFunctor,
) -> Result<ReleaseLabel, ContractError>
where
    MigrateStorageFunctor: FnOnce(&mut dyn Storage) -> Result<(), ContractError>,
    StdError: Into<ContractError>,
{
    load_version(storage)
        .and_then(|current| {
            release::allow_software_and_storage_update::<FROM_STORAGE_VERSION>(&current, &new)
        })
        .and_then(|()| save_version(storage, &new))
        .map_err(Into::into)
        .and_then(|()| migrate_storage(storage))
        .map(|()| release::label())
}

fn load_version(storage: &mut dyn Storage) -> Result<Version, StdError> {
    VERSION_STORAGE_KEY.load(storage)
}

fn save_version(storage: &mut dyn Storage, new: &Version) -> Result<(), StdError> {
    VERSION_STORAGE_KEY.save(storage, new)
}
