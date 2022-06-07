#[cfg(feature = "cosmwasm-bindings")]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, to_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Storage, Timestamp,
};
use cw2::set_contract_version;
use time_oracle::Alarms;

use crate::dispatcher::Dispatcher;
use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::config::Config;
use crate::state::dispatch_log::DispatchLog;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const TIME_ALARMS: Alarms = Alarms::new("alarms", "alarms_idx", "alarms_next_id");

#[cfg_attr(feature = "cosmwasm-bindings", entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let lpp_addr = validate_addr(deps.as_ref(), msg.lpp)?;
    let time_oracle_addr = validate_addr(deps.as_ref(), msg.time_oracle)?;
    let market_oracle_addr = validate_addr(deps.as_ref(), msg.market_oracle)?;
    let treasury_addr = validate_addr(deps.as_ref(), msg.treasury)?;

    Config::new(
        info.sender,
        msg.cadence_hours,
        lpp_addr,
        time_oracle_addr,
        treasury_addr,
        market_oracle_addr,
        msg.tvl_to_apr,
    )
    .store(deps.storage)?;
    DispatchLog::update(deps.storage, env.block.time)?;

    try_add_alarm(
        deps.api,
        deps.storage,
        env.contract.address,
        env.block.time.plus_seconds(to_seconds(msg.cadence_hours)),
    )?;

    Ok(Response::new().add_attribute("method", "instantiate"))
}

fn validate_addr(deps: Deps, addr: Addr) -> Result<Addr, ContractError> {
    deps.api
        .addr_validate(addr.as_str())
        .map_err(|_| ContractError::InvalidContractAddress(addr))
}

fn to_seconds(cadence_hours: u32) -> u64 {
    cadence_hours as u64 * 60 * 60
}

#[cfg_attr(feature = "cosmwasm-bindings", entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Config { cadence_hours } => try_config(deps, info, cadence_hours),
        ExecuteMsg::Alarm { time } => try_dispatch(deps, env, info, time),
    }
}

pub fn try_config(
    deps: DepsMut,
    info: MessageInfo,
    cadence_hours: u32,
) -> Result<Response, ContractError> {
    let config = Config::load(deps.storage)?;
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    Config::update(deps.storage, cadence_hours)?;

    Ok(Response::new().add_attribute("method", "config"))
}

#[cfg_attr(feature = "cosmwasm-bindings", entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps.storage)?),
    }
}

fn query_config(storage: &dyn Storage) -> StdResult<ConfigResponse> {
    let config = Config::load(storage)?;
    Ok(ConfigResponse {
        cadence_hours: config.cadence_hours,
    })
}

pub fn try_dispatch(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    time: Timestamp,
) -> Result<Response, ContractError> {
    ensure!(
        time >= env.block.time,
        ContractError::AlarmTimeValidation {}
    );
    let config = Config::load(deps.storage)?;

    if info.sender != config.time_oracle {
        return Err(ContractError::UnrecognisedAlarm(info.sender));
    }

    try_add_alarm(
        deps.api,
        deps.storage,
        env.contract.address,
        env.block
            .time
            .plus_seconds(to_seconds(config.cadence_hours)),
    )?;

    Dispatcher::dispatch(deps, config, env.block.time)
}

fn try_add_alarm(
    api: &dyn Api,
    storage: &mut dyn Storage,
    addr: Addr,
    time: Timestamp,
) -> Result<Response, ContractError> {
    let valid = api
        .addr_validate(addr.as_str())
        .map_err(|_| ContractError::InvalidAlarmAddress(addr))?;
    TIME_ALARMS.add(storage, valid, time)?;
    Ok(Response::new().add_attribute("method", "try_add_alarm"))
}

#[cfg(test)]
mod tests {
    use crate::msg::ConfigResponse;
    use crate::state::tvl_intervals::{Intervals, Stop};

    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, BlockInfo, Coin, SubMsg, WasmMsg};

    fn instantiate_msg() -> InstantiateMsg {
        InstantiateMsg {
            cadence_hours: 10,
            lpp: Addr::unchecked("lpp"),
            time_oracle: Addr::unchecked("time"),
            treasury: Addr::unchecked("treasury"),
            market_oracle: Addr::unchecked("market_oracle"),
            tvl_to_apr: Intervals::from(vec![Stop::new(0, 5), Stop::new(1000000, 10)]).unwrap(),
        }
    }
    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = instantiate_msg();
        let info = mock_info("creator", &coins(1000, "unolus"));

        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(10, value.cadence_hours);
    }

    #[test]
    fn configure() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = instantiate_msg();
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Config { cadence_hours: 20 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Config { cadence_hours: 12 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 12
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(12, value.cadence_hours);

        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Config { cadence_hours: 20 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 12
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(20, value.cadence_hours);
    }

    #[test]
    fn dispatch() {
        let mut deps = mock_dependencies_with_balance(&coins(20, "unolus"));

        let msg = instantiate_msg();
        let info = mock_info("time", &coins(2, "unolus"));
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::Alarm {
            time: mock_env().block.time,
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // lpp period is zero => lpp returns zero reward => no dispatch message is send
        assert_eq!(0, res.messages.len());
    }
    #[test]
    fn dispatch_with_valid_period() {
        let mut deps = mock_dependencies_with_balance(&coins(20, "unolus"));

        let msg = instantiate_msg();
        let info = mock_info("time", &coins(2, "unolus"));
        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let mut env = mock_env();
        env.block = BlockInfo {
            height: 12_345,
            time: env.block.time.plus_seconds(100 * 24 * 60 * 60),
            chain_id: "cosmos-testnet-14002".to_string(),
        };

        let msg = ExecuteMsg::Alarm {
            time: env.block.time,
        };

        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 2);
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(WasmMsg::Execute {
                    contract_addr: "treasury".to_string(),
                    msg: to_binary(&treasury::msg::ExecuteMsg::SendRewards {
                        lpp_addr: Addr::unchecked("lpp"),
                        amount: Coin::new(44386002, "unolus"),
                    })
                    .unwrap(),
                    funds: vec![],
                }),
                SubMsg::new(WasmMsg::Execute {
                    contract_addr: "lpp".to_string(),
                    msg: to_binary(&lpp::msg::ExecuteMsg::DistributeRewards {}).unwrap(),
                    funds: vec![],
                })
            ]
        );
    }
}
