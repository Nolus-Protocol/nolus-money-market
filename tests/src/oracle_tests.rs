use currency::CurrencyDef;
use serde::{Deserialize, Serialize};

use currencies::{
    LeaseC1, LeaseC2, LeaseGroup, LeaseGroup as AlarmCurrencies, Lpn as BaseCurrency, Lpn, Lpns,
    Lpns as BaseCurrencies, PaymentC1, PaymentC4, PaymentC5, PaymentC6, PaymentC7, PaymentGroup,
    PaymentGroup as PriceCurrencies,
};
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    percent::Percent,
    price::{self, base::BasePrice, dto::PriceDTO, Price},
};
use marketprice::config::Config as PriceConfig;
use oracle::{
    api::{
        swap::{SwapPath, SwapTarget},
        AlarmsCount, MigrateMsg, QueryMsg as OracleQ, SudoMsg, SwapTreeResponse,
    },
    result::ContractResult,
    ContractError,
};
use platform::{batch::Batch, coin_legacy, contract::Code};
use sdk::{
    cosmwasm_ext::{InterChainMsg, Response as CwResponse},
    cosmwasm_std::{
        self, coin, Addr, Attribute, Binary, Deps, DepsMut, Env, Event, MessageInfo, Storage,
        Timestamp,
    },
    cw_multi_test::{AppResponse, Contract as CwContract},
    cw_storage_plus::Item,
    testing::{self, CwContractWrapper},
};
use tree::HumanReadableTree;

use crate::common::{
    leaser as leaser_mod, oracle as oracle_mod,
    protocols::Registry,
    test_case::{
        app::App,
        builder::BlankBuilder as TestCaseBuilder,
        response::{RemoteChain as _, ResponseWithInterChainMsgs},
        TestCase,
    },
    CwCoin, ADDON_OPTIMAL_INTEREST_RATE, ADMIN, BASE_INTEREST_RATE, USER, UTILIZATION_OPTIMAL,
};

type LeaseCurrency = LeaseC1;
type TheCoin = Coin<Lpn>;
type BaseC = PaymentC4;
type Alarm = oracle::api::Alarm<AlarmCurrencies, BaseCurrency, BaseCurrencies>;
type ExecuteMsg =
    oracle::api::ExecuteMsg<BaseCurrency, BaseCurrencies, AlarmCurrencies, PriceCurrencies>;

fn cw_coin<CoinT>(coin: CoinT) -> CwCoin
where
    CoinT: Into<Coin<Lpn>>,
{
    coin_legacy::to_cosmwasm(coin.into())
}

fn create_test_case() -> TestCase<Addr, Addr, Addr, Addr, Addr, Addr, Addr, Addr> {
    TestCaseBuilder::<Lpn>::with_reserve(&[cw_coin(10_000_000_000_000_000_000_000_000_000)])
        .init_lpp_with_funds(
            None,
            &[coin(5_000_000_000_000_000_000_000_000_000, Lpn::bank())],
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_protocols_registry(Registry::NoProtocol)
        .init_oracle(Some(
            CwContractWrapper::new(
                oracle::contract::execute,
                oracle::contract::instantiate,
                oracle::contract::query,
            )
            .with_reply(oracle::contract::reply)
            .with_sudo(oracle::contract::sudo)
            .with_migrate(oracle::contract::migrate),
        ))
        .init_treasury()
        .init_profit(24)
        .init_reserve()
        .init_leaser()
        .into_generic()
}

#[test]
fn test_lease_serde() {
    use lease::api::ExecuteMsg::PriceAlarm as LeasePriceAlarm;
    use oracle::api::ExecuteAlarmMsg::PriceAlarm;
    use sdk::cosmwasm_std;

    let LeasePriceAlarm {} =
        cosmwasm_std::from_json(cosmwasm_std::to_json_vec(&PriceAlarm {}).unwrap()).unwrap()
    else {
        unreachable!()
    };

    let PriceAlarm {} =
        cosmwasm_std::from_json(cosmwasm_std::to_json_vec(&LeasePriceAlarm {}).unwrap()).unwrap();
}

#[test]
fn register_feeder() {
    let mut test_case = create_test_case();
    oracle_mod::add_feeder(&mut test_case, testing::user(ADMIN));
}

#[test]
fn internal_test_integration_setup_test() {
    let mut test_case = create_test_case();

    oracle_mod::add_feeder(&mut test_case, testing::user(ADMIN));

    let response: AppResponse = oracle_mod::feed_price(
        &mut test_case,
        testing::user(ADMIN),
        Coin::<BaseC>::new(5),
        Coin::<Lpn>::new(7),
    );
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("execute")
            .add_attribute("_contract_address", test_case.address_book.oracle())]
    );
}

