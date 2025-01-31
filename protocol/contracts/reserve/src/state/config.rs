use std::mem;

use serde::{Deserialize, Serialize};

use platform::contract::Code;
use sdk::{cosmwasm_std::Storage, cw_storage_plus::Item};

use crate::error::Result;

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Config {
    lease_code: Code,
}

impl Config {
    const STORAGE: Item<Self> = Item::new("config");

    pub const fn new(lease_code: Code) -> Self {
        Self { lease_code }
    }

    pub const fn lease_code(&self) -> Code {
        self.lease_code
    }

    pub fn store(&self, storage: &mut dyn Storage) -> Result<()> {
        Self::STORAGE.save(storage, self).map_err(Into::into)
    }

    pub fn load(storage: &dyn Storage) -> Result<Self> {
        Self::STORAGE.load(storage).map_err(Into::into)
    }

    pub fn update_lease_code(storage: &mut dyn Storage, lease_code: Code) -> Result<()> {
        Self::STORAGE
            .update(storage, |_config: Self| Ok(Self::new(lease_code)))
            .map(mem::drop)
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::MockStorage, Storage};
    use platform::contract::{Code, CodeId};

    use super::Config;

    #[test]
    fn store_load() {
        let lease_code = Code::unchecked(12);
        let mut store = MockStorage::new();
        assert_eq!(Ok(()), Config::new(lease_code).store(&mut store));
        assert_lease_code_id(lease_code, &store);
    }

    #[test]
    fn update_load() {
        let lease_code_id = Code::unchecked(28);
        let new_lease_code_id = Code::unchecked(CodeId::from(lease_code_id) + 10);
        let mut store = MockStorage::new();
        assert_eq!(Ok(()), Config::new(lease_code_id).store(&mut store));
        assert_eq!(
            Ok(()),
            Config::update_lease_code(&mut store, new_lease_code_id)
        );
        assert_lease_code_id(new_lease_code_id, &store);
    }

    fn assert_lease_code_id(lease_code: Code, store: &dyn Storage) {
        assert_eq!(lease_code, Config::load(store).unwrap().lease_code())
    }
}
