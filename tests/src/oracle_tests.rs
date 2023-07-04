use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json_wasm::from_str;

use currency::{
    lease::{Atom, Cro, Osmo, Wbtc, Weth},
    lpn::Usdc,
    Currency,
};
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    percent::Percent,
    price::{self, dto::PriceDTO},
};
use leaser::msg::QueryMsg;
use marketprice::{config::Config as PriceConfig, SpotPrice};
use oracle::{
    alarms::Alarm,
    msg::{AlarmsCount, QueryMsg as OracleQ},
    result::ContractResult,
    ContractError,
};
use platform::{batch::Batch, coin_legacy};
use sdk::{
    cosmwasm_ext::{CustomMsg, Response as CwResponse},
    cosmwasm_std::{
        coin, wasm_execute, Addr, Attribute, Binary, Coin as CwCoin, Deps, DepsMut, Env, Event,
        MessageInfo, StdError as CwError, Storage, Timestamp,
    },
    cw_multi_test::{AppResponse, Contract as CwContract, Executor},
    cw_storage_plus::Item,
    testing::ContractWrapper,
};
use swap::SwapTarget;
use tree::HumanReadableTree;

use crate::common::{
    oracle_wrapper,
    test_case::{Builder as TestCaseBuilder, TestCase},
    MockApp, ADDON_OPTIMAL_INTEREST_RATE, ADMIN, BASE_INTEREST_RATE, USER, UTILIZATION_OPTIMAL,
};

type Lpn = Usdc;
type LeaseCurrency = Atom;
type TheCoin = Coin<Lpn>;
type BaseC = Osmo;

fn cw_coin<CoinT>(coin: CoinT) -> CwCoin
where
    CoinT: Into<Coin<Lpn>>,
{
    coin_legacy::to_cosmwasm(coin.into())
}

fn create_test_case() -> TestCase {
    TestCaseBuilder::<Lpn>::with_reserve(&[cw_coin(10_000_000_000_000_000_000_000_000_000)])
        .init_lpp_with_funds(
            None,
            &[coin(
                5_000_000_000_000_000_000_000_000_000,
                Lpn::BANK_SYMBOL,
            )],
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
        )
        .init_time_alarms()
        .init_oracle(None)
        .init_treasury()
        .init_profit(24)
        .init_leaser()
        .into_generic()
}

#[test]
fn test_lease_serde() {
    use lease::api::ExecuteMsg::PriceAlarm as LeasePriceAlarm;
    use oracle::msg::ExecuteAlarmMsg::PriceAlarm;

    let LeasePriceAlarm {} = serde_json_wasm::from_slice(&serde_json_wasm::to_vec(&PriceAlarm {}).unwrap()).unwrap() else {
        unreachable!()
    };

    let PriceAlarm {} =
        serde_json_wasm::from_slice(&serde_json_wasm::to_vec(&LeasePriceAlarm {}).unwrap())
            .unwrap();
}

#[test]
fn register_feeder() {
    let mut test_case = create_test_case();
    let _user = Addr::unchecked(USER);
    let _admin = Addr::unchecked(ADMIN);

    oracle_wrapper::add_feeder(&mut test_case, ADMIN);
}

#[test]
fn internal_test_integration_setup_test() {
    let mut test_case = create_test_case();

    oracle_wrapper::add_feeder(&mut test_case, ADMIN);

    let response: AppResponse = oracle_wrapper::feed_price::<BaseC, Usdc>(
        &mut test_case,
        Addr::unchecked(ADMIN),
        Coin::new(5),
        Coin::new(7),
    );
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("execute").add_attribute("_contract_addr", "contract2")]
    );
}

