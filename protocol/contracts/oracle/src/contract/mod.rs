use cosmwasm_std::Timestamp;
use currencies::{
    LeaseGroup as AlarmCurrencies, Lpn as BaseCurrency, Lpns as BaseCurrencies,
    PaymentGroup as PriceCurrencies, Stable as StableCurrency,
};
use platform::{
    batch::{Emit, Emitter},
    error as platform_error, response,
};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{
        entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Storage, SubMsgResult,
    },
};
use serde::Serialize;
use versioning::{package_version, SemVer, Version, VersionSegment};

use crate::{
    api::{
        Config, ExecuteMsg, InstantiateMsg, MigrateMsg, PricesResponse, QueryMsg, SudoMsg,
        SwapTreeResponse,
    },
    contract::{alarms::MarketAlarms, oracle::Oracle as GenericOracle},
    error::ContractError,
    result::ContractResult,
    state::supported_pairs::SupportedPairs,
};

use self::{config::query_config, oracle::feeder::Feeders};

mod alarms;
mod config;
pub mod exec;
mod oracle;

const CONTRACT_STORAGE_VERSION_FROM: VersionSegment = 2;
const CONTRACT_STORAGE_VERSION: VersionSegment = CONTRACT_STORAGE_VERSION_FROM + 1;
const PACKAGE_VERSION: SemVer = package_version!();
const CONTRACT_VERSION: Version = Version::new(CONTRACT_STORAGE_VERSION, PACKAGE_VERSION);

type Oracle<'storage, S> =
    GenericOracle<'storage, S, PriceCurrencies, BaseCurrency, BaseCurrencies>;

#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg<PriceCurrencies>,
) -> ContractResult<CwResponse> {
    versioning::initialize(deps.storage, CONTRACT_VERSION)
        .map_err(ContractError::InitializeVersioning)?;

    msg.config
        .store(deps.storage)
        .and_then(|()| {
            SupportedPairs::<PriceCurrencies, BaseCurrency>::new::<StableCurrency>(
                msg.swap_tree.into_tree(),
            )
        })
        .and_then(|supported_pairs| supported_pairs.save(deps.storage))
        .and_then(|()| validate_swap_tree(deps.storage, env.block.time))
        .map(|()| response::empty_response())
}

#[entry_point]
pub fn migrate(
    deps: DepsMut<'_>,
    env: Env,
    MigrateMsg {}: MigrateMsg,
) -> ContractResult<CwResponse> {
    versioning::update_legacy_software(
        deps.storage,
        CONTRACT_VERSION,
        ContractError::UpdateSoftware,
    )
    .and_then(|out| validate_swap_tree(deps.storage, env.block.time).map(|()| out))
    .and_then(response::response)
    .inspect_err(platform_error::log(deps.api))
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg<PriceCurrencies>) -> ContractResult<Binary> {
    match msg {
        QueryMsg::ContractVersion {} => to_json_binary(&PACKAGE_VERSION),
        QueryMsg::Config {} => to_json_binary(&query_config(deps.storage)?),
        QueryMsg::Feeders {} => Feeders::get(deps.storage)
            .map_err(ContractError::LoadFeeders)
            .and_then(|ref feeders| to_json_binary(feeders)),
        QueryMsg::IsFeeder { address } => Feeders::is_feeder(deps.storage, &address)
            .map_err(ContractError::LoadFeeders)
            .and_then(|ref f| to_json_binary(&f)),
        QueryMsg::BaseCurrency {} => {
            to_json_binary(&currency::dto::<BaseCurrency, BaseCurrencies>())
        }
        QueryMsg::StableCurrency {} => {
            to_json_binary(&currency::dto::<StableCurrency, PriceCurrencies>())
        }
        QueryMsg::SupportedCurrencyPairs {} => to_json_binary(
            &SupportedPairs::<PriceCurrencies, BaseCurrency>::load(deps.storage)?
                .swap_pairs_df()
                .collect::<Vec<_>>(),
        ),
        QueryMsg::Currencies {} => to_json_binary(
            &SupportedPairs::<PriceCurrencies, BaseCurrency>::load(deps.storage)?
                .currencies()
                .collect::<Vec<_>>(),
        ),
        QueryMsg::BasePrice { currency } => to_json_binary(
            &Oracle::load(deps.storage)?.try_query_base_price(env.block.time, &currency)?,
        ),
        QueryMsg::StablePrice { currency } => to_json_binary(
            &Oracle::load(deps.storage)?
                .try_query_stable_price::<StableCurrency>(env.block.time, &currency)?,
        ),
        QueryMsg::Prices {} => {
            let prices = Oracle::load(deps.storage)?.try_query_prices(env.block.time)?;

            to_json_binary(&PricesResponse { prices })
        }
        QueryMsg::SwapPath { from, to } => to_json_binary(
            &SupportedPairs::<PriceCurrencies, BaseCurrency>::load(deps.storage)?
                .load_swap_path(&from, &to)?,
        ),
        QueryMsg::SwapTree {} => to_json_binary(&SwapTreeResponse::<PriceCurrencies> {
            tree: SupportedPairs::<PriceCurrencies, BaseCurrency>::load(deps.storage)?
                .query_swap_tree()
                .into_human_readable(),
        }),
        QueryMsg::AlarmsStatus {} => {
            to_json_binary(&Oracle::load(deps.storage)?.try_query_alarms(env.block.time)?)
        }
    }
}

