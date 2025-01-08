use std::{
    cmp::Ordering,
    fmt::{Display, Formatter, Result as FmtResult},
};

use serde::{Deserialize, Serialize};

use sdk::cosmwasm_std::{StdError, StdResult, Storage};
#[cfg(feature = "schema")]
use sdk::schemars::{self, JsonSchema};

//TODO issue#466 hide `Id`
pub use self::release::{Id, Release};

mod release;

pub type VersionSegment = u16;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SemVer {
    major: VersionSegment,
    minor: VersionSegment,
    patch: VersionSegment,
}

impl SemVer {
    pub const fn parse(version: &str) -> Self {
        let version: &[u8] = version.as_bytes();
        let mut version_index: usize = 0;

        let mut segments: [VersionSegment; 3] = [0; 3];
        let mut segment_index: usize = 0;

        while version_index < version.len() {
            match version[version_index] {
                digit @ b'0'..=b'9' => {
                    segments[segment_index] *= 10;
                    segments[segment_index] += (digit - b'0') as VersionSegment;
                }
                b'.' => {
                    segment_index += 1;

                    assert!(segment_index != segments.len(), "Unexpected segment!");
                    assert!(
                        version_index + 1 < version.len(),
                        "Version can't end with a dot!"
                    );
                }
                _ => panic!(
                    "Unexpected symbol encountered! Expected an ASCII number or an ASCII dot!"
                ),
            }

            version_index += 1;
        }

        assert!(segment_index + 1 == segments.len(), "Invalid version string! Expected three segments (major, minor and patch), but got less!");

        let [major, minor, patch]: [VersionSegment; 3] = segments;

        Self {
            major,
            minor,
            patch,
        }
    }
}

impl Display for SemVer {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

// #[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
/// A reference type representing a software package
// TODO rename to SoftwarePackage
pub struct Version {
    storage: VersionSegment,
    /// the reference identification attribute
    software: SemVer,
}

impl Version {
    pub const fn new(storage: VersionSegment, software: SemVer) -> Self {
        Self { storage, software }
    }

    pub const fn same_storage(&self, other: &Self) -> bool {
        self.check_storage(other.storage)
    }

    pub const fn next_storage(&self, other: &Self) -> bool {
        other.check_storage(self.storage.wrapping_add(1))
    }

    const fn check_storage(&self, expected: VersionSegment) -> bool {
        self.storage == expected
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "version: {}, storage: {}",
            self.software, self.storage
        ))
    }
}

impl Eq for Version {}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        let res = self.software == other.software;
        if res {
            debug_assert_eq!(self.storage, other.storage);
        }
        res
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.software.partial_cmp(&other.software)
    }
}

#[macro_export]
macro_rules! package_version {
    () => {{
        $crate::SemVer::parse(::core::env!(
            "CARGO_PKG_VERSION",
            "Cargo package version is not set as an environment variable!"
        ))
    }};
}

pub fn initialize(_storage: &mut dyn Storage, _version: Version) -> StdResult<()> {
    // no op
    // TBD remove from the stack upward
    Ok(())
}

pub fn update_legacy_software<ContractError, MapErrorFunctor>(
    storage: &mut dyn Storage,
    new: Version,
    map_error: MapErrorFunctor,
) -> Result<Release, ContractError>
where
    MapErrorFunctor: FnOnce(StdError) -> ContractError,
{
    Release::pull_prev(storage)
        .and_then(|prev_release| {
            let this_release = Release::this(new);
            prev_release.update_software(this_release)
        })
        .map_err(map_error)
}

pub struct FullUpdateOutput<MigrateStorageOutput> {
    pub to: Release,
    pub storage_migration_output: MigrateStorageOutput,
}

pub fn update_legacy_software_and_storage<
    ContractError,
    MigrateStorageFunctor,
    StorageMigrationOutput,
    MapErrorFunctor,
>(
    storage: &mut dyn Storage,
    new: Version,
    migrate_storage: MigrateStorageFunctor,
    map_error: MapErrorFunctor,
) -> Result<FullUpdateOutput<StorageMigrationOutput>, ContractError>
where
    MigrateStorageFunctor:
        FnOnce(&mut dyn Storage) -> Result<StorageMigrationOutput, ContractError>,
    MapErrorFunctor: FnOnce(StdError) -> ContractError,
{
    Release::pull_prev(storage)
        .and_then(|prev_release| {
            let this_release = Release::this(new);
            prev_release.update_software_and_storage(this_release)
        })
        .map_err(map_error)
        .and_then(|new_release| {
            migrate_storage(storage).map(|storage_migration_output| FullUpdateOutput {
                to: new_release,
                storage_migration_output,
            })
        })
}

#[cfg(test)]
mod tests {
    use crate::SemVer;

    #[test]
    fn valid() {
        const VERSIONS: &[(&str, SemVer)] = &[
            (
                "0.0.1",
                SemVer {
                    major: 0,
                    minor: 0,
                    patch: 1,
                },
            ),
            (
                "1.3.2",
                SemVer {
                    major: 1,
                    minor: 3,
                    patch: 2,
                },
            ),
            (
                "12.34.56",
                SemVer {
                    major: 12,
                    minor: 34,
                    patch: 56,
                },
            ),
        ];

        for &(version, expected) in VERSIONS {
            assert_eq!(SemVer::parse(version), expected);
        }
    }

    #[test]
    #[should_panic = "Invalid version string! Expected three segments (major, minor and patch), but got less!"]
    fn invalid_empty() {
        _ = SemVer::parse("");
    }

    #[test]
    #[should_panic = "Invalid version string! Expected three segments (major, minor and patch), but got less!"]
    fn invalid_one_segment() {
        _ = SemVer::parse("1");
    }

    #[test]
    #[should_panic = "Version can't end with a dot!"]
    fn invalid_one_segment_and_dot() {
        _ = SemVer::parse("1.");
    }

    #[test]
    #[should_panic = "Invalid version string! Expected three segments (major, minor and patch), but got less!"]
    fn invalid_two_segments() {
        _ = SemVer::parse("1.2");
    }

    #[test]
    #[should_panic = "Version can't end with a dot!"]
    fn invalid_two_segments_and_dot() {
        _ = SemVer::parse("1.2.");
    }

    #[test]
    #[should_panic = "Unexpected segment!"]
    fn invalid_three_segments_and_dot() {
        _ = SemVer::parse("1.2.3.");
    }

    #[test]
    #[should_panic = "Unexpected segment!"]
    fn invalid_four_segments() {
        _ = SemVer::parse("1.2.3.4");
    }

    #[test]
    #[should_panic = "Unexpected symbol encountered! Expected an ASCII number or an ASCII dot!"]
    fn excluded_postfix() {
        _ = SemVer::parse("1.2.3-rc1");
    }
}
