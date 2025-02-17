use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
// The two usecases, building the current release, and deserializing a release, call for `&'static str` and String, respectively.
// We use Cow since it is an enum of the two. We do not need to mutate it.
pub struct Id(Cow<'static, str>); //TODO typedef the Cow to `????Str`

impl Id {
    pub(crate) const fn new_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    #[cfg(feature = "testing")]
    pub const fn new_test(id: &'static str) -> Self {
        Self::new_static(id)
    }
}

impl Id {
    pub const VOID: Self = Self::new_static("void-release");
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}
