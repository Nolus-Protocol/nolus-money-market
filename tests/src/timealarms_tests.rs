use currency::{lpn::Usdc, native::Nls};
use finance::{coin::Coin, currency::Currency, duration::Duration};
use sdk::{
    cosmwasm_std::{coin, from_binary, Addr, Attribute, Event, Timestamp},
    cw_multi_test::{AppResponse, Executor},
};
use timealarms::msg::DispatchAlarmsResponse;

use crate::{
    common::{cwcoin, test_case::TestCase, AppExt, ADMIN},
    timealarms_tests::mock_lease::{
        instantiate_may_fail_contract, instantiate_reschedule_contract,
    },
};

/// The mock for lease SC. It mimics the scheme for time notification.
/// If GATE, it returns Ok on notifications, returns Err otherwise.
mod mock_lease {
    use serde::{Deserialize, Serialize};

    use finance::duration::Duration;
    use sdk::{
        cosmwasm_ext::Response,
        cosmwasm_std::{
            to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, StdError, StdResult,
        },
        cw_storage_plus::Item,
        schemars::{self, JsonSchema},
        testing::{Contract, ContractWrapper, Executor},
    };
    use timealarms::stub::TimeAlarmsRef;

    use crate::common::{MockApp, ADMIN};

    const GATE: Item<'static, bool> = Item::new("alarm gate");
    const TIMEALARMS_ADDR: Item<'static, Addr> = Item::new("ta_addr");

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub struct MockInstantiateMsg {
        time_alarms_contract: Addr,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum MockExecuteMsg {
        // mimic the scheme
        TimeAlarm {},
        // setup GATE
        Gate(bool),
    }

    fn instantiate(
        deps: DepsMut<'_>,
        _: Env,
        _: MessageInfo,
        msg: MockInstantiateMsg,
    ) -> StdResult<Response> {
        GATE.save(deps.storage, &true)?;
        TIMEALARMS_ADDR.save(deps.storage, &msg.time_alarms_contract)?;
        Ok(Response::new().add_attribute("method", "instantiate"))
    }

    fn execute(
        deps: DepsMut<'_>,
        env: Env,
        _: MessageInfo,
        msg: MockExecuteMsg,
    ) -> StdResult<Response> {
        match msg {
            MockExecuteMsg::TimeAlarm {} => {
                let gate = GATE.load(deps.storage).expect("storage problem");

                if gate {
                    Ok(Response::new()
                        .add_attribute("lease_reply", env.block.time.to_string())
                        .set_data(to_binary(&env.contract.address)?))
                } else {
                    Err(StdError::generic_err("closed gate"))
                }
            }
            MockExecuteMsg::Gate(gate) => {
                GATE.update(deps.storage, |_| -> StdResult<bool> { Ok(gate) })?;

                Ok(Response::new().add_attribute("method", "set_gate"))
            }
        }
    }

    fn execute_reschedule_alarm(
        deps: DepsMut<'_>,
        env: Env,
        _: MessageInfo,
        msg: MockExecuteMsg,
    ) -> StdResult<Response> {
        match msg {
            MockExecuteMsg::TimeAlarm {} => {
                let timealarms = TIMEALARMS_ADDR
                    .load(deps.storage)
                    .expect("test setup error");
                TimeAlarmsRef::unchecked(timealarms)
                    .setup_alarm(env.block.time + Duration::from_secs(5))
                    .unwrap();

                Ok(Response::new()
                    .add_attribute("lease_reply", env.block.time.to_string())
                    .set_data(to_binary(&env.contract.address)?))
            }
            MockExecuteMsg::Gate(_gate) => {
                unimplemented!()
            }
        }
    }

    fn query(_: Deps<'_>, _: Env, _msg: MockExecuteMsg) -> StdResult<Binary> {
        Err(StdError::generic_err("not implemented"))
    }

    fn contract_may_fail_endpoints() -> Box<Contract> {
        let contract = ContractWrapper::new(execute, instantiate, query);
        Box::new(contract)
    }

    fn contract_reschedule_endpoints() -> Box<Contract> {
        let contract = ContractWrapper::new(execute_reschedule_alarm, instantiate, query);
        Box::new(contract)
    }

    pub fn instantiate_may_fail_contract(app: &mut MockApp) -> Addr {
        proper_instantiate(
            app,
            contract_may_fail_endpoints(),
            Addr::unchecked("unused"),
        )
    }

    pub fn instantiate_reschedule_contract(app: &mut MockApp, timealarms_contract: Addr) -> Addr {
        proper_instantiate(app, contract_reschedule_endpoints(), timealarms_contract)
    }

    fn proper_instantiate(
        app: &mut MockApp,
        endpoints: Box<Contract>,
        timealarms_contract: Addr,
    ) -> Addr {
        let cw_template_id = app.store_code(endpoints);
        app.instantiate_contract(
            cw_template_id,
            Addr::unchecked(ADMIN),
            &MockInstantiateMsg {
                time_alarms_contract: timealarms_contract,
            },
            &[],
            "test",
            None,
        )
        .unwrap()
    }
}

type Lpn = Usdc;

#[test]
fn test_lease_serde() {
    use lease::api::ExecuteMsg::TimeAlarm as LeaseTimeAlarm;
    use timealarms::msg::ExecuteAlarmMsg::TimeAlarm;

    let LeaseTimeAlarm {} = serde_json_wasm::from_slice(&serde_json_wasm::to_vec(&TimeAlarm {}).unwrap()).unwrap() else {
        unreachable!()
    };

    let TimeAlarm {} =
        serde_json_wasm::from_slice(&serde_json_wasm::to_vec(&LeaseTimeAlarm {}).unwrap()).unwrap();
}

fn test_case() -> TestCase<Lpn> {
    let mut test_case = TestCase::<Lpn>::with_reserve(
        None,
        &[coin(
            10_000_000_000_000_000_000_000_000_000,
            Lpn::BANK_SYMBOL,
        )],
    );
    test_case.init(
        &Addr::unchecked(ADMIN),
        vec![coin(1_000_000_000_000_000_000_000_000, Lpn::BANK_SYMBOL)],
    );

    test_case.init_timealarms();

    test_case
        .app
        .update_block(|bl| bl.time = Timestamp::from_nanos(0));

    test_case
}

fn add_alarm(test_case: &mut TestCase<Lpn>, recv: &Addr, time_secs: u64) {
    let alarm_msg = timealarms::msg::ExecuteMsg::AddAlarm {
        time: Timestamp::from_seconds(time_secs),
    };
    let timealarms = test_case.timealarms.clone().unwrap();
    test_case
        .app
        .execute_contract(recv.clone(), timealarms, &alarm_msg, &[])
        .unwrap();
}

fn dispatch(test_case: &mut TestCase<Lpn>, max_count: u32) -> AppResponse {
    let dispatch_msg = timealarms::msg::ExecuteMsg::DispatchAlarms { max_count };
    test_case
        .app
        .execute_contract(
            Addr::unchecked(ADMIN),
            test_case.timealarms.clone().unwrap(),
            &dispatch_msg,
            &[],
        )
        .unwrap()
}

fn any_error(resp: &AppResponse) -> bool {
    let maybe_attr = resp
        .events
        .iter()
        .flat_map(|ev| &ev.attributes)
        .find(|atr| atr.key == "delivered");

    matches!(maybe_attr.map(|attr| attr.value.as_str()), Some("error"))
}

fn sent_alarms(resp: &AppResponse) -> Option<u32> {
    resp.data
        .as_ref()
        .map(|data| from_binary::<DispatchAlarmsResponse>(data).unwrap().0)
}

#[test]
fn fired_alarms_are_removed() {
    let mut test_case = test_case();
    let lease1 = instantiate_may_fail_contract(&mut test_case.app);
    let lease2 = instantiate_may_fail_contract(&mut test_case.app);

    add_alarm(&mut test_case, &lease1, 1);
    //overwritten
    add_alarm(&mut test_case, &lease1, 2);
    add_alarm(&mut test_case, &lease2, 3);

    // advance by 5 seconds
    test_case.app.time_shift(Duration::from_secs(5));

    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(2));

