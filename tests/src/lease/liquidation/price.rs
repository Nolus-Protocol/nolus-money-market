use currencies::{Lpns, PaymentGroup};
use currency::CurrencyDef as _;
use finance::{coin::Amount, percent::Percent};
use lease::api::{query::StateResponse, ExecuteMsg};
use platform::coin_legacy::to_cosmwasm_on_dex;
use sdk::{
    cosmwasm_std::{Addr, Event},
    cw_multi_test::AppResponse,
};
use swap::testing::SwapRequest;

use crate::{
    common::{
        self, cwcoin, ibc,
        leaser::{self, Instantiator as LeaserInstantiator},
        test_case::{response::ResponseWithInterChainMsgs, TestCase},
        CwCoin, ADMIN, USER,
    },
    lease::{self as lease_mod, LeaseTestCase, LpnCurrency},
};

use super::{LeaseCoin, LeaseCurrency, LpnCoin, PaymentCurrency, DOWNPAYMENT};

#[test]
#[should_panic = "No liquidation warning emitted!"]
fn liquidation_warning_price_0() {
    liquidation_warning(
        2085713.into(),
        1857159.into(),
        LeaserInstantiator::liability().max(), //not used
        "N/A",
    );
}

#[test]
fn liquidation_warning_price_1() {
    liquidation_warning(
        // ref: 2085713
        2085713.into(),
        // ref: 1857159
        1827159.into(),
        LeaserInstantiator::FIRST_LIQ_WARN,
        "1",
    );
}

#[test]
fn liquidation_warning_price_2() {
    liquidation_warning(
        // ref: 2085713
        2085713.into(),
        // ref: 1857159
        1757159.into(),
        LeaserInstantiator::SECOND_LIQ_WARN,
        "2",
    );
}

#[test]
fn liquidation_warning_price_3() {
    liquidation_warning(
        // ref: 2085713
        2085713.into(),
        // ref: 1857159
        1707159.into(),
        LeaserInstantiator::THIRD_LIQ_WARN,
        "3",
    );
}

#[test]
fn full_liquidation() {
    let mut test_case = lease_mod::create_test_case::<PaymentCurrency>();

    let lease_addr: Addr = lease_mod::open_lease(&mut test_case, DOWNPAYMENT, None);

    let reserve: Addr = test_case.address_book.reserve().clone();

    let ica_addr: Addr = TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID);

    let lease_amount: Amount = 2857142857142;
    let borrowed_amount: Amount = 1857142857142;
    let liq_outcome = borrowed_amount - 11123; // to trigger an interaction with Reserve
    test_case.send_funds_from_admin(
        reserve.clone(),
        &[cwcoin::<LpnCurrency, _>(borrowed_amount - liq_outcome)],
    );

    // the base is chosen to be close to the asset amount to trigger a full liquidation
    let mut response: ResponseWithInterChainMsgs<'_, ()> = deliver_new_price(
        &mut test_case,
        lease_addr.clone(),
        (lease_amount - 2).into(),
        borrowed_amount.into(),
    )
    .ignore_response();

    let requests: Vec<SwapRequest<PaymentGroup, PaymentGroup>> = common::swap::expect_swap(
        &mut response,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    () = response.unwrap_response();

    let mut response: ResponseWithInterChainMsgs<'_, ()> = common::swap::do_swap(
        &mut test_case.app,
        lease_addr.clone(),
        ica_addr.clone(),
        requests.into_iter(),
        |amount: u128, _, _| {
            assert_eq!(amount, lease_amount);

            liq_outcome
        },
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
        to_cosmwasm_on_dex(LpnCoin::new(liq_outcome))
    );

    let response: AppResponse = ibc::do_transfer(
        &mut test_case.app,
        ica_addr,
        lease_addr.clone(),
        true,
        &transfer_amount,
    )
    .unwrap_response();

    response.assert_event(
        &Event::new("wasm-ls-liquidation")
            .add_attribute("payment-amount", borrowed_amount.to_string())
            .add_attribute("loan-close", "true"),
    );
    assert!(
        platform::bank::balance::<LpnCurrency, Lpns>(&reserve, test_case.app.query())
            .unwrap()
            .is_zero()
    );

    assert_eq!(
        test_case
            .app
            .query()
            .query_all_balances(lease_addr.clone())
            .unwrap(),
        &[],
    );

    let state = lease_mod::state_query(&test_case, lease_addr.as_str());
    assert!(
        matches!(state, StateResponse::Liquidated()),
        "should have been in Liquidated state"
    );
    leaser::assert_no_leases(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        Addr::unchecked(USER),
    )
}

fn liquidation_warning(base: LeaseCoin, quote: LpnCoin, liability: Percent, level: &str) {
    let mut test_case = lease_mod::create_test_case::<PaymentCurrency>();
    let lease = lease_mod::open_lease(&mut test_case, DOWNPAYMENT, None);

    let response: AppResponse =
        deliver_new_price(&mut test_case, lease, base, quote).unwrap_response();

    let event = response
        .events
        .iter()
        .find(|event| event.ty == "wasm-ls-liquidation-warning")
        .expect("No liquidation warning emitted!");

    let attribute = event
        .attributes
        .iter()
        .find(|attribute| attribute.key == "customer")
        .expect("Customer attribute not present!");

    assert_eq!(attribute.value, USER);

    let attribute = event
        .attributes
        .iter()
        .find(|attribute| attribute.key == "ltv")
        .expect("LTV attribute not present!");

    assert_eq!(attribute.value, liability.units().to_string());

    let attribute = event
        .attributes
        .iter()
        .find(|attribute| attribute.key == "level")
        .expect("Level attribute not present!");

    assert_eq!(attribute.value, level);

    let attribute = event
        .attributes
        .iter()
        .find(|attribute| attribute.key == "lease-asset")
        .expect("Lease Asset attribute not present!");

    assert_eq!(&attribute.value, LeaseCurrency::ticker());
}

fn deliver_new_price(
    test_case: &mut LeaseTestCase,
    lease: Addr,
    base: LeaseCoin,
    quote: LpnCoin,
) -> ResponseWithInterChainMsgs<'_, AppResponse> {
    common::oracle::feed_price(test_case, Addr::unchecked(ADMIN), base, quote);

    test_case
        .app
        .execute(
            test_case.address_book.oracle().clone(),
            lease,
            &ExecuteMsg::PriceAlarm(),
            &[],
        )
        .unwrap()
}
