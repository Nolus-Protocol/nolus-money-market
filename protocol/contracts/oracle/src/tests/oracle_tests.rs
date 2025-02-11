use currencies::{
    testing::{PaymentC1, PaymentC3, PaymentC4, PaymentC5, PaymentC8},
    LeaseGroup, Lpn, Lpns, PaymentGroup as PriceCurrencies,
};
use finance::{
    coin::Coin,
    price::{self, base::BasePrice, dto::PriceDTO},
};
use platform::{contract::testing, tests};
use sdk::{
    cosmwasm_ext::Response as CwResponse,
    cosmwasm_std::{
        self,
        testing::{self as cw_testing, MockQuerier},
        Addr, Event, MessageInfo,
    },
};

use crate::{
    api::{Alarm, AlarmsCount, DispatchAlarmsResponse, ExecuteMsg, QueryMsg},
    contract, error,
    error::Error,
    tests::{dummy_default_instantiate_msg, setup_test},
};

use super::dummy_feed_prices_msg;

#[test]
fn feed_prices_unknown_feeder() {
    let (mut deps, _) = setup_test(dummy_default_instantiate_msg());

    let msg = dummy_feed_prices_msg();

    let info = MessageInfo {
        sender: Addr::unchecked("test"),
        funds: vec![],
    };

    let err = contract::execute(deps.as_mut(), cw_testing::mock_env(), info, msg).unwrap_err();
    assert_eq!(Error::UnknownFeeder {}, err)
}

#[test]
fn feed_direct_price() {
    fn generate_price() -> PriceDTO<PriceCurrencies> {
        price::total_of(Coin::<PaymentC1>::new(10))
            .is(Coin::<Lpn>::new(120))
            .into()
    }
    let (mut deps, info) = setup_test(dummy_default_instantiate_msg());

    // Feed direct price PaymentC1/OracleBaseAsset
    let msg = ExecuteMsg::FeedPrices {
        prices: vec![generate_price()],
    };
    let _res = contract::execute(deps.as_mut(), cw_testing::mock_env(), info, msg).unwrap();

    // query price for PaymentC3
    let res = contract::query(
        deps.as_ref(),
        cw_testing::mock_env(),
        QueryMsg::BasePrice {
            currency: currency::dto::<PaymentC1, PriceCurrencies>().into_super_group(),
        },
    )
    .unwrap();
    let value: PriceDTO<PriceCurrencies> = cosmwasm_std::from_json(res).unwrap();
    assert_eq!(generate_price(), value);
}

#[test]
fn feed_indirect_price() {
    let (mut deps, info) = setup_test(dummy_default_instantiate_msg());

    let price_a_to_b =
        PriceDTO::from(price::total_of(Coin::<PaymentC3>::new(10)).is(Coin::<PaymentC5>::new(120)));
    let price_b_to_c =
        PriceDTO::from(price::total_of(Coin::<PaymentC5>::new(10)).is(Coin::<PaymentC4>::new(5)));
    let price_c_to_usdc =
        PriceDTO::from(price::total_of(Coin::<PaymentC4>::new(10)).is(Coin::<Lpn>::new(5)));

    // Feed indirect price from PaymentC3 to OracleBaseAsset
    let msg = ExecuteMsg::FeedPrices {
        prices: vec![price_a_to_b, price_b_to_c, price_c_to_usdc],
    };
    let _res = contract::execute(deps.as_mut(), cw_testing::mock_env(), info, msg).unwrap();

    // query price for PaymentC3
    let res = contract::query(
        deps.as_ref(),
        cw_testing::mock_env(),
        QueryMsg::BasePrice {
            currency: currency::dto::<PaymentC3, PriceCurrencies>().into_super_group(),
        },
    )
    .unwrap();

    let expected_price = BasePrice::<LeaseGroup, _, Lpns>::from(
        price::total_of(Coin::<PaymentC3>::new(1)).is(Coin::<Lpn>::new(3)),
    );
    let value: BasePrice<LeaseGroup, _, _> = cosmwasm_std::from_json(res).unwrap();
    assert_eq!(expected_price, value)
}

