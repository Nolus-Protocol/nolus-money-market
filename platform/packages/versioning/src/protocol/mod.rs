use serde::{Deserialize, Serialize};

pub use protocol_::Protocol;

use crate::{release::Id, Error};

#[cfg(feature = "protocol_contract")]
mod current;
mod protocol_;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Release {
    id: Id,
    protocol: Protocol,
}

impl Release {
    pub fn check_update_allowed(&self, to: &Self, to_release: &Id) -> Result<(), Error> {
        to.check_release_match(to_release)
            .and_then(|()| self.check_protocol_match(to))
    }

    fn check_release_match(&self, target: &Id) -> Result<(), Error> {
        if &self.id == target {
            Ok(())
        } else {
            Err(Error::release_mismatch(&self.id, target))
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
