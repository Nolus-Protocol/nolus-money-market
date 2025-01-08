use std::ops::{Deref, DerefMut};

use access_control::SingleUserAccess;
use admin_contract::msg::{
    ProtocolQueryResponse, ProtocolsQueryResponse, QueryMsg as ProtocolsRegistry,
};
use currency::platform::PlatformGroup;
use finance::{duration::Duration, percent::Percent};
use platform::{batch::Batch, message::Response as MessageResponse, response};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{
        entry_point, to_json_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo,
        QuerierWrapper, Storage, Timestamp,
    },
};
use timealarms::stub::TimeAlarmsRef;
use versioning::{package_version, SemVer, Version, VersionSegment};

use crate::{
    cmd::RewardCalculator,
    msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SudoMsg},
    pool::{Pool, PoolImpl},
    result::ContractResult,
    state::{Config, DispatchLog},
    ContractError,
};

const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 0;
const CONTRACT_STORAGE_VERSION: VersionSegment = CONTRACT_STORAGE_VERSION_FROM + 1;
const PACKAGE_VERSION: SemVer = package_version!();
const CONTRACT_VERSION: Version = Version::new(CONTRACT_STORAGE_VERSION, PACKAGE_VERSION);

#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<CwResponse> {
    versioning::initialize(deps.storage, CONTRACT_VERSION).map_err(ContractError::InitVersion)?;
    setup_dispatching(deps.storage, deps.querier, deps.api, env, msg)
        .map(response::response_only_messages)
}

#[entry_point]
pub fn migrate(deps: DepsMut<'_>, _env: Env, _msg: MigrateMsg) -> ContractResult<CwResponse> {
    versioning::update_legacy_software(deps.storage, CONTRACT_VERSION, Into::into)
        .and_then(response::response)
}

#[entry_point]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<CwResponse> {
    match msg {
        ExecuteMsg::TimeAlarm {} => {
            SingleUserAccess::new(
                deps.storage.deref(),
                crate::access_control::TIMEALARMS_NAMESPACE,
            )
            .check(&info.sender)?;

            try_dispatch(deps, &env, info.sender).map(response::response_only_messages)
        }
    }
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, _env: Env, msg: SudoMsg) -> ContractResult<CwResponse> {
    match msg {
        SudoMsg::Config { cadence_hours } => {
            Config::update_cadence_hours(deps.storage, cadence_hours)
                .map(|()| response::empty_response())
        }
        SudoMsg::Rewards { tvl_to_apr } => {
            Config::update_tvl_to_apr(deps.storage, tvl_to_apr).map(|()| response::empty_response())
        }
    }
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            to_json_binary(&query_config(deps.storage)?).map_err(ContractError::Serialize)
        }
        QueryMsg::CalculateRewards {} => query_reward_apr(deps.storage, deps.querier, &env)
            .and_then(|ref apr| to_json_binary(apr).map_err(ContractError::Serialize)),
    }
}

fn try_load_config(storage: &dyn Storage) -> ContractResult<Config> {
    Config::load(storage).map_err(ContractError::LoadConfig)
}

fn query_config(storage: &dyn Storage) -> ContractResult<ConfigResponse> {
    try_load_config(storage).map(|Config { cadence_hours, .. }| ConfigResponse { cadence_hours })
}

fn try_build_reward<'q>(
    config: Config,
    querier: QuerierWrapper<'q>,
    env: &'q Env,
) -> ContractResult<RewardCalculator<impl Pool + 'q>> {
    protocols(config.protocols_registry, querier).and_then(|protocols| {
        let pools: Result<Vec<_>, _> = protocols
            .into_iter()
            .map(|protocol| {
                PoolImpl::new(
                    lpp_platform::new_stub(protocol.contracts.lpp, querier, env),
                    oracle_platform::new_unchecked_stable_quote_stub::<PlatformGroup, _>(
                        protocol.contracts.oracle,
                        querier,
                    ),
                )
            })
            .collect();

        pools.map(|pools| RewardCalculator::new(pools, &config.tvl_to_apr))
    })
}

fn query_reward_apr(
    storage: &dyn Storage,
    querier: QuerierWrapper<'_>,
    env: &Env,
) -> ContractResult<Percent> {
    try_load_config(storage)
        .and_then(|config| try_build_reward(config, querier, env))
        .map(|rewards| rewards.apr())
}

fn try_dispatch(deps: DepsMut<'_>, env: &Env, timealarm: Addr) -> ContractResult<MessageResponse> {
    let now = env.block.time;

    let config = try_load_config(deps.storage)?;
    let setup_alarm = setup_alarm(
        timealarm,
        &now,
        Duration::from_hours(config.cadence_hours),
        deps.querier,
    )?;

    let last_dispatch = DispatchLog::last_dispatch(deps.storage);
    DispatchLog::update(deps.storage, env.block.time)?;
    let rewards_span = Duration::between(&last_dispatch, &now);

    try_build_reward(config, deps.querier, env)
        .and_then(|reward| reward.distribute(rewards_span))
        .map(|dispatch_res| dispatch_res.merge_with(MessageResponse::messages_only(setup_alarm)))
}