#[entry_point]
pub fn execute(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<BaseCurrency, BaseCurrencies, AlarmCurrencies, PriceCurrencies>,
) -> ContractResult<CwResponse> {
    exec::do_executute(deps, env, msg, info.sender)
}

#[entry_point]
pub fn sudo(
    deps: DepsMut<'_>,
    env: Env,
    msg: SudoMsg<PriceCurrencies>,
) -> ContractResult<CwResponse> {
    match msg {
        SudoMsg::UpdateConfig(price_config) => Config::update(deps.storage, price_config),
        SudoMsg::RegisterFeeder { feeder_address } => Feeders::try_register(deps, feeder_address),
        SudoMsg::RemoveFeeder { feeder_address } => Feeders::try_remove(deps, feeder_address),
        SudoMsg::SwapTree { tree } => {
            SupportedPairs::<PriceCurrencies, BaseCurrency>::new::<StableCurrency>(tree.into_tree())
                .and_then(|supported_pairs| supported_pairs.save(deps.storage))
                .and_then(|()| validate_swap_tree(deps.storage, env.block.time))
            // TODO move the swap tree validation at the tree instantiation
        }
    }
    .map(|()| response::empty_response())
}

// TODO: compare gas usage of this solution vs reply on error
#[entry_point]
pub fn reply(deps: DepsMut<'_>, _env: Env, msg: Reply) -> ContractResult<CwResponse> {
    const EVENT_TYPE: &str = "market-alarm";
    const KEY_DELIVERED: &str = "delivered";
    const KEY_DETAILS: &str = "details";

    let mut alarms: MarketAlarms<'_, &mut (dyn Storage + '_), PriceCurrencies> =
        MarketAlarms::new(deps.storage);

    let emitter: Emitter = Emitter::of_type(EVENT_TYPE);

    match msg.result {
        SubMsgResult::Ok(_) => alarms
            .last_delivered()
            .map(|()| emitter.emit(KEY_DELIVERED, "success")),
        SubMsgResult::Err(error) => alarms.last_failed().map(|()| {
            emitter
                .emit(KEY_DELIVERED, "error")
                .emit(KEY_DETAILS, error)
        }),
    }
    .map(response::response_only_messages)
}

