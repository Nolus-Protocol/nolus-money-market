use cosmwasm_std::{QuerierWrapper, Timestamp};
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

use ::currency::lease::LeaseGroup;
use finance::currency;
#[cfg(feature = "contract-with-bindings")]
use sdk::cosmwasm_std::entry_point;
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply},
    neutron_sdk::sudo::msg::SudoMsg,
};
use versioning::{version, VersionSegment};

use crate::{
    api::{
        dex::ConnectionParams, ExecuteMsg, MigrateMsg, NewLeaseContract, StateQuery, StateResponse,
    },
    contract::{state::Controller, state::Response},
    error::{ContractError, ContractResult},
    lease::LeaseDTO,
};

use self::{
    dex::{Account, DexConnectable},
    state::{RequestLoan, State},
};

mod cmd;
mod dex;
pub mod msg;
mod state;

// version info for migration info
// const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 0;
const CONTRACT_STORAGE_VERSION: VersionSegment = 0;

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn instantiate(
    mut deps: DepsMut<'_>,
    _env: Env,
    info: MessageInfo,
    new_lease: NewLeaseContract,
) -> ContractResult<CwResponse> {
    //TODO move the following validation into the deserialization
    currency::validate::<LeaseGroup>(&new_lease.form.currency)?;
    deps.api.addr_validate(new_lease.form.customer.as_str())?;

    platform::contract::validate_addr(&deps.querier, &new_lease.form.time_alarms)?;
    platform::contract::validate_addr(&deps.querier, &new_lease.form.market_price_oracle)?;
    platform::contract::validate_addr(&deps.querier, &new_lease.form.loan.lpp)?;
    platform::contract::validate_addr(&deps.querier, &new_lease.form.loan.profit)?;

    versioning::initialize(deps.storage, version!(CONTRACT_STORAGE_VERSION))?;

    let (batch, next_state) = RequestLoan::new(&mut deps, info, new_lease)?;
    impl_::save(deps.storage, &next_state.into())?;
    Ok(batch.into())
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn migrate(deps: DepsMut<'_>, _env: Env, _msg: MigrateMsg) -> ContractResult<CwResponse> {
    versioning::update_software(deps.storage, version!(CONTRACT_STORAGE_VERSION))?;

    Ok(CwResponse::default())
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn reply(mut deps: DepsMut<'_>, env: Env, msg: Reply) -> ContractResult<CwResponse> {
    impl_::load(deps.storage)?
        .reply(&mut deps, env, msg)
        .and_then(
            |Response {
                 cw_response,
                 next_state,
             }| {
                impl_::save(deps.storage, &next_state)?;

                Ok(cw_response)
            },
        )
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn execute(
    mut deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<CwResponse> {
    impl_::load(deps.storage)?
        .execute(&mut deps, env, info, msg)
        .and_then(
            |Response {
                 cw_response,
                 next_state,
             }| {
                impl_::save(deps.storage, &next_state)?;

                Ok(cw_response)
            },
        )
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn sudo(deps: DepsMut<'_>, env: Env, msg: SudoMsg) -> ContractResult<CwResponse> {
    let state = impl_::load(deps.storage)?;
    match msg {
        SudoMsg::OpenAck {
            port_id: _,
            channel_id: _,
            counterparty_channel_id: _,
            counterparty_version,
        } => state.on_open_ica(counterparty_version, deps.as_ref(), env),
        SudoMsg::Response { request: _, data } => state.on_response(data, deps.as_ref(), env),
        SudoMsg::Timeout { request: _ } => state.on_timeout(deps.as_ref(), env),
        SudoMsg::Error {
            request: _,
            details: _,
        } => state.on_error(deps.as_ref(), env),
        _ => unreachable!(),
    }
    .and_then(
        |Response {
             cw_response,
             next_state,
         }| {
            impl_::save(deps.storage, &next_state)?;

            Ok(cw_response)
        },
    )
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn query(deps: Deps<'_>, env: Env, _msg: StateQuery) -> ContractResult<Binary> {
    let resp = impl_::load(deps.storage)?.state(env.block.time, &deps.querier)?;
    to_binary(&resp).map_err(ContractError::from)
}

#[enum_dispatch]
trait Contract {
    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> ContractResult<StateResponse>;
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Lease {
    lease: LeaseDTO,
    dex: Account,
}

impl DexConnectable for Lease {
    fn dex(&self) -> &ConnectionParams {
        self.dex.dex()
    }
}

mod impl_ {
    use sdk::{
        cosmwasm_std::{StdResult, Storage},
        cw_storage_plus::Item,
    };

    use super::state::State;

    const STATE_DB_KEY: &str = "state";
    const STATE_DB_ITEM: Item<'static, State> = Item::new(STATE_DB_KEY);

    pub(super) fn load(storage: &dyn Storage) -> StdResult<State> {
        STATE_DB_ITEM.load(storage)
    }

    pub(super) fn save(storage: &mut dyn Storage, next_state: &State) -> StdResult<()> {
        STATE_DB_ITEM.save(storage, next_state)
    }
}
