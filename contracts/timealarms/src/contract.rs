use platform::{
    batch::{Emit, Emitter},
    response,
};
#[cfg(feature = "contract-with-bindings")]
use sdk::cosmwasm_std::entry_point;
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{
        to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Storage, SubMsgResult,
    },
};
use versioning::{package_version, version, VersionSegment};

use crate::{
    alarms::TimeAlarms,
    msg::{DispatchAlarmsResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SudoMsg},
    result::ContractResult,
};

// version info for migration info
// const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 0;
const CONTRACT_STORAGE_VERSION: VersionSegment = 1;

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn instantiate(
    deps: DepsMut<'_>,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> ContractResult<CwResponse> {
    versioning::initialize(deps.storage, version!(CONTRACT_STORAGE_VERSION))?;

    Ok(response::empty_response())
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn migrate(deps: DepsMut<'_>, _env: Env, _msg: MigrateMsg) -> ContractResult<CwResponse> {
    versioning::update_software(deps.storage, version!(CONTRACT_STORAGE_VERSION))
        .and_then(response::response)
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> ContractResult<CwResponse> {
    let mut time_alarms: TimeAlarms<'_, &mut dyn Storage> = TimeAlarms::new(deps.storage);

    match msg {
        ExecuteMsg::AddAlarm { time } => time_alarms
            .try_add(&deps.querier, &env, info.sender, time)
            .map(response::response_only_messages),
        ExecuteMsg::DispatchAlarms { max_count } => time_alarms
            .try_notify(env.block.time, max_count)
            .and_then(|(total, resp)| {
                response::response_with_messages(&DispatchAlarmsResponse(total), resp)
            }),
    }
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn sudo(_deps: DepsMut<'_>, _env: Env, msg: SudoMsg) -> ContractResult<CwResponse> {
    match msg {}
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::ContractVersion {} => Ok(to_binary(&package_version!())?),
        QueryMsg::AlarmsStatus {} => Ok(to_binary(
            &TimeAlarms::new(deps.storage).try_any_alarm(env.block.time)?,
        )?),
    }
}

#[cfg_attr(feature = "contract-with-bindings", entry_point)]
pub fn reply(deps: DepsMut<'_>, env: Env, msg: Reply) -> ContractResult<CwResponse> {
    const EVENT_TYPE: &str = "time-alarm";
    const KEY_DELIVERED: &str = "delivered";
    const KEY_DETAILS: &str = "details";

    let emitter: Emitter = Emitter::of_type(EVENT_TYPE);

    let mut time_alarms: TimeAlarms<'_, &mut dyn Storage> = TimeAlarms::new(deps.storage);

    Ok(response::response_only_messages(match msg.result {
        SubMsgResult::Ok(_) => {
            time_alarms.last_delivered()?;

            emitter.emit(KEY_DELIVERED, "success")
        }
        SubMsgResult::Err(err) => {
            time_alarms.last_failed(env.block.time)?;

            emitter.emit(KEY_DELIVERED, "error").emit(KEY_DETAILS, err)
        }
    }))
}

#[cfg(test)]
mod tests {
    use sdk::cosmwasm_std::{
        coins,
        testing::{mock_dependencies, mock_env, mock_info},
    };

    use crate::msg::InstantiateMsg;

    use super::instantiate;

    #[test]
    fn proper_initialization() {
        let msg = InstantiateMsg {};
        let mut deps = mock_dependencies();
        let info = mock_info("CREATOR", &coins(1000, "token"));
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    }
}
