use serde::de::DeserializeOwned;

use currency::lpn::Lpns;
use finance::currency::{visit_any_on_ticker, AnyVisitor, AnyVisitorResult, Currency};
use marketprice::SpotPrice;
use platform::{contract, response};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{Addr, DepsMut, Env, Storage, Timestamp},
};

use crate::{
    contract::{alarms::MarketAlarms, oracle::Oracle},
    error::ContractError,
    msg::{DispatchAlarmsResponse, ExecuteMsg},
    result::ContractResult,
    state::config::Config,
};

use super::oracle::{feed::Feeds, feeder::Feeders};

pub struct ExecWithOracleBase<'a> {
    deps: DepsMut<'a>,
    env: Env,
    msg: ExecuteMsg,
    sender: Addr,
}

impl<'a> ExecWithOracleBase<'a> {
    pub fn cmd(
        deps: DepsMut<'a>,
        env: Env,
        msg: ExecuteMsg,
        sender: Addr,
    ) -> ContractResult<CwResponse> {
        let visitor = Self {
            deps,
            env,
            msg,
            sender,
        };

        let config = Config::load(visitor.deps.storage)?;
        visit_any_on_ticker::<Lpns, _>(&config.base_asset, visitor)
    }
}

impl<'a> AnyVisitor for ExecWithOracleBase<'a> {
    type Output = CwResponse;
    type Error = ContractError;

    fn on<OracleBase>(self) -> AnyVisitorResult<Self>
    where
        OracleBase: Currency + DeserializeOwned,
    {
        match self.msg {
            ExecuteMsg::FeedPrices { prices } => {
                if !Feeders::is_feeder(self.deps.storage, &self.sender)? {
                    return Err(ContractError::UnknownFeeder {});
                }

                try_feed_prices::<OracleBase>(
                    self.deps.storage,
                    self.env.block.time,
                    self.sender,
                    prices,
                )
                .map(|()| Default::default())
            }
            ExecuteMsg::DispatchAlarms { max_count } => {
                Oracle::<_, OracleBase>::load(self.deps.storage)?
                    .try_notify_alarms(self.env.block.time, max_count)
                    .and_then(|(total, resp)| {
                        response::response_with_messages(&DispatchAlarmsResponse(total), resp)
                    })
            }
            ExecuteMsg::AddPriceAlarm { alarm } => {
                contract::validate_addr(&self.deps.querier, &self.sender)?;

                MarketAlarms::new(self.deps.storage)
                    .try_add_price_alarm::<OracleBase>(self.sender, alarm)
                    .map(|()| Default::default())
            }
        }
    }
}

fn try_feed_prices<OracleBase>(
    storage: &mut dyn Storage,
    block_time: Timestamp,
    sender: Addr,
    prices: Vec<SpotPrice>,
) -> ContractResult<()>
where
    OracleBase: Currency + DeserializeOwned,
{
    let config = Config::load(storage)?;
    let oracle = Feeds::<OracleBase>::with(config.price_config);

    oracle
        .feed_prices(storage, block_time, &sender, &prices)
        .map(|()| Default::default())
}
