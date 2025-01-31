use serde::{Deserialize, Serialize};

#[cfg(feature = "schema")]
use sdk::schemars::{self, JsonSchema};

pub use protocol_::Protocol;

use crate::{Error, ProtocolReleaseId};

#[cfg(feature = "protocol_contract")]
mod current;
mod protocol_;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(test, derive(Debug))]
pub struct Release {
    id: ProtocolReleaseId,
    protocol: Protocol,
}

impl Release {
    pub fn check_update_allowed(
        &self,
        to: &Self,
        to_release: &ProtocolReleaseId,
    ) -> Result<(), Error> {
        to.check_release_match(to_release)
            .and_then(|()| self.check_protocol_match(to))
    }

    fn check_release_match(&self, target: &ProtocolReleaseId) -> Result<(), Error> {
        if &self.id == target {
            Ok(())
        } else {
            Err(Error::release_mismatch(&self.id.0, &target.0))
        }
    }

    fn check_protocol_match(&self, to: &Self) -> Result<(), Error> {
        if self.protocol == to.protocol {
            Ok(())
        } else {
            Err(Error::protocol_mismatch(&self.protocol, &to.protocol))
        }
    }
}