// test for issue #26. It was resolved in MR !132 by separation of price feeding and alarms delivery processes
#[test]
fn feed_price_with_alarm_issue() {
    let mut test_case = create_test_case();
    oracle_mod::add_feeder(&mut test_case, testing::user(ADMIN));

    let lease = open_lease(&mut test_case, Coin::new(1000));

    // there is no price in the oracle and feed for this alarm
    () = test_case
        .app
        .execute(
            lease,
            test_case.address_book.oracle().clone(),
            &ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<PaymentC4>::new(1)).is(Coin::<Lpn>::new(1)),
                    None,
                ),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let _: AppResponse = oracle_mod::feed_price(
        &mut test_case,
        testing::user(ADMIN),
        Coin::<BaseC>::new(5),
        Coin::<Lpn>::new(7),
    );
}

#[test]
fn feed_price_with_alarm() {
    let mut test_case = create_test_case();
    let feeder = testing::user(ADMIN);
    oracle_mod::add_feeder(&mut test_case, feeder.clone());

    let lease = open_lease(&mut test_case, Coin::new(1000));

    () = test_case
        .app
        .execute(
            lease,
            test_case.address_book.oracle().clone(),
            &ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<PaymentC4>::new(1)).is(Coin::<Lpn>::new(10)),
                    None,
                ),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let _: AppResponse = oracle_mod::feed_price(
        &mut test_case,
        feeder,
        Coin::<PaymentC4>::new(1),
        Coin::<Lpn>::new(5),
    );
}

#[test]
fn overwrite_alarm_and_dispatch() {
    let mut test_case = create_test_case();
    update_tree(&mut test_case, swap_tree());

    let feeder = testing::user(ADMIN);
    oracle_mod::add_feeder(&mut test_case, feeder.clone());

    let lease = open_lease(&mut test_case, Coin::new(1000));

    () = test_case
        .app
        .execute(
            lease.clone(),
            test_case.address_book.oracle().clone(),
            &ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<PaymentC4>::new(1)).is(Coin::<Lpn>::new(5)),
                    Some(price::total_of(Coin::<PaymentC4>::new(1)).is(Coin::<Lpn>::new(5))),
                ),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    () = test_case
        .app
        .execute(
            lease.clone(),
            test_case.address_book.oracle().clone(),
            &ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<PaymentC4>::new(1)).is(Coin::<Lpn>::new(10)),
                    None,
                ),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    // If doesn't panic, then prices should be fed successfully.
    let _: AppResponse = oracle_mod::feed_price(
        &mut test_case,
        feeder,
        Coin::<PaymentC4>::new(1),
        Coin::<Lpn>::new(5),
    );

    let res: AppResponse = test_case
        .app
        .execute(
            lease.clone(),
            test_case.address_book.oracle().clone(),
            &ExecuteMsg::DispatchAlarms { max_count: 5 },
            &[],
        )
        .unwrap()
        .unwrap_response();

    platform::tests::assert_event(
        &res.events,
        &Event::new("wasm-pricealarm").add_attribute("receiver", lease.as_str()),
    );

    platform::tests::assert_event(
        &res.events,
        &Event::new("reply").add_attribute("mode", "handle_success"),
    );

    platform::tests::assert_event(
        &res.events,
        &Event::new("wasm-market-alarm").add_attribute("delivered", "success"),
    );
}

fn open_lease<ProtocolsRegistry, Treasury, Profit, Reserve, Lpp, Oracle, TimeAlarms>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: TheCoin,
) -> Addr {
    let customer = testing::user(ADMIN);
    let mut response: ResponseWithInterChainMsgs<'_, AppResponse> = test_case
        .app
        .execute(
            customer.clone(),
            test_case.address_book.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: currency::dto::<LeaseCurrency, _>(),
                max_ltd: None,
            },
            &[cw_coin(downpayment)],
        )
        .unwrap();

    response.expect_register_ica(TestCase::DEX_CONNECTION_ID, TestCase::LEASE_ICA_ID);

    () = response.ignore_response().unwrap_response();

    leaser_mod::expect_a_lease(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        customer,
    )
}

