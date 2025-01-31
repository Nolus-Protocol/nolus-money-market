use std::{
    borrow::Cow,
    fmt::{Display, Formatter, Result as FmtResult},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
/// A 'reference type' representing a software package
pub struct Protocol {
    /// the protocol name
    ///
    /// It is a part of the protocol id.
    /// See [`crate::software::ReferenceId`] doc on the need to use [`Cow`]
    name: Cow<'static, str>,

    /// the reference identification attribute
    network: Cow<'static, str>,
}

#[macro_export]
macro_rules! protocol_name {
    () => {{
        ::core::env!(
            "PROTOCOL_NAME",
            "The protocol name is not set as an environment variable!"
        )
    }};
}

#[macro_export]
macro_rules! protocol_network {
    () => {{
        ::core::env!(
            "PROTOCOL_NETWORK",
            "The protocol network is not set as an environment variable!"
        )
    }};
}

impl Protocol {
    pub const fn new_static(name: &'static str, network: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            network: Cow::Borrowed(network),
        }
    }
}

impl Display for Protocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!(
            "name: {}, network: {}",
            self.name, self.network
        ))
    }
}