#[test]
fn query_prices_unsupported_denom() {
    let (deps, _) = setup_test(dummy_default_instantiate_msg());

    let detached = currency::dto::<PaymentC8, PriceCurrencies>().into_super_group();
    assert_eq!(
        error::unsupported_currency::<_, Lpn>(detached),
        contract::query(
            deps.as_ref(),
            cw_testing::mock_env(),
            QueryMsg::BasePrice { currency: detached },
        )
        .unwrap_err()
    );
}

#[test]
fn feed_prices_unsupported_pairs() {
    let (mut deps, info) = setup_test(dummy_default_instantiate_msg());

    let unsupported =
        PriceDTO::from(price::total_of(Coin::<PaymentC3>::new(10)).is(Coin::<PaymentC4>::new(12)));
    let prices = vec![
        unsupported,
        PriceDTO::from(price::total_of(Coin::<PaymentC5>::new(10)).is(Coin::<PaymentC4>::new(22))),
    ];

    let msg = ExecuteMsg::FeedPrices { prices };
    let err = contract::execute(deps.as_mut(), cw_testing::mock_env(), info, msg).unwrap_err();
    assert_eq!(error::unsupported_denom_pairs(&unsupported), err);
}

#[test]
fn deliver_alarm() {
    let (mut deps, info) = setup_test(dummy_default_instantiate_msg());
    setup_receiver(&mut deps.querier);

    let current_price = price::total_of(Coin::<PaymentC4>::new(10)).is(Coin::<Lpn>::new(23451));
    let feed_price_msg = ExecuteMsg::FeedPrices {
        prices: vec![current_price.into()],
    };
    let feed_resp = contract::execute(
        deps.as_mut(),
        cw_testing::mock_env(),
        info.clone(),
        feed_price_msg,
    );
    assert_eq!(Ok(CwResponse::default()), feed_resp);

    {
        let alarm_below_price =
            price::total_of(Coin::<PaymentC4>::new(10)).is(Coin::<Lpn>::new(23450));
        let add_alarm_msg = ExecuteMsg::AddPriceAlarm {
            alarm: Alarm::new(alarm_below_price, None),
        };
        let add_alarm_resp = contract::execute(
            deps.as_mut(),
            cw_testing::mock_env(),
            info.clone(),
            add_alarm_msg,
        );
        assert_eq!(Ok(CwResponse::default()), add_alarm_resp);

        let dispatch_alarms_msg = ExecuteMsg::DispatchAlarms { max_count: 10 };
        let dispatch_alarms_resp = contract::execute(
            deps.as_mut(),
            cw_testing::mock_env(),
            info.clone(),
            dispatch_alarms_msg,
        )
        .unwrap();
        assert!(!any_error(&dispatch_alarms_resp));
        assert_eq!(sent_alarms(&dispatch_alarms_resp), Some(0));
        assert_eq!(0, dispatch_alarms_resp.messages.len());
    }
    {
        let alarm_below_price =
            price::total_of(Coin::<PaymentC4>::new(10)).is(Coin::<Lpn>::new(23452));
        let add_alarm_msg = ExecuteMsg::AddPriceAlarm {
            alarm: Alarm::new(alarm_below_price, None),
        };
        let add_alarm_resp = contract::execute(
            deps.as_mut(),
            cw_testing::mock_env(),
            info.clone(),
            add_alarm_msg,
        );
        assert_eq!(Ok(CwResponse::default()), add_alarm_resp);

        let dispatch_alarms_msg = ExecuteMsg::DispatchAlarms { max_count: 10 };
        let receiver = info.sender.clone();
        let dispatch_alarms_resp = contract::execute(
            deps.as_mut(),
            cw_testing::mock_env(),
            info,
            dispatch_alarms_msg,
        )
        .unwrap();
        assert!(!any_error(&dispatch_alarms_resp));
        tests::assert_event(
            &dispatch_alarms_resp.events,
            &Event::new("pricealarm").add_attribute("receiver", receiver),
        );
        assert_eq!(sent_alarms(&dispatch_alarms_resp), Some(1));
        assert_eq!(1, dispatch_alarms_resp.messages.len());
    }
}

fn setup_receiver(querier: &mut MockQuerier) {
    querier.update_wasm(testing::valid_contract_handler);
}

fn sent_alarms(resp: &CwResponse) -> Option<AlarmsCount> {
    tests::parse_resp::<DispatchAlarmsResponse>(&resp.data).map(|resp| resp.0)
}

fn any_error(resp: &CwResponse) -> bool {
    tests::any_error(&resp.events)
}
