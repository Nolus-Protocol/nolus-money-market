use crate::{protocol::Protocol, protocol_name, protocol_network, release::Id, ProtocolReleaseId};

use super::Release;

impl Release {
    pub const fn current() -> Self {
        const ID: &str = env!(
            "PROTOCOL_RELEASE_ID",
            "No protocol release identifier provided as an environment variable! Please set \
            \"PROTOCOL_RELEASE_ID\" environment variable!",
        );

        Self {
            id: ProtocolReleaseId(Id::new_static(ID)),
            protocol: Protocol::new_static(protocol_name!(), protocol_network!()),
        }
    }
}