    // try to resend same alarms
    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(0));
}

#[test]
#[ignore = "bug: reply removes rescheduled alarm"]
fn reschedule_alarm() {
    let mut test_case = test_case();
    let lease1 =
        instantiate_reschedule_contract(&mut test_case.app, test_case.timealarms.clone().unwrap());

    add_alarm(&mut test_case, &lease1, 1);

    test_case.app.time_shift(Duration::from_secs(5));

    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(1));

    // try to resend the newly scheduled alarms
    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(1));
}

#[test]
fn test_time_notify() {
    let mut test_case = test_case();

    // instantiate lease, add alarms
    let lease1 = instantiate_may_fail_contract(&mut test_case.app);
    let lease2 = instantiate_may_fail_contract(&mut test_case.app);
    let lease3 = instantiate_may_fail_contract(&mut test_case.app);
    let lease4 = instantiate_may_fail_contract(&mut test_case.app);

    add_alarm(&mut test_case, &lease1, 1);
    add_alarm(&mut test_case, &lease2, 2);

    add_alarm(&mut test_case, &lease3, 6);
    add_alarm(&mut test_case, &lease4, 7);

    // advance by 5 seconds
    test_case.app.time_shift(Duration::from_secs(5));

    let resp = dispatch(&mut test_case, 100);

    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(2));

    let resp = dispatch(&mut test_case, 100);
    assert_eq!(sent_alarms(&resp), Some(0));

    test_case.app.time_shift(Duration::from_secs(5));

    // close the GATE, lease return error on notification
    let close_gate = mock_lease::MockExecuteMsg::Gate(false);
    test_case
        .app
        .execute_contract(Addr::unchecked(ADMIN), lease3.clone(), &close_gate, &[])
        .unwrap();
    let resp = dispatch(&mut test_case, 100);
    dbg!(&resp);
    assert!(any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(2));
    resp.assert_event(&Event::new("wasm-timealarm").add_attribute("receiver", lease3.clone()));
    resp.assert_event(&Event::new("wasm-timealarm").add_attribute("receiver", lease4.clone()));

    // open the GATE, check for remaining alarm
    let open_gate = mock_lease::MockExecuteMsg::Gate(true);
    test_case
        .app
        .execute_contract(Addr::unchecked(ADMIN), lease3.clone(), &open_gate, &[])
        .unwrap();

    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(1));
    resp.assert_event(&Event::new("wasm-timealarm").add_attribute("receiver", lease3.clone()));

    // check if something is left
    let resp = dispatch(&mut test_case, 100);
    assert!(!any_error(&resp));
    assert_eq!(sent_alarms(&resp), Some(0));
}

