use serde::{Deserialize, Serialize};

use platform::batch::Batch;
use sdk::cosmwasm_std::{Addr, QuerierWrapper};

use crate::{api::FinalizerExecuteMsg, error::ContractResult};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FinalizerRef {
    addr: Addr,
}

impl FinalizerRef {
    pub(super) fn try_new(addr: Addr, querier: QuerierWrapper<'_>) -> ContractResult<Self> {
        use platform::contract;

        contract::validate_addr(querier, &addr)
            .map(|()| Self { addr })
            .map_err(Into::into)
    }

    pub(super) fn notify(&self, customer: Addr) -> ContractResult<Batch> {
        Batch::default()
            .schedule_execute_wasm_no_reply_no_funds(
                self.addr.clone(),
                &FinalizerExecuteMsg::FinalizeLease { customer },
            )
            .map_err(Into::into)
    }
}