#[test]
#[should_panic]
fn wrong_timealarms_addr() {
    let mut test_case = create_test_case();

    let alarm_msg = timealarms::msg::ExecuteMsg::AddAlarm {
        time: Timestamp::from_seconds(100),
    };

    () = test_case
        .app
        .execute(
            testing::user(USER),
            test_case.address_book.oracle().clone(),
            &alarm_msg,
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();
}

#[test]
fn test_config_update() {
    let mut test_case = create_test_case();

    let _admin = testing::user(ADMIN);
    let feeder1 = testing::user("feeder1");
    let feeder2 = testing::user("feeder2");
    let feeder3 = testing::user("feeder3");
    let base = 2;
    let quote = 10;

    oracle_mod::add_feeder(&mut test_case, feeder1.clone());
    oracle_mod::add_feeder(&mut test_case, feeder2.clone());
    oracle_mod::add_feeder(&mut test_case, feeder3);

    oracle_mod::feed_price(
        &mut test_case,
        feeder1,
        Coin::<BaseC>::new(base),
        Coin::<Lpn>::new(quote),
    );
    oracle_mod::feed_price(
        &mut test_case,
        feeder2,
        Coin::<BaseC>::new(base),
        Coin::<Lpn>::new(quote),
    );

    let price: PriceDTO<PaymentGroup> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.oracle().clone(),
            &OracleQ::<PriceCurrencies>::BasePrice {
                currency: currency::dto::<BaseC, _>(),
            },
        )
        .unwrap();

    assert_eq!(
        price,
        price::total_of(Coin::<BaseC>::new(base))
            .is(Coin::<Lpn>::new(quote))
            .into()
    );

    let response: AppResponse = test_case
        .app
        .sudo(
            test_case.address_book.oracle().clone(),
            &SudoMsg::<PriceCurrencies>::UpdateConfig(PriceConfig::new(
                Percent::from_percent(100),
                Duration::from_secs(5),
                12,
                Percent::from_percent(75),
            )),
        )
        .expect("Oracle not properly connected!")
        .unwrap_response();
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_address", test_case.address_book.oracle())]
    );

    let price: Result<BasePrice<LeaseGroup, Lpn, Lpns>, _> =
        test_case.app.query().query_wasm_smart(
            test_case.address_book.oracle().clone(),
            &OracleQ::<PriceCurrencies>::BasePrice {
                currency: currency::dto::<BaseC, _>(),
            },
        );

    assert!(price.is_err());
}

#[test]
fn test_swap_path() {
    let mut test_case = create_test_case();

    update_tree(&mut test_case, swap_tree());

    let resp: SwapPath<PriceCurrencies> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.oracle().clone(),
            &OracleQ::<PriceCurrencies>::SwapPath {
                from: currency::dto::<PaymentC4, _>(),
                to: currency::dto::<PaymentC1, _>(),
            },
        )
        .unwrap();

    let expect = vec![
        SwapTarget {
            pool_id: 1,
            target: currency::dto::<Lpn, _>(),
        },
        SwapTarget {
            pool_id: 11,
            target: currency::dto::<PaymentC1, _>(),
        },
    ];

    assert_eq!(resp, expect);
}

#[test]
fn test_query_swap_tree() {
    let mut test_case = create_test_case();

    let tree: HumanReadableTree<SwapTarget<PaymentGroup>> = swap_tree();

    update_tree(&mut test_case, tree.clone());

    let resp: SwapTreeResponse<PaymentGroup> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.oracle().clone(),
            &OracleQ::<PriceCurrencies>::SwapTree {},
        )
        .unwrap();

    assert_eq!(resp.tree, tree);
}

#[test]
#[ignore = "impossible to setup invalid tree, instead a new code shoudl be used"]
fn migrate_invalid_swap_tree() {
    let mut test_case = create_test_case();
    update_tree(&mut test_case, invalid_swap_tree());
    let err = test_case
        .app
        .migrate(
            test_case.address_book.protocols_registry().clone(),
            test_case.address_book.oracle().clone(),
            &MigrateMsg {},
            5, // must be equal to the stored code
        )
        .unwrap_err();
    assert!(matches!(
        err.downcast_ref::<ContractError>(),
        Some(&ContractError::BrokenSwapTree(_))
    ));
}