#[test]
fn test_profit_alarms() {
    let admin = Addr::unchecked(ADMIN);
    let mut test_case = TestCase::<Lpn>::with_reserve(
        None,
        &[
            cwcoin(Coin::<Lpn>::new(1_000_000)),
            cwcoin(Coin::<Nls>::new(1_000_000)),
        ],
    );
    test_case.init(
        &admin,
        vec![
            cwcoin(Coin::<Lpn>::new(100_000)),
            cwcoin(Coin::<Nls>::new(100_000)),
        ],
    );
    test_case.init_timealarms();
    test_case.init_treasury();
    test_case.init_profit(1);

    test_case
        .app
        .send_tokens(
            admin.clone(),
            test_case.profit_addr.clone().unwrap(),
            &[cwcoin(Coin::<Nls>::new(100_000))],
        )
        .unwrap();

    test_case.app.time_shift(Duration::from_hours(10));

    let dispatch_msg = timealarms::msg::ExecuteMsg::DispatchAlarms { max_count: 1 };

    let resp = test_case
        .app
        .execute_contract(
            Addr::unchecked(ADMIN),
            test_case.timealarms.clone().unwrap(),
            &dispatch_msg,
            &[],
        )
        .unwrap();

    assert_eq!(
        resp.events.last().unwrap().attributes.last().unwrap(),
        Attribute::new("delivered", "success")
    );
}
