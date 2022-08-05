use cosmwasm_std::{
    to_binary, Addr, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, Storage, Timestamp,
    WasmMsg,
};
use platform::{
    bank::{BankAccount, BankAccountView, BankStub},
    batch::Batch,
};

use crate::{msg::ConfigResponse, state::config::Config, ContractError};
use finance::{coin::Coin, currency::Nls, duration::Duration};

pub struct Profit {}

impl Profit {
    pub(crate) fn try_config(
        deps: DepsMut,
        info: MessageInfo,
        cadence_hours: u16,
    ) -> Result<Response, ContractError> {
        let config = Config::load(deps.storage)?;
        if info.sender != config.owner {
            return Err(ContractError::Unauthorized {});
        }
        Config::update(deps.storage, cadence_hours)?;

        Ok(Response::new())
    }
    pub(crate) fn transfer(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
    ) -> Result<Response, ContractError> {
        let config = Config::load(deps.storage)?;

        if info.sender != config.timealarms {
            return Err(ContractError::UnrecognisedAlarm(info.sender));
        }

        let balance = deps.querier.query_all_balances(&env.contract.address)?;

        if balance.is_empty() {
            return Ok(Response::new().add_attribute("result", "no profit to dispatch"));
        }

        let current_time = env.block.time;

        let msg = Self::alarm_subscribe_msg(
            &config.timealarms,
            current_time,
            Duration::from_hours(config.cadence_hours),
        )?;

        let mut bank = BankStub::my_account(&env, &deps.querier);
        let balance: Coin<Nls> = bank.balance()?;
        bank.send(balance, &config.treasury);

        let mut batch: Batch = bank.into();
        batch.schedule_execute_no_reply(msg);

        // batch.emit_timestamp(event_type, event_key, timestamp);

        Ok(batch.into())
    }
    pub fn query_config(storage: &dyn Storage) -> StdResult<ConfigResponse> {
        let config = Config::load(storage)?;
        Ok(ConfigResponse {
            cadence_hours: config.cadence_hours,
        })
    }

    pub(crate) fn alarm_subscribe_msg(
        timealarms_addr: &Addr,
        current_time: Timestamp,
        cadence: Duration,
    ) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            funds: vec![],
            contract_addr: timealarms_addr.to_string(),
            msg: to_binary(&timealarms::msg::ExecuteMsg::AddAlarm {
                time: current_time + cadence,
            })?,
        }))
    }
}
