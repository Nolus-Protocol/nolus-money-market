use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::{Deserialize, Serialize};

pub type VersionSegment = u16;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemVer {
    major: VersionSegment,
    minor: VersionSegment,
    patch: VersionSegment,
}

#[macro_export]
macro_rules! package_version {
    () => {{
        ::core::env!(
            "CARGO_PKG_VERSION",
            "Cargo package version is not set as an environment variable!"
        )
    }};
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

#[cfg(test)]
mod tests {
    use super::SemVer;

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
