use std::collections::HashMap;

use currencies::PaymentGroup;
use finance::{coin::Amount, duration::Duration, price};
use lease::api::{query::StateResponse, ExecuteMsg};
use platform::coin_legacy::to_cosmwasm_on_dex;
use sdk::{cosmwasm_std::Addr, cw_multi_test::AppResponse};
use swap::testing::SwapRequest;

use crate::common::{
    ibc,
    leaser::Instantiator as LeaserInstantiator,
    test_case::{response::ResponseWithInterChainMsgs, TestCase},
    CwCoin,
};

use super::{
    super::{create_test_case, feed_price, open_lease, price_lpn_of, state_query, LeaseTestCase},
    LeaseCoin, PaymentCoin, PaymentCurrency,
};

fn liquidation_time_alarm(
    downpayment: PaymentCoin,
    time_pass: Duration,
    liquidation_amount: Option<LeaseCoin>,
) {
    let mut test_case: LeaseTestCase = create_test_case::<PaymentCurrency>();
    let lease_addr: Addr = open_lease(&mut test_case, downpayment, None);

    let StateResponse::Opened {
        amount: lease_amount,
        ..
    }: StateResponse = state_query(&test_case, lease_addr.as_ref())
    else {
        unreachable!()
    };
    let lease_amount: LeaseCoin = lease_amount.try_into().unwrap();

    test_case.app.time_shift(time_pass);

    feed_price(&mut test_case);

    let mut response: ResponseWithInterChainMsgs<'_, ()> = test_case
        .app
        .execute(
            test_case.address_book.time_alarms().clone(),
            lease_addr.clone(),
            &ExecuteMsg::TimeAlarm {},
            &[],
        )
        .unwrap()
        .ignore_response();

    let Some(liquidation_amount): Option<LeaseCoin> = liquidation_amount else {
        () = response.unwrap_response();

        return;
    };

    let requests: Vec<SwapRequest<PaymentGroup, PaymentGroup>> = crate::common::swap::expect_swap(
        &mut response,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    () = response.unwrap_response();

    let ica_addr: Addr = TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID);

    let mut response: ResponseWithInterChainMsgs<'_, ()> = crate::common::swap::do_swap(
        &mut test_case.app,
        lease_addr.clone(),
        ica_addr.clone(),
        requests.into_iter(),
        |amount: u128, _, _| amount,
    )
    .ignore_response();

    let transfer_amount: CwCoin = ibc::expect_remote_transfer(
        &mut response,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    () = response.unwrap_response();

    assert_eq!(
        transfer_amount,
        to_cosmwasm_on_dex(price::total(liquidation_amount, price_lpn_of()).unwrap())
    );

    let response: ResponseWithInterChainMsgs<'_, AppResponse> = ibc::do_transfer(
        &mut test_case.app,
        ica_addr,
        lease_addr.clone(),
        true,
        &transfer_amount,
    );

    let liquidation_end_response: AppResponse = response.unwrap_response();

    assert_eq!(
        test_case
            .app
            .query()
            .query_all_balances(lease_addr.clone())
            .unwrap(),
        &[],
    );

    let liquidation_attributes: HashMap<String, String> = liquidation_end_response
        .events
        .into_iter()
        .find(|event| event.ty == "wasm-ls-liquidation")
        .expect("No liquidation emitted!")
        .attributes
        .into_iter()
        .map(|attribute| (attribute.key, attribute.value))
        .collect();

    let query_result: StateResponse = state_query(&test_case, lease_addr.as_str());

    let liquidated_amount: LeaseCoin = liquidation_attributes["amount-amount"]
        .parse::<Amount>()
        .unwrap()
        .into();

    assert_eq!(liquidated_amount, liquidation_amount);

    if let StateResponse::Opened {
        amount,
        due_interest,
        due_margin,
        overdue_interest,
        overdue_margin,
        ..
    } = query_result
    {
        assert_eq!(
            LeaseCoin::try_from(amount).unwrap(),
            lease_amount - liquidated_amount
        );
        assert!(due_interest.is_zero());
        assert!(due_margin.is_zero());

        assert!(overdue_interest.is_zero());
        assert!(overdue_margin.is_zero());
    }
}

#[test]
fn liquidation_by_time_due_more_than_min_no_overdue() {
    liquidation_time_alarm(
        1_000_000.into(),
        LeaserInstantiator::REPAYMENT_PERIOD - Duration::from_nanos(1),
        None,
    );
}

#[test]
fn liquidation_by_time_overdue_less_than_min() {
    liquidation_time_alarm(
        100.into(),
        LeaserInstantiator::REPAYMENT_PERIOD + Duration::from_days(1),
        None,
    );
}

#[test]
fn liquidation_by_time_overdue_more_than_min() {
    liquidation_time_alarm(
        1_000_000_000.into(),
        LeaserInstantiator::REPAYMENT_PERIOD,
        Some(LeaseCoin::new(45792562)), //the total interest due for the LeaserInstantiator::REPAYMENT_PERIOD = (7% + 3%) * 65/(100-65)*Downpayment * LeaserInstantiator::REPAYMENT_PERIOD/365
    );
}
