use access_control::ContractOwnerAccess;
use platform::{batch::Batch, contract::CodeId, response};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{
        self, ensure_eq, entry_point, Addr, Api, Binary, CodeInfoResponse, Deps, DepsMut, Env,
        MessageInfo, QuerierWrapper, Reply, Storage, WasmMsg,
    },
};
use versioning::{package_version, version, ReleaseLabel, SemVer, Version, VersionSegment};

use crate::{
    contracts::{MigrationSpec, Protocol, ProtocolContracts},
    error::Error as ContractError,
    msg::{
        ExecuteMsg, InstantiateMsg, MigrateContracts, MigrateMsg, PlatformQueryResponse,
        ProtocolQueryResponse, ProtocolsQueryResponse, QueryMsg, SudoMsg,
    },
    result::Result as ContractResult,
    state::{contract::Contract as ContractState, contracts as state_contracts},
    validate::Validate as _,
};

// version info for migration info
const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 3;
const CONTRACT_STORAGE_VERSION: VersionSegment = CONTRACT_STORAGE_VERSION_FROM + 1;
const PACKAGE_VERSION: SemVer = package_version!();
const CONTRACT_VERSION: Version = version!(CONTRACT_STORAGE_VERSION, PACKAGE_VERSION);

#[entry_point]
pub fn instantiate(
    mut deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    InstantiateMsg {
        ref dex_admin,
        contracts,
    }: InstantiateMsg,
) -> ContractResult<CwResponse> {
    versioning::initialize(deps.storage, CONTRACT_VERSION)?;

    ContractOwnerAccess::new(deps.branch().storage).grant_to(dex_admin)?;

    contracts.validate(deps.querier)?;

    state_contracts::store(deps.storage, contracts).map(|()| response::empty_response())
}

#[entry_point]
pub fn migrate(
    deps: DepsMut<'_>,
    env: Env,
    MigrateMsg {
        migrate_contracts:
            MigrateContracts {
                release,
                migration_spec,
            },
    }: MigrateMsg,
) -> ContractResult<CwResponse> {
    versioning::update_software(deps.storage, CONTRACT_VERSION, Into::into).and_then(
        |reported_label| {
            check_release_label(reported_label.clone(), release.clone())
                .and_then(|()| {
                    crate::contracts::migrate(
                        deps.storage,
                        env.contract.address,
                        release,
                        migration_spec,
                    )
                })
                .and_then(|messages| response::response_with_messages(reported_label, messages))
        },
    )
}

#[entry_point]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<CwResponse> {
    match msg {
        ExecuteMsg::Instantiate {
            code_id,
            expected_address,
            protocol,
            label,
            message,
        } => {
            ensure_sender_is_owner(deps.storage, &info.sender)?;

            ContractState::Instantiate {
                expected_code_id: code_id.u64(),
                expected_address,
            }
            .store(deps.storage)?;

            let batch = Batch::default().schedule_execute_reply_on_success(
                WasmMsg::Instantiate2 {
                    admin: Some(env.contract.address.into_string()),
                    code_id: code_id.u64(),
                    label,
                    msg: Binary::new(message.into_bytes()),
                    funds: info.funds,
                    salt: Binary::new(protocol.into_bytes()),
                },
                Default::default(),
            );

            Ok(response::response_only_messages(batch))
        }
        ExecuteMsg::RegisterProtocol { name, ref protocol } => {
            ensure_sender_is_owner(deps.storage, &info.sender)?;

            register_protocol(deps.storage, deps.querier, name, protocol)
        }
        ExecuteMsg::DeregisterProtocol(migration_spec) => {
            deregister_protocol(deps.storage, &info.sender, migration_spec)
        }
        ExecuteMsg::EndOfMigration {} => {
            ensure_eq!(
                info.sender,
                env.contract.address,
                ContractError::AccessControl(access_control::error::Error::Unauthorized {})
            );

            ContractState::clear(deps.storage);

            Ok(response::empty_response())
        }
    }
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, env: Env, msg: SudoMsg) -> ContractResult<CwResponse> {
    match msg {
        SudoMsg::ChangeDexAdmin { new_dex_admin } => deps
            .api
            .addr_validate(new_dex_admin.as_str())
            .map_err(Into::into)
            .and_then(|new_dex_admin| {
                ContractOwnerAccess::new(deps.storage)
                    .grant_to(&new_dex_admin)
                    .map(|()| response::empty_response())
                    .map_err(Into::into)
            }),
        SudoMsg::RegisterProtocol { name, ref protocol } => {
            register_protocol(deps.storage, deps.querier, name, protocol)
        }
        SudoMsg::MigrateContracts(MigrateContracts {
            release,
            migration_spec,
        }) => {
            crate::contracts::migrate(deps.storage, env.contract.address, release, migration_spec)
                .map(response::response_only_messages)
        }
        SudoMsg::ExecuteContracts(execute_messages) => {
            crate::contracts::execute(deps.storage, execute_messages)
                .map(response::response_only_messages)
        }
    }
}

