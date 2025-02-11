use serde::{Deserialize, Serialize};

use platform::bank::LazySenderStub;
use sdk::cosmwasm_std::{Addr, QuerierWrapper, StdError};
use thiserror::Error;

use crate::msg::{ConfigResponse, QueryMsg};

pub type ProfitStub = LazySenderStub;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "stub_testing", derive(PartialEq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProfitRef {
    addr: Addr,
}

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("[Profit] [Std] {0}")]
    Std(#[from] StdError),
}

impl ProfitRef {
    pub fn new(addr: Addr, querier: &QuerierWrapper<'_>) -> Result<Self, Error> {
        querier
            .query_wasm_smart(addr.clone(), &QueryMsg::Config {})
            .map(|_: ConfigResponse| Self { addr })
            .map_err(Into::into)
    }

    pub fn into_stub(self) -> ProfitStub {
        ProfitStub::new(self.addr)
    }
}

#[cfg(feature = "stub_testing")]
impl ProfitRef {
    pub fn unchecked<A>(addr: A) -> Self
    where
        A: Into<String>,
    {
        Self {
            addr: Addr::unchecked(addr),
        }
    }
}
