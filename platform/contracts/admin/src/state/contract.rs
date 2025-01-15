use serde::{Deserialize, Serialize};

use platform::contract::CodeId;
use sdk::{
    cosmwasm_std::{Addr, StdResult, Storage},
    cw_storage_plus::Item,
};
use versioning::ReleaseId;

const STORE: Item<Contract> = Item::new("contract_state_machine");

#[derive(Serialize, Deserialize)]
pub(crate) enum Contract {
    AwaitContractsMigrationReply {
        release: ReleaseId,
    },
    Instantiate {
        expected_code_id: CodeId,
        expected_address: Addr,
    },
}

impl Contract {
    pub(crate) fn store(&self, storage: &mut dyn Storage) -> StdResult<()> {
        STORE.save(storage, self)
    }

    pub(crate) fn load(storage: &dyn Storage) -> StdResult<Self> {
        STORE.load(storage)
    }

    pub(crate) fn clear(storage: &mut dyn Storage) {
        STORE.remove(storage)
    }
}