fn protocols(
    protocols_registry: Addr,
    querier: QuerierWrapper<'_>,
) -> ContractResult<impl IntoIterator<Item = ProtocolQueryResponse>> {
    querier
        .query_wasm_smart(protocols_registry.clone(), &ProtocolsRegistry::Protocols {})
        .map_err(ContractError::QueryProtocols)
        .and_then(|protocols: ProtocolsQueryResponse| {
            protocols
                .into_iter()
                .map(|protocol| {
                    querier
                        .query_wasm_smart::<ProtocolQueryResponse>(
                            protocols_registry.clone(),
                            &ProtocolsRegistry::Protocol(protocol),
                        )
                        .map_err(ContractError::QueryProtocols)
                })
                .collect::<ContractResult<Vec<_>>>()
        })
        .map_err(Into::into)
}

fn setup_alarm(
    timealarm: Addr,
    now: &Timestamp,
    alarm_in: Duration,
    querier: QuerierWrapper<'_>,
) -> ContractResult<Batch> {
    TimeAlarmsRef::new(timealarm, querier)
        .map_err(ContractError::SetupTimeAlarmStub)
        .and_then(|stub| {
            stub.setup_alarm(now + alarm_in)
                .map_err(ContractError::SetupTimeAlarm)
        })
}

fn setup_dispatching(
    mut storage: &mut dyn Storage,
    querier: QuerierWrapper<'_>,
    api: &dyn Api,
    env: Env,
    msg: InstantiateMsg,
) -> ContractResult<impl Into<MessageResponse>> {
    // cannot validate the address since the Admin plays the role of the registry
    // and it is not yet instantiated
    api.addr_validate(msg.protocols_registry.as_str())
        .map_err(ContractError::ValidateRegistryAddr)?;
    platform::contract::validate_addr(querier, &msg.timealarms)
        .map_err(ContractError::ValidateTimeAlarmsAddr)?;

    SingleUserAccess::new(
        storage.deref_mut(),
        crate::access_control::TIMEALARMS_NAMESPACE,
    )
    .grant_to(&msg.timealarms)?;

    Config::new(msg.cadence_hours, msg.protocols_registry, msg.tvl_to_apr)
        .store(storage)
        .map_err(ContractError::SaveConfig)?;
    DispatchLog::update(storage, env.block.time)?;

    setup_alarm(
        msg.timealarms,
        &env.block.time,
        Duration::from_hours(msg.cadence_hours),
        querier,
    )
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::MessageInfo;
    use finance::percent::Percent;
    use sdk::{
        cosmwasm_ext::Response as CwResponse,
        cosmwasm_std::{
            coins, from_json,
            testing::{mock_dependencies_with_balance, mock_env},
            DepsMut,
        },
        testing,
    };

    use crate::{
        contract::sudo,
        msg::{ConfigResponse, InstantiateMsg, QueryMsg, SudoMsg},
        state::reward_scale::{Bar, RewardScale, TotalValueLocked},
    };

    use super::{instantiate, query};

    const PROTOCOLS_REGISTRY_ADDR: &str = "admin";
    const TIMEALARMS_ADDR: &str = "timealarms";
    const TREASURY_ADDR: &str = "treasury";

    fn do_instantiate(deps: DepsMut<'_>) {
        let msg = InstantiateMsg {
            cadence_hours: 10,
            protocols_registry: testing::user(PROTOCOLS_REGISTRY_ADDR),
            timealarms: testing::user(TIMEALARMS_ADDR),
            tvl_to_apr: RewardScale::try_from(vec![
                Bar {
                    tvl: TotalValueLocked::new(0),
                    apr: Percent::from_permille(5),
                },
                Bar {
                    tvl: TotalValueLocked::new(1000),
                    apr: Percent::from_permille(10),
                },
            ])
            .unwrap(),
        };
        let info = MessageInfo {
            sender: testing::user("creator"),
            funds: vec![cosmwasm_std::coin(1000, "unolus")],
        };

        let res: CwResponse = instantiate(deps, mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());
    }

    #[test]
    fn proper_initialization() {
        let mut deps = testing::customized_mock_deps_with_contracts(
            mock_dependencies_with_balance(&coins(2, "token")),
            [
                testing::user(PROTOCOLS_REGISTRY_ADDR),
                testing::user(TIMEALARMS_ADDR),
                testing::user(TREASURY_ADDR),
            ],
        );
        do_instantiate(deps.as_mut());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_json(res).unwrap();
        assert_eq!(10, value.cadence_hours);
    }

    #[test]
    fn configure() {
        let mut deps = testing::customized_mock_deps_with_contracts(
            mock_dependencies_with_balance(&coins(2, "token")),
            [
                testing::user(PROTOCOLS_REGISTRY_ADDR),
                testing::user(TIMEALARMS_ADDR),
                testing::user(TREASURY_ADDR),
            ],
        );

        do_instantiate(deps.as_mut());

        let CwResponse {
            messages,
            attributes,
            events,
            data,
            ..
        }: CwResponse = sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::Config { cadence_hours: 12 },
        )
        .unwrap();

        assert_eq!(messages.len(), 0);
        assert_eq!(attributes.len(), 0);
        assert_eq!(events.len(), 0);
        assert_eq!(data, None);

        // should now be 12
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_json(res).unwrap();
        assert_eq!(value.cadence_hours, 12);

        let CwResponse {
            messages,
            attributes,
            events,
            data,
            ..
        }: CwResponse = sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::Config { cadence_hours: 20 },
        )
        .unwrap();

        assert_eq!(messages.len(), 0);
        assert_eq!(attributes.len(), 0);
        assert_eq!(events.len(), 0);
        assert_eq!(data, None);

        // should now be 12
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_json(res).unwrap();
        assert_eq!(value.cadence_hours, 20);
    }
}