#[entry_point]
pub fn reply(deps: DepsMut<'_>, _env: Env, msg: Reply) -> ContractResult<CwResponse> {
    match ContractState::load(deps.storage)? {
        ContractState::AwaitContractsMigrationReply { release } => migration_reply(msg, release),
        ContractState::Instantiate {
            expected_code_id,
            expected_address,
        } => {
            ContractState::clear(deps.storage);

            instantiate_reply(
                deps.api,
                deps.querier,
                msg,
                expected_code_id,
                expected_address,
            )
        }
    }
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::InstantiateAddress { code_id, protocol } => {
            let CodeInfoResponse { checksum, .. } =
                deps.querier.query_wasm_code_info(code_id.u64())?;

            let creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;

            let canonical_addr =
                cosmwasm_std::instantiate2_address(checksum.as_ref(), &creator, protocol.as_ref())?;

            let addr = deps.api.addr_humanize(&canonical_addr)?;

            cosmwasm_std::to_json_binary(&addr).map_err(From::from)
        }
        QueryMsg::Protocols {} => {
            state_contracts::protocols(deps.storage).and_then(|ref protocols| {
                cosmwasm_std::to_json_binary::<ProtocolsQueryResponse>(protocols)
                    .map_err(Into::into)
            })
        }
        QueryMsg::Platform {} => {
            state_contracts::load_platform(deps.storage).and_then(|ref platform| {
                cosmwasm_std::to_json_binary::<PlatformQueryResponse>(platform).map_err(Into::into)
            })
        }
        QueryMsg::Protocol(protocol) => state_contracts::load_protocol(deps.storage, protocol)
            .and_then(|ref protocol| {
                cosmwasm_std::to_json_binary::<ProtocolQueryResponse>(protocol).map_err(Into::into)
            }),
    }
}

fn instantiate_reply(
    api: &dyn Api,
    querier: QuerierWrapper<'_>,
    msg: Reply,
    expected_code_id: CodeId,
    expected_addr: Addr,
) -> ContractResult<CwResponse> {
    let instantiated_addr = platform::reply::from_instantiate2_addr_only(api, msg)?;

    if instantiated_addr != expected_addr {
        return Err(ContractError::DifferentInstantiatedAddress {
            reported: instantiated_addr,
            expected: expected_addr,
        });
    }

    let reported_code_id = querier.query_wasm_contract_info(instantiated_addr)?.code_id;

    if reported_code_id == expected_code_id {
        Ok(response::empty_response())
    } else {
        Err(ContractError::DifferentInstantiatedCodeId {
            reported: reported_code_id,
            expected: expected_code_id,
        })
    }
}

fn ensure_sender_is_owner(storage: &mut dyn Storage, sender: &Addr) -> ContractResult<()> {
    ContractOwnerAccess::new(storage)
        .check(sender)
        .map_err(Into::into)
}

fn register_protocol(
    storage: &mut dyn Storage,
    querier: QuerierWrapper<'_>,
    name: String,
    protocol: &Protocol<Addr>,
) -> ContractResult<CwResponse> {
    protocol.validate(querier)?;

    state_contracts::add_protocol(storage, name, protocol).map(|()| response::empty_response())
}

fn deregister_protocol(
    storage: &mut dyn Storage,
    sender: &Addr,
    migration_spec: ProtocolContracts<MigrationSpec>,
) -> ContractResult<CwResponse> {
    state_contracts::protocols(storage)?
        .into_iter()
        .find_map(|name| {
            state_contracts::load_protocol(storage, name.clone())
                .map(|protocol| {
                    (protocol.contracts.leaser == sender)
                        .then_some(protocol.contracts)
                        .inspect(|_| () = state_contracts::remove_protocol(storage, name))
                })
                .transpose()
        })
        .unwrap_or(Err(ContractError::SenderNotARegisteredLeaser {}))
        .and_then(|protocol| {
            ContractState::AwaitContractsMigrationReply {
                release: ReleaseLabel::void(),
            }
            .store(storage)
            .map(|()| response::response_only_messages(protocol.migrate_standalone(migration_spec)))
            .map_err(Into::into)
        })
}

fn migration_reply(msg: Reply, expected_release: ReleaseLabel) -> ContractResult<CwResponse> {
    platform::reply::from_execute(msg)?
        .ok_or(ContractError::NoMigrationResponseData {})
        .and_then(|reported_release| check_release_label(reported_release, expected_release))
        .map(|()| response::empty_response())
}

fn check_release_label(
    reported_release: ReleaseLabel,
    expected_release: ReleaseLabel,
) -> ContractResult<()> {
    ensure_eq!(
        reported_release,
        expected_release,
        ContractError::WrongRelease {
            reported: reported_release.into(),
            expected: expected_release.into()
        }
    );

    Ok(())
}