// test for issue #26. It was resolved in MR !132 by separation of price feeding and alarms delivery processes
#[test]
fn feed_price_with_alarm_issue() {
    let mut test_case = create_test_case();
    oracle_wrapper::add_feeder(&mut test_case, ADMIN);

    let lease = open_lease(&mut test_case, Coin::new(1000));

    let oracle = test_case.oracle().clone();

    // there is no price in the oracle and feed for this alarm
    test_case
        .app
        .execute_contract(
            lease,
            oracle,
            &oracle::msg::ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<Cro>::new(1)).is(Coin::<Usdc>::new(1)),
                    None,
                ),
            },
            &[],
        )
        .unwrap();

    test_case.message_receiver.assert_empty();

    let _ = oracle_wrapper::feed_price::<BaseC, Usdc>(
        &mut test_case,
        Addr::unchecked(ADMIN),
        Coin::new(5),
        Coin::new(7),
    );
}

#[test]
fn feed_price_with_alarm() {
    let mut test_case = create_test_case();
    oracle_wrapper::add_feeder(&mut test_case, ADMIN);

    let lease = open_lease(&mut test_case, Coin::new(1000));

    let oracle = test_case.oracle().clone();

    test_case
        .app
        .execute_contract(
            lease,
            oracle,
            &oracle::msg::ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<Cro>::new(1)).is(Coin::<Usdc>::new(10)),
                    None,
                ),
            },
            &[],
        )
        .unwrap();

    test_case.message_receiver.assert_empty();

    let res = oracle_wrapper::feed_price::<Cro, Usdc>(
        &mut test_case,
        Addr::unchecked(ADMIN),
        Coin::new(1),
        Coin::new(5),
    );

    dbg!(res);
}

#[test]
fn overwrite_alarm_and_dispatch() {
    let mut test_case = create_test_case();
    oracle_wrapper::add_feeder(&mut test_case, ADMIN);

    let lease = open_lease(&mut test_case, Coin::new(1000));
    let oracle = test_case.oracle().clone();

    test_case
        .app
        .execute_contract(
            lease.clone(),
            oracle.clone(),
            &oracle::msg::ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<Cro>::new(1)).is(Coin::<Usdc>::new(5)),
                    Some(price::total_of(Coin::<Cro>::new(1)).is(Coin::<Usdc>::new(5))),
                ),
            },
            &[],
        )
        .unwrap();

    test_case
        .app
        .execute_contract(
            lease.clone(),
            oracle.clone(),
            &oracle::msg::ExecuteMsg::AddPriceAlarm {
                alarm: Alarm::new(
                    price::total_of(Coin::<Cro>::new(1)).is(Coin::<Usdc>::new(10)),
                    None,
                ),
            },
            &[],
        )
        .unwrap();

    test_case.message_receiver.assert_empty();

    // If doesn't panic, then prices should be fed successfully.
    let _: AppResponse = oracle_wrapper::feed_price::<Cro, Usdc>(
        &mut test_case,
        Addr::unchecked(ADMIN),
        Coin::new(1),
        Coin::new(5),
    );

    let res: AppResponse = test_case
        .app
        .execute_contract(
            lease,
            oracle,
            &oracle::msg::ExecuteMsg::DispatchAlarms { max_count: 5 },
            &[],
        )
        .unwrap();

    platform::tests::assert_event(
        &res.events,
        &Event::new("wasm-pricealarm").add_attribute("receiver", "contract6"),
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

fn open_lease(test_case: &mut TestCase, downpayment: TheCoin) -> Addr {
    test_case
        .app
        .execute_contract(
            Addr::unchecked(ADMIN),
            test_case.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: LeaseCurrency::TICKER.into(),
                max_ltd: None,
            },
            &[cw_coin(downpayment)],
        )
        .unwrap();

    test_case
        .message_receiver
        .assert_register_ica(TestCase::LEASER_CONNECTION_ID);

    test_case.message_receiver.assert_empty();

    get_lease_address(test_case)
}