#[test]
#[should_panic]
fn test_zero_price_dto() {
    let mut test_case = create_test_case();

    let feeder1 = testing::user("feeder1");

    oracle_mod::add_feeder(&mut test_case, feeder1.clone());

    // can be created only via deserialization
    let price: Price<LeaseC2, Lpn> =
        cosmwasm_std::from_json(r#"{"amount":{"amount":0},"amount_quote":{"amount":1}"#).unwrap();

    let response: AppResponse = oracle_mod::feed_price_pair(&mut test_case, feeder1, price);
    assert_eq!(response.data, None);
    let no_events: &[Event; 0] = &[];
    assert_eq!(&response.events, no_events);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DummyInstMsg {
    oracle: Addr,
    should_fail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DummyExecMsg {
    PriceAlarm(),
    ShouldFail(bool),
}

const ORACLE_ADDR: Item<Addr> = Item::new("oracle_addr");

const SHOULD_FAIL: Item<bool> = Item::new("should_fail");

fn schedule_alarm(
    storage: &dyn Storage,
    base: Amount,
    quote: Amount,
) -> ContractResult<CwResponse> {
    Batch::default()
        .schedule_execute_wasm_no_reply::<_, BaseC>(
            ORACLE_ADDR.load(storage).unwrap(),
            &ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of::<BaseC>(base.into()).is::<Lpn>(quote.into()),
                    None,
                ),
            },
            None,
        )
        .map_err(Into::into)
        .map(|batch| {
            platform::response::response_only_messages(platform::message::Response::messages_only(
                batch,
            ))
        })
}

fn execute<const RESCHEDULE: bool, const PRICE_BASE: Amount, const PRICE_QUOTE: Amount>(
    DepsMut { storage, .. }: DepsMut<'_>,
    _: Env,
    _: MessageInfo,
    msg: DummyExecMsg,
) -> Result<CwResponse, DummyContractError> {
    match msg {
        DummyExecMsg::PriceAlarm() => {
            if SHOULD_FAIL.load(storage).map_err(anyhow::Error::from)? {
                Err(DummyContractError(anyhow::anyhow!(
                    "Error while delivering price alarm!"
                )))
            } else if RESCHEDULE {
                schedule_alarm(storage, PRICE_BASE, PRICE_QUOTE)
                    .map_err(anyhow::Error::from)
                    .map_err(DummyContractError)
            } else {
                Ok(CwResponse::new())
            }
        }
        DummyExecMsg::ShouldFail(value) => SHOULD_FAIL
            .save(storage, &value)
            .map(|()| CwResponse::new())
            .map_err(anyhow::Error::from)
            .map_err(DummyContractError),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct DummyContractError(#[from] anyhow::Error);

type ExecFn =
    fn(DepsMut<'_>, Env, MessageInfo, DummyExecMsg) -> Result<CwResponse, DummyContractError>;

fn dummy_contract<const PRICE_BASE: Amount, const PRICE_QUOTE: Amount>(
    execute: ExecFn,
) -> Box<dyn CwContract<InterChainMsg>> {
    Box::new(CwContractWrapper::new(
        execute,
        |DepsMut { storage, .. },
         _: Env,
         _: MessageInfo,
         DummyInstMsg {
             oracle,
             should_fail,
         }: DummyInstMsg|
         -> Result<CwResponse, DummyContractError> {
            ORACLE_ADDR
                .save(storage, &oracle)
                .map_err(anyhow::Error::from)?;

            SHOULD_FAIL
                .save(storage, &should_fail)
                .map_err(anyhow::Error::from)?;

            schedule_alarm(storage, PRICE_BASE, PRICE_QUOTE)
                .map_err(anyhow::Error::from)
                .map_err(DummyContractError)
        },
        move |_: Deps<'_>, _: Env, (): ()| -> Result<Binary, DummyContractError> {
            unimplemented!()
        },
    ))
}

fn instantiate_dummy_contract(
    app: &mut App,
    dummy_code: Code,
    oracle: Addr,
    should_fail: bool,
) -> Addr {
    app.instantiate(
        dummy_code,
        testing::user(ADMIN),
        &DummyInstMsg {
            oracle,
            should_fail,
        },
        &[],
        "dummy_contract",
        None,
    )
    .unwrap()
    .unwrap_response()
}

fn dispatch_alarms(app: &mut App, oracle: Addr, max_count: AlarmsCount) -> AppResponse {
    app.execute(
        testing::user("unlisted_client"),
        oracle,
        &ExecuteMsg::DispatchAlarms { max_count },
        &[],
    )
    .unwrap()
    .unwrap_response()
}

fn set_should_fail(app: &mut App, dummy_contract: Addr, should_fail: bool) {
    () = app
        .execute(
            testing::user(ADMIN),
            dummy_contract,
            &DummyExecMsg::ShouldFail(should_fail),
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();
}

#[test]
fn price_alarm_rescheduling() {
    let mut test_case = create_test_case();
    update_tree(&mut test_case, swap_tree());

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 2, 1>));

    instantiate_dummy_contract(
        &mut test_case.app,
        dummy_code,
        test_case.address_book.oracle().clone(),
        false,
    );

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<true, 3, 1>));

    instantiate_dummy_contract(
        &mut test_case.app,
        dummy_code,
        test_case.address_book.oracle().clone(),
        false,
    );

    let feeder_addr = testing::user("feeder");

    oracle_mod::add_feeder(&mut test_case, feeder_addr.clone());

    oracle_mod::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(3),
        Coin::<Lpn>::new(1),
    );

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .all(|event| {
                event
                    .attributes
                    .contains(&Attribute::new("delivered", "success"))
            }),
        "{:?}",
        response.events
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .count(),
        2,
        "{:?}",
        response.events
    );

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert_eq!(
        response
            .events
            .iter()
            .find(|event| event.ty == "wasm-market-alarm"),
        None,
        "{:?}",
        response.events
    );

    oracle_mod::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(4),
        Coin::<Lpn>::new(1),
    );

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .all(|event| {
                event
                    .attributes
                    .contains(&Attribute::new("delivered", "success"))
            }),
        "{:?}",
        response.events
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .count(),
        1,
        "{:?}",
        response.events
    );
}

