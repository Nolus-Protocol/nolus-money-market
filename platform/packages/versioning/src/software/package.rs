use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::{Display, Formatter, Result as FmtResult},
};

use serde::{Deserialize, Serialize};

use super::version::{SemVer, VersionSegment};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A 'reference type' representing a software package
pub struct Package {
    /// the package name
    ///
    /// It is a part of the package id.
    /// See [`ReferenceId`] doc on the need to use [`Cow`]
    name: Cow<'static, str>,

    /// the reference identification attribute
    version: SemVer,
    storage: VersionSegment,
}

#[macro_export]
macro_rules! package_name {
    () => {{
        ::core::env!(
            "CARGO_PKG_NAME",
            "Cargo package name is not set as an environment variable!"
        )
    }};
}

impl Package {
    pub const fn new(name: &'static str, version: SemVer, storage: VersionSegment) -> Self {
        Self {
            name: Cow::Borrowed(name),
            version,
            storage,
        }
    }

    pub const fn version(&self) -> SemVer {
        self.version
    }

    pub fn same_name(&self, other: &Self) -> bool {
        self.name == other.name
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

impl Display for Package {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "name: {}, version: {}, storage: {}",
            self.name, self.version, self.storage
        ))
    }
}

impl Eq for Package {}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        let res = self.name == other.name && self.version == other.version;
        if res {
            debug_assert_eq!(self.storage, other.storage);
        }
        res
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let res = match self.name.partial_cmp(&other.name) {
            Some(Ordering::Equal) => self.version.partial_cmp(&other.version),
            res => res,
        };
        debug_assert_eq!(res == Some(Ordering::Equal), self.eq(other));
        res
    }
}

#[cfg(test)]
mod test {
    use std::cmp::Ordering;

    use crate::{software::SemVer, VersionSegment};

    use super::Package;

    const NAME1: &str = "p1";
    const NAME2: &str = "p2";
    const VER1: SemVer = SemVer::parse("0.0.3");
    const VER2: SemVer = SemVer::parse("0.0.4");
    const STOR1: VersionSegment = 4;
    const STOR2: VersionSegment = STOR1 + 1;

    #[test]
    fn eq() {
        assert_eq!(
            Package::new(NAME1, VER1, STOR1),
            Package::new(NAME1, VER1, STOR1)
        );
        assert_ne!(
            Package::new(NAME1, VER1, STOR1),
            Package::new(NAME1, VER2, STOR1)
        );
        assert_ne!(
            Package::new(NAME1, VER1, STOR1),
            Package::new(NAME1, VER2, STOR2)
        );

        assert_ne!(
            Package::new(NAME1, VER1, STOR1),
            Package::new(NAME2, VER1, STOR1)
        );
        assert_eq!(
            Package::new(NAME2, VER1, STOR1),
            Package::new(NAME2, VER1, STOR1)
        );
    }

    #[test]
    fn cmp() {
        assert_eq!(
            Some(Ordering::Equal),
            Package::new(NAME1, VER1, STOR1).partial_cmp(&Package::new(NAME1, VER1, STOR1))
        );

        assert_eq!(
            Some(Ordering::Less),
            Package::new(NAME1, VER1, STOR1).partial_cmp(&Package::new(NAME1, VER2, STOR1))
        );
        assert_eq!(
            Some(Ordering::Greater),
            Package::new(NAME1, VER2, STOR1).partial_cmp(&Package::new(NAME1, VER1, STOR1))
        );

        assert_eq!(
            Some(Ordering::Less),
            Package::new(NAME1, VER1, STOR1).partial_cmp(&Package::new(NAME2, VER1, STOR1))
        );
        assert_eq!(
            Some(Ordering::Greater),
            Package::new(NAME2, VER1, STOR1).partial_cmp(&Package::new(NAME1, VER1, STOR1))
        );
    }
}
//TODO add eq, ord tests