fn get_lease_address(test_case: &TestCase) -> Addr {
    let query_response: HashSet<Addr> = test_case
        .app
        .wrap()
        .query_wasm_smart(
            test_case.leaser().clone(),
            &QueryMsg::Leases {
                owner: Addr::unchecked(ADMIN),
            },
        )
        .unwrap();
    assert_eq!(query_response.len(), 1);
    query_response.iter().next().unwrap().clone()
}

#[test]
#[should_panic]
fn wrong_timealarms_addr() {
    let mut test_case = create_test_case();

    let alarm_msg = timealarms::msg::ExecuteMsg::AddAlarm {
        time: Timestamp::from_seconds(100),
    };

    let oracle = test_case.oracle().clone();

    test_case
        .app
        .execute_contract(Addr::unchecked(USER), oracle, &alarm_msg, &[])
        .unwrap();
}

#[test]
fn test_config_update() {
    let mut test_case = create_test_case();

    let oracle = test_case.oracle().clone();

    let _admin = Addr::unchecked(ADMIN);
    let feeder1 = Addr::unchecked("feeder1");
    let feeder2 = Addr::unchecked("feeder2");
    let feeder3 = Addr::unchecked("feeder3");
    let base = 2;
    let quote = 10;

    oracle_wrapper::add_feeder(&mut test_case, &feeder1);
    oracle_wrapper::add_feeder(&mut test_case, &feeder2);
    oracle_wrapper::add_feeder(&mut test_case, &feeder3);

    test_case.message_receiver.assert_empty();

    oracle_wrapper::feed_price::<BaseC, Usdc>(
        &mut test_case,
        feeder1,
        Coin::new(base),
        Coin::new(quote),
    );
    oracle_wrapper::feed_price::<BaseC, Usdc>(
        &mut test_case,
        feeder2,
        Coin::new(base),
        Coin::new(quote),
    );

    let price: SpotPrice = test_case
        .app
        .wrap()
        .query_wasm_smart(
            test_case.oracle().clone(),
            &OracleQ::Price {
                currency: BaseC::TICKER.into(),
            },
        )
        .unwrap();

    assert_eq!(
        price,
        PriceDTO::try_from(price::total_of(Coin::<BaseC>::new(base)).is(Coin::<Usdc>::new(quote)))
            .unwrap()
    );

    let response: AppResponse = test_case
        .app
        .wasm_sudo(
            oracle.clone(),
            &oracle::msg::SudoMsg::UpdateConfig(PriceConfig::new(
                Percent::from_percent(100),
                Duration::from_secs(5),
                12,
                Percent::from_percent(75),
            )),
        )
        .expect("Oracle not properly connected!");
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_addr", "contract2")]
    );

    let price: Result<SpotPrice, _> = test_case.app.wrap().query_wasm_smart(
        oracle,
        &OracleQ::Price {
            currency: BaseC::TICKER.into(),
        },
    );

    assert!(price.is_err());
}

fn swap_tree() -> HumanReadableTree<SwapTarget> {
    from_str(&format!(
        r#"{{
                "value":[0,"{usdc}"],
                "children":[
                    {{
                        "value":[1,"{base_c}"],
                        "children":[
                            {{"value":[2,"{weth}"]}},
                            {{"value":[3,"{wbtc}"]}}
                        ]
                    }}
                ]
            }}"#,
        usdc = Usdc::TICKER,
        base_c = BaseC::TICKER,
        weth = Weth::TICKER,
        wbtc = Wbtc::TICKER,
    ))
    .unwrap()
}

#[test]
fn test_swap_path() {
    let mut test_case = create_test_case();

    let oracle = test_case.oracle().clone();

    let response: AppResponse = test_case
        .app
        .wasm_sudo(
            oracle.clone(),
            &oracle::msg::SudoMsg::SwapTree { tree: swap_tree() },
        )
        .unwrap();
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_addr", "contract2")]
    );

    let resp: swap::SwapPath = test_case
        .app
        .wrap()
        .query_wasm_smart(
            oracle,
            &OracleQ::SwapPath {
                from: Wbtc::TICKER.into(),
                to: Weth::TICKER.into(),
            },
        )
        .unwrap();

    let expect = vec![
        SwapTarget {
            pool_id: 3,
            target: BaseC::TICKER.into(),
        },
        SwapTarget {
            pool_id: 2,
            target: Weth::TICKER.into(),
        },
    ];

    assert_eq!(resp, expect);
}