#[test]
fn price_alarm_rescheduling_with_failing() {
    let mut test_case = create_test_case();
    update_tree(&mut test_case, swap_tree());

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 2, 1>));

    instantiate_dummy_contract(
        &mut test_case.app,
        dummy_code,
        test_case.address_book.oracle().clone(),
        false,
    );

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 3, 1>));

    let dummy_failing = instantiate_dummy_contract(
        &mut test_case.app,
        dummy_code,
        test_case.address_book.oracle().clone(),
        true,
    );

    let feeder_addr = testing::user("feeder");

    oracle_mod::add_feeder(&mut test_case, feeder_addr.clone());

    oracle_mod::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(3),
        Coin::<Lpn>::new(1),
    );

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .filter(|event| {
                event
                    .attributes
                    .contains(&Attribute::new("delivered", "success"))
            })
            .count(),
        1,
        "{:?}",
        response.events
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .filter(|event| {
                event
                    .attributes
                    .contains(&Attribute::new("delivered", "error"))
            })
            .count(),
        1,
        "{:?}",
        response.events
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .count(),
        2,
        "{:?}",
        response.events
    );

    set_should_fail(&mut test_case.app, dummy_failing, false);

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .filter(|event| {
                event
                    .attributes
                    .contains(&Attribute::new("delivered", "success"))
            })
            .count(),
        1,
        "{:?}",
        response.events
    );

    assert_eq!(
        response
            .events
            .iter()
            .filter(|event| event.ty == "wasm-market-alarm")
            .count(),
        1,
        "{:?}",
        response.events
    );

    let response = dispatch_alarms(
        &mut test_case.app,
        test_case.address_book.oracle().clone(),
        5,
    );

    assert_eq!(
        response
            .events
            .iter()
            .find(|event| event.ty == "wasm-market-alarm"),
        None,
        "{:?}",
        response.events
    );
}

fn invalid_swap_tree() -> HumanReadableTree<SwapTarget<PriceCurrencies>> {
    swap_tree_impl::<PaymentC7>()
}

fn swap_tree() -> HumanReadableTree<SwapTarget<PriceCurrencies>> {
    swap_tree_impl::<PaymentC1>()
}

fn swap_tree_impl<LpnDescendantC>() -> HumanReadableTree<SwapTarget<PriceCurrencies>>
where
    LpnDescendantC: CurrencyDef,
{
    cosmwasm_std::from_json(format!(
        r#"{{
                "value":[0,"{lpn}"],
                "children":[
                    {{
                        "value":[1,"{base_c}"],
                        "children":[
                            {{"value":[2,"{p5}"]}},
                            {{"value":[3,"{p6}"]}}
                        ]
                    }},
                    {{
                        "value":[11,"{p1}"]
                    }}
                ]
            }}"#,
        lpn = Lpn::definition().dto(),
        base_c = BaseC::definition().dto(),
        p5 = PaymentC5::definition().dto(),
        p6 = PaymentC6::definition().dto(),
        p1 = LpnDescendantC::definition().dto(),
    ))
    .unwrap()
}

fn update_tree(
    test_case: &mut TestCase<Addr, Addr, Addr, Addr, Addr, Addr, Addr, Addr>,
    new_tree: HumanReadableTree<SwapTarget<PaymentGroup>>,
) {
    let response: AppResponse = test_case
        .app
        .sudo(
            test_case.address_book.oracle().clone(),
            &SudoMsg::SwapTree { tree: new_tree },
        )
        .unwrap()
        .unwrap_response();
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_address", test_case.address_book.oracle())]
    );
}