fn validate_swap_tree(store: &dyn Storage, now: Timestamp) -> ContractResult<()> {
    // we use calculation of all prices since it does not add a significant overhead over the swap tree validation
    // otherwise we would have to implement a separate and mostly mirroring algorithm
    Oracle::load(store)
        .and_then(|oracle| {
            oracle
                .try_query_prices(now)
                .map_err(|e| ContractError::BrokenSwapTree(e.to_string()))
        })
        .map(std::mem::drop)
}

fn to_json_binary<T>(data: &T) -> ContractResult<Binary>
where
    T: Serialize + ?Sized,
{
    cosmwasm_std::to_json_binary(data).map_err(ContractError::ConvertToBinary)
}

#[cfg(test)]
mod tests {
    use currencies::{
        testing::{LeaseC1, PaymentC1, PaymentC9},
        LeaseGroup, Lpn, Lpns,
    };
    use finance::{duration::Duration, percent::Percent, price};
    use sdk::cosmwasm_std::{self, testing::mock_env};

    use crate::{
        api::{swap::SwapTarget, Alarm, Config, ExecuteMsg, QueryMsg, SwapLeg},
        contract::query,
        test_tree,
        tests::{dummy_instantiate_msg, setup_test},
    };

    use super::{AlarmCurrencies, BaseCurrencies, BaseCurrency, PriceCurrencies};

    #[test]
    fn proper_initialization() {
        use marketprice::config::Config as PriceConfig;
        let msg = dummy_instantiate_msg(
            60,
            Percent::from_percent(50),
            test_tree::minimal_swap_tree(),
        );
        let (deps, _info) = setup_test(msg);

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: Config = cosmwasm_std::from_json(res).unwrap();
        assert_eq!(
            Config {
                price_config: PriceConfig::new(
                    Percent::from_percent(50),
                    Duration::from_secs(60),
                    1,
                    Percent::from_percent(88),
                ),
            },
            value
        );

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SupportedCurrencyPairs {},
        )
        .unwrap();
        let value: Vec<SwapLeg<PriceCurrencies>> = cosmwasm_std::from_json(res).unwrap();

        let expected = vec![SwapLeg {
            from: currency::dto::<PaymentC9, PriceCurrencies>(),
            to: SwapTarget {
                pool_id: 1,
                target: currency::dto::<Lpn, PriceCurrencies>(),
            },
        }];

        assert_eq!(expected, value);
    }

    #[test]
    fn impl_swap_path() {
        use crate::api::swap::QueryMsg as QueryMsgApi;

        let from = currency::dto::<PaymentC1, PriceCurrencies>().into_super_group();
        let to = currency::dto::<Lpn, PriceCurrencies>().into_super_group();
        let query_impl = QueryMsg::SwapPath { from, to };
        let query_api = cosmwasm_std::from_json::<QueryMsgApi<PriceCurrencies>>(
            &cosmwasm_std::to_json_vec(&query_impl).unwrap(),
        )
        .unwrap();
        assert_eq!(QueryMsgApi::SwapPath { from, to }, query_api);
    }

    #[test]
    fn impl_add_price_alarm() {
        use crate::api::alarms::ExecuteMsg as ExecuteMsgApi;

        let alarm = Alarm::<AlarmCurrencies, BaseCurrency, BaseCurrencies>::new(
            price::total_of::<LeaseC1>(10.into()).is::<BaseCurrency>(1.into()),
            Some(price::total_of(7.into()).is(1.into())),
        );
        let query_impl = ExecuteMsg::<
            BaseCurrency,
            BaseCurrencies,
            AlarmCurrencies,
            PriceCurrencies,
        >::AddPriceAlarm {
            alarm: alarm.clone(),
        };
        let query_api = cosmwasm_std::from_json::<ExecuteMsgApi<LeaseGroup, Lpn, Lpns>>(
            &cosmwasm_std::to_json_vec(&query_impl).unwrap(),
        )
        .unwrap();
        assert_eq!(
            ExecuteMsgApi::AddPriceAlarm::<LeaseGroup, Lpn, Lpns> { alarm },
            query_api
        );
    }
}
