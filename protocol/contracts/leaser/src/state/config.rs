use serde::{Deserialize, Serialize};

use finance::{duration::Duration, percent::Percent};
use lease::api::open::{ConnectionParams, PositionSpecDTO};
use platform::contract::CodeId;
use sdk::{
    cosmwasm_ext::as_dyn::storage,
    cosmwasm_std::Addr,
    cw_storage_plus::Item,
    schemars::{self, JsonSchema},
};

use crate::{msg::InstantiateMsg, result::ContractResult};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
pub struct Config {
    pub lease_code_id: CodeId,
    pub lpp: Addr,
    pub profit: Addr,
    pub time_alarms: Addr,
    pub market_price_oracle: Addr,
    pub lease_position_spec: PositionSpecDTO,
    pub lease_interest_rate_margin: Percent,
    pub lease_due_period: Duration,
    pub dex: ConnectionParams,
}

impl Config {
    const STORAGE: Item<'static, Self> = Item::new("config");

    pub fn new(msg: InstantiateMsg) -> Self {
        Self {
            lease_code_id: msg.lease_code_id.into(),
            lpp: msg.lpp,
            profit: msg.profit,
            time_alarms: msg.time_alarms,
            market_price_oracle: msg.market_price_oracle,
            lease_position_spec: msg.lease_position_spec,
            lease_interest_rate_margin: msg.lease_interest_rate_margin,
            lease_due_period: msg.lease_due_period,
            dex: msg.dex,
        }
    }

    pub fn store<S>(&self, storage: &mut S) -> ContractResult<()>
    where
        S: storage::DynMut + ?Sized,
    {
        Self::STORAGE
            .save(storage.as_dyn_mut(), self)
            .map_err(Into::into)
    }

    pub fn load<S>(storage: &S) -> ContractResult<Self>
    where
        S: storage::Dyn + ?Sized,
    {
        Self::STORAGE.load(storage.as_dyn()).map_err(Into::into)
    }

    pub fn update<S>(
        storage: &mut S,
        lease_interest_rate_margin: Percent,
        lease_position_spec: PositionSpecDTO,
        lease_due_period: Duration,
    ) -> ContractResult<()>
    where
        S: storage::DynMut + ?Sized,
    {
        Self::STORAGE
            .update(storage.as_dyn_mut(), |mut c| -> ContractResult<Config> {
                c.lease_interest_rate_margin = lease_interest_rate_margin;
                c.lease_position_spec = lease_position_spec;
                c.lease_due_period = lease_due_period;
                Ok(c)
            })
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn update_lease_code<S>(storage: &mut S, new_code: CodeId) -> ContractResult<()>
    where
        S: storage::DynMut + ?Sized,
    {
        Self::STORAGE
            .update(storage.as_dyn_mut(), |mut c| -> ContractResult<Config> {
                c.lease_code_id = new_code;
                Ok(c)
            })
            .map(|_| ())
            .map_err(Into::into)
    }
}