#[test]
fn test_query_swap_tree() {
    let mut test_case = create_test_case();

    let oracle = test_case.oracle().clone();

    let tree: HumanReadableTree<SwapTarget> = swap_tree();

    let response: AppResponse = test_case
        .app
        .wasm_sudo(
            oracle.clone(),
            &oracle::msg::SudoMsg::SwapTree { tree: tree.clone() },
        )
        .unwrap();
    assert_eq!(response.data, None);
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_addr", "contract2")]
    );

    let resp: oracle::msg::SwapTreeResponse = test_case
        .app
        .wrap()
        .query_wasm_smart(oracle, &OracleQ::SwapTree {})
        .unwrap();

    assert_eq!(resp.tree, tree);
}

#[test]
#[should_panic]
fn test_zero_price_dto() {
    let mut test_case = create_test_case();

    let oracle = test_case.oracle().clone();

    let feeder1 = Addr::unchecked("feeder1");

    oracle_wrapper::add_feeder(&mut test_case, &feeder1);

    // can be created only via deserialization
    let price: SpotPrice = from_str(
        r#"{"amount":{"amount":0,"ticker":"OSMO"},"amount_quote":{"amount":1,"ticker":"USDC"}}"#,
    )
    .unwrap();

    let response: AppResponse = test_case
        .app
        .execute(
            feeder1,
            wasm_execute(
                oracle,
                &oracle::msg::ExecuteMsg::FeedPrices {
                    prices: vec![price],
                },
                vec![],
            )
            .unwrap()
            .into(),
        )
        .unwrap();
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

const ORACLE_ADDR: Item<'static, Addr> = Item::new("oracle_addr");

const SHOULD_FAIL: Item<'static, bool> = Item::new("should_fail");

fn schedule_alarm(
    storage: &dyn Storage,
    base: Amount,
    quote: Amount,
) -> ContractResult<CwResponse> {
    Ok(platform::response::response_only_messages(
        platform::message::Response::messages_only({
            let mut batch: Batch = Batch::default();

            batch.schedule_execute_wasm_no_reply::<_, BaseC>(
                &ORACLE_ADDR.load(storage)?,
                oracle::msg::ExecuteMsg::AddPriceAlarm {
                    alarm: Alarm::new(
                        price::total_of::<BaseC>(base.into()).is::<Usdc>(quote.into()),
                        None,
                    ),
                },
                None,
            )?;

            batch
        }),
    ))
}

fn execute<const RESCHEDULE: bool, const PRICE_BASE: Amount, const PRICE_QUOTE: Amount>(
    DepsMut { storage, .. }: DepsMut<'_>,
    _: Env,
    _: MessageInfo,
    msg: DummyExecMsg,
) -> ContractResult<CwResponse> {
    match msg {
        DummyExecMsg::PriceAlarm() => {
            if SHOULD_FAIL.load(storage)? {
                Err(ContractError::Std(CwError::generic_err(
                    "Error while delivering price alarm!",
                )))
            } else if RESCHEDULE {
                schedule_alarm(storage, PRICE_BASE, PRICE_QUOTE)
            } else {
                Ok(CwResponse::new())
            }
        }
        DummyExecMsg::ShouldFail(value) => SHOULD_FAIL
            .save(storage, &value)
            .map(|()| CwResponse::new())
            .map_err(Into::into),
    }
}

type ExecFn = fn(DepsMut<'_>, Env, MessageInfo, DummyExecMsg) -> ContractResult<CwResponse>;

fn dummy_contract<const PRICE_BASE: Amount, const PRICE_QUOTE: Amount>(
    execute: ExecFn,
) -> Box<dyn CwContract<CustomMsg>> {
    Box::new(ContractWrapper::new(
        execute,
        |DepsMut { storage, .. },
         _: Env,
         _: MessageInfo,
         DummyInstMsg {
             oracle,
             should_fail,
         }: DummyInstMsg|
         -> ContractResult<CwResponse> {
            ORACLE_ADDR.save(storage, &oracle)?;

            SHOULD_FAIL.save(storage, &should_fail)?;

            schedule_alarm(storage, PRICE_BASE, PRICE_QUOTE)
        },
        move |_: Deps<'_>, _: Env, (): ()| -> ContractResult<Binary> { unimplemented!() },
    ))
}

fn instantiate_dummy_contract(
    app: &mut MockApp,
    dummy_code: u64,
    oracle: Addr,
    should_fail: bool,
) -> Addr {
    app.instantiate_contract(
        dummy_code,
        Addr::unchecked(ADMIN),
        &DummyInstMsg {
            oracle,
            should_fail,
        },
        &[],
        "dummy_contract",
        None,
    )
    .unwrap()
}

fn dispatch_alarms(app: &mut MockApp, oracle: Addr, max_count: AlarmsCount) -> AppResponse {
    app.execute_contract(
        Addr::unchecked("unlisted_client"),
        oracle,
        &oracle::msg::ExecuteMsg::DispatchAlarms { max_count },
        &[],
    )
    .unwrap()
}

fn set_should_fail(app: &mut MockApp, dummy_contract: Addr, should_fail: bool) {
    app.execute_contract(
        Addr::unchecked(ADMIN),
        dummy_contract,
        &DummyExecMsg::ShouldFail(should_fail),
        &[],
    )
    .unwrap();
}

#[test]
fn price_alarm_rescheduling() {
    let mut test_case = create_test_case();

    let oracle = test_case.oracle().clone();

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 2, 1>));

    instantiate_dummy_contract(&mut test_case.app, dummy_code, oracle.clone(), false);

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<true, 3, 1>));

    instantiate_dummy_contract(&mut test_case.app, dummy_code, oracle.clone(), false);

    let feeder_addr = Addr::unchecked("feeder");

    oracle_wrapper::add_feeder(&mut test_case, feeder_addr.as_str());

    oracle_wrapper::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(3),
        Coin::<Usdc>::new(1),
    );

    let response = dispatch_alarms(&mut test_case.app, oracle.clone(), 5);

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

    let response = dispatch_alarms(&mut test_case.app, oracle.clone(), 5);

    assert_eq!(
        response
            .events
            .iter()
            .find(|event| event.ty == "wasm-market-alarm"),
        None,
        "{:?}",
        response.events
    );

    oracle_wrapper::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(4),
        Coin::<Usdc>::new(1),
    );

    let response = dispatch_alarms(&mut test_case.app, oracle, 5);

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

    let oracle = test_case.oracle().clone();

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 2, 1>));

    instantiate_dummy_contract(&mut test_case.app, dummy_code, oracle.clone(), false);

    let dummy_code = test_case
        .app
        .store_code(dummy_contract::<2, 1>(execute::<false, 3, 1>));

    let dummy_failing =
        instantiate_dummy_contract(&mut test_case.app, dummy_code, oracle.clone(), true);

    let feeder_addr = Addr::unchecked("feeder");

    oracle_wrapper::add_feeder(&mut test_case, feeder_addr.as_str());

    oracle_wrapper::feed_price(
        &mut test_case,
        feeder_addr.clone(),
        Coin::<BaseC>::new(3),
        Coin::<Usdc>::new(1),
    );

    let response = dispatch_alarms(&mut test_case.app, oracle.clone(), 5);

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

    let response = dispatch_alarms(&mut test_case.app, oracle.clone(), 5);

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

    let response = dispatch_alarms(&mut test_case.app, oracle, 5);

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
