use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    zero::Zero,
};

use super::{Percent, Units};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(into = "Percent", try_from = "Percent")]
pub struct BoundPercent<const UPPER_BOUND: Units>(Percent);

impl<const UPPER_BOUND: Units> BoundPercent<UPPER_BOUND> {
    pub const ZERO: Self = Self(Percent::ZERO);

    pub const fn try_from_percent(percent: Percent) -> Result<Self> {
        if percent.units() <= UPPER_BOUND {
            Ok(Self(percent))
        } else {
            Err(Error::UpperBoundCrossed {
                bound: UPPER_BOUND,
                value: percent.units(),
            })
        }
    }

    pub const fn percent(&self) -> Percent {
        self.0
    }
}

impl<const UPPER_BOUND: Units> Zero for BoundPercent<UPPER_BOUND> {
    const ZERO: Self = Self::ZERO;
}

impl<const UPPER_BOUND: Units> TryFrom<Percent> for BoundPercent<UPPER_BOUND> {
    type Error = Error;

    fn try_from(value: Percent) -> Result<Self> {
        Self::try_from_percent(value)
    }
}

impl<const UPPER_BOUND: Units> From<BoundPercent<UPPER_BOUND>> for Percent {
    fn from(value: BoundPercent<UPPER_BOUND>) -> Self {
        value.percent()
    }
}

pub type BoundToHundredPercent = BoundPercent<{ Percent::HUNDRED.units() }>;
