use std::ops::{Deref, DerefMut};

use access_control::ContractOwnerAccess;
use lease::api::MigrateMsg as LeaseMigrateMsg;
use platform::{batch::Batch, contract, message::Response as MessageResponse, reply, response};
#[cfg(feature = "contract-with-bindings")]
use sdk::cosmwasm_std::entry_point;
use sdk::{
    cosmwasm_ext::Response,
    cosmwasm_std::{
        to_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, QuerierWrapper, Reply,
    },
};
use versioning::{version, VersionSegment};

use crate::{
    cmd::Borrow,
    error::ContractError,
    leaser::{self, Leaser},
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SudoMsg},
    result::ContractResult,
    state::{config::Config, leases::Leases},
};

#[cfg(feature = "migration")]
const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 0;
const CONTRACT_STORAGE_VERSION: VersionSegment = 1;

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn instantiate(
    mut deps: DepsMut<'_>,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    contract::validate_addr(&deps.querier, &msg.lpp_ust_addr)?;
    contract::validate_addr(&deps.querier, &msg.time_alarms)?;
    contract::validate_addr(&deps.querier, &msg.market_price_oracle)?;
    contract::validate_addr(&deps.querier, &msg.profit)?;

    versioning::initialize(deps.storage, version!(CONTRACT_STORAGE_VERSION))?;

    ContractOwnerAccess::new(deps.storage.deref_mut()).grant_to(&info.sender)?;

    let lease_code = msg.lease_code_id;
    Config::new(msg).store(deps.storage)?;

    leaser::update_lpp(deps.storage, lease_code.into(), Batch::default())
        .map(response::response_only_messages)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn migrate(deps: DepsMut<'_>, _env: Env, msg: MigrateMsg) -> ContractResult<Response> {
    #[cfg(feature = "migration")]
    let resp =
        versioning::update_software_and_storage::<CONTRACT_STORAGE_VERSION_FROM, _, _, _, _>(
            deps.storage,
            version!(CONTRACT_STORAGE_VERSION),
            |storage: &mut _| {
                use super::state::v0::Config as ConfigOld;
                ConfigOld::migrate(storage, msg.min_asset, msg.min_sell_asset)
                    .and_then(|config_new| config_new.store(storage))
            },
            Into::into,
        )
        .map(|(release_label, ())| release_label);

    #[cfg(not(feature = "migration"))]
    let resp = {
        // Statically assert that the message is empty when doing a software-only update.
        let MigrateMsg {}: MigrateMsg = msg;

        versioning::update_software(deps.storage, version!(CONTRACT_STORAGE_VERSION), Into::into)
    };

    // TODO platform log_error resp.or_else(|err| log_error(err, deps.api))
    resp.and_then(response::response)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<Response> {
    match msg {
        ExecuteMsg::OpenLease { currency, max_ltd } => Borrow::with(
            deps,
            info.funds,
            info.sender,
            env.contract.address.clone(),
            finalizer(env),
            currency,
            max_ltd,
        ),
        ExecuteMsg::FinalizeLease { customer } => {
            validate_customer(customer, deps.api, &deps.querier)
                .and_then(|customer| {
                    validate_lease(info.sender, deps.as_ref()).map(|lease| (customer, lease))
                })
                .and_then(|(customer, lease)| {
                    Leases::remove(deps.storage, customer, &lease).map_err(Into::into)
                })
                .map(|removed| {
                    debug_assert!(removed);
                    MessageResponse::default()
                })
        }
        ExecuteMsg::MigrateLeases {
            new_code_id,
            max_leases,
        } => ContractOwnerAccess::new(deps.storage.deref())
            .check(&info.sender)
            .map_err(Into::into)
            .and_then(move |()| {
                leaser::try_migrate_leases(
                    deps.storage,
                    new_code_id.into(),
                    max_leases,
                    migrate_msg(env),
                )
            }),
        ExecuteMsg::MigrateLeasesCont {
            key: next_customer,
            max_leases,
        } => ContractOwnerAccess::new(deps.storage.deref())
            .check(&info.sender)
            .map_err(Into::into)
            .and_then(|()| validate_customer(next_customer, deps.api, &deps.querier))
            .and_then(move |next_customer_validated| {
                leaser::try_migrate_leases_cont(
                    deps.storage,
                    next_customer_validated,
                    max_leases,
                    migrate_msg(env),
                )
            }),
    }
    .map(response::response_only_messages)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn sudo(deps: DepsMut<'_>, _env: Env, msg: SudoMsg) -> ContractResult<Response> {
    match msg {
        SudoMsg::SetupDex(params) => leaser::try_setup_dex(deps.storage, params),
        SudoMsg::Config {
            lease_interest_rate_margin,
            lease_position_spec,
            lease_interest_payment,
        } => leaser::try_configure(
            deps.storage,
            lease_interest_rate_margin,
            lease_position_spec,
            lease_interest_payment,
        ),
    }
    .map(response::response_only_messages)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn query(deps: Deps<'_>, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&Leaser::new(deps).config()?),
        QueryMsg::Quote {
            downpayment,
            lease_asset,
            max_ltd,
        } => to_binary(&Leaser::new(deps).quote(downpayment, lease_asset, max_ltd)?),
        QueryMsg::Leases { owner } => to_binary(&Leaser::new(deps).customer_leases(owner)?),
    }
    .map_err(Into::into)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn reply(deps: DepsMut<'_>, _env: Env, msg: Reply) -> ContractResult<Response> {
    reply::from_instantiate::<()>(deps.api, msg)
        .map(|r| r.address)
        .map_err(|err| ContractError::ParseError {
            err: err.to_string(),
        })
        .and_then(|lease| {
            Leases::save(deps.storage, lease.clone()).map(|stored| {
                debug_assert!(stored);
                lease
            })
        })
        .map(|lease| Response::new().add_attribute("lease_address", lease))
}

fn validate_customer(
    customer: Addr,
    api: &dyn Api,
    querier: &QuerierWrapper<'_>,
) -> ContractResult<Addr> {
    api.addr_validate(customer.as_str())
        .map_err(|_| ContractError::InvalidContinuationKey {
            err: "invalid address".into(),
        })
        .and_then(|next_customer| {
            contract::validate_addr(querier, &next_customer)
                .is_err()
                .then_some(next_customer)
                .ok_or_else(|| ContractError::InvalidContinuationKey {
                    err: "smart contract key".into(),
                })
        })
}

fn validate_lease(lease: Addr, deps: Deps<'_>) -> ContractResult<Addr> {
    Leaser::new(deps)
        .config()
        .map(|config| config.config.lease_code_id)
        .and_then(|lease_code_id| {
            contract::validate_code_id(&deps.querier, &lease, lease_code_id).map_err(Into::into)
        })
        .map(|()| lease)
}

fn migrate_msg(env: Env) -> impl Fn(Addr) -> LeaseMigrateMsg {
    let finalizer = finalizer(env);
    move |customer| LeaseMigrateMsg::new(customer, finalizer.clone())
}

fn finalizer(env: Env) -> Addr {
    env.contract.address
}
