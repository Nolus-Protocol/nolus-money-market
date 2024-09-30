use std::slice;

use ::lease::api::{query::StateResponse, ExecuteMsg};
use ::swap::testing::SwapRequest;
use currencies::PaymentGroup;
use currency::CurrencyDef;
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    fraction::Fraction,
    percent::Percent,
    price::{self, Price},
    ratio::Rational,
    zero::Zero,
};
use platform::coin_legacy::to_cosmwasm_on_dex;
use sdk::{
    cosmwasm_std::{Addr, Timestamp},
    cw_multi_test::AppResponse,
};

use crate::common::{
    self, cwcoin, ibc,
    leaser::Instantiator as LeaserInstantiator,
    swap::{self, DexDenom},
    test_case::{app::App, response::ResponseWithInterChainMsgs, TestCase},
    CwCoin, USER,
};

use super::{
    price_lpn_of, LeaseCoin, LeaseCurrency, LeaseTestCase, LpnCoin, LpnCurrency, PaymentCoin,
    PaymentCurrency, DOWNPAYMENT,
};

#[test]
fn partial_repay() {
    let mut test_case: LeaseTestCase = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;

    let amount = super::quote_borrow(&test_case, downpayment);
    let partial_payment: PaymentCoin = Fraction::<PaymentCoin>::of(
        &Rational::new(1, 2),
        super::create_payment_coin(amount.into()),
    )
    .unwrap();
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, partial_payment);

    let lease_addr: Addr = super::open_lease(&mut test_case, downpayment, None);

    repay(&mut test_case, lease_addr.clone(), partial_payment);

    let query_result = super::state_query(&test_case, lease_addr.as_str());

    assert_eq!(query_result, expected_result);
}

#[test]
fn partial_repay_after_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment: PaymentCoin = DOWNPAYMENT;

    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    test_case.app.time_shift(Duration::from_nanos(
        LeaserInstantiator::REPAYMENT_PERIOD.nanos() >> 1,
    ));

    let query_result = super::state_query(&test_case, lease_address.as_ref());

    let StateResponse::Opened {
        overdue_margin,
        overdue_interest,
        due_margin,
        ..
    } = query_result
    else {
        unreachable!()
    };

    super::feed_price(&mut test_case);

    let due_margin_to_pay: LpnCoin = LpnCoin::try_from(due_margin)
        .unwrap()
        .checked_div(2)
        .unwrap();

    repay(
        &mut test_case,
        lease_address.clone(),
        price::total(
            LpnCoin::try_from(overdue_margin).unwrap()
                + LpnCoin::try_from(overdue_interest).unwrap()
                + due_margin_to_pay,
            super::price_lpn_of::<PaymentCurrency>().inv(),
        )
        .unwrap(),
    );

    let query_result = super::state_query(&test_case, lease_address.as_str());

    if let StateResponse::Opened {
        overdue_margin,
        overdue_interest,
        ..
    } = query_result
    {
        assert!(
            overdue_margin.is_zero(),
            "Expected 0 for margin interest due, got {}",
            overdue_margin.amount()
        );

        assert!(
            overdue_interest.is_zero(),
            "Expected 0 for interest due, got {}",
            overdue_interest.amount()
        );
    } else {
        unreachable!()
    }
}

#[test]
#[should_panic = "[Lease] Insufficient payment amount"]
fn insufficient_payment() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;

    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    let payment: PaymentCoin = super::create_payment_coin(49);
    repay(&mut test_case, lease_address, payment);
}

#[test]
fn full_repay() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment: PaymentCoin = DOWNPAYMENT;
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let borrowed_lpn = super::quote_borrow(&test_case, downpayment);
    let borrowed: PaymentCoin = price::total(borrowed_lpn, super::price_lpn_of().inv()).unwrap();

    repay(&mut test_case, lease_address.clone(), borrowed);

    let expected_amount: LeaseCoin = price::total(
        price::total(
            downpayment + borrowed,
            /* Payment -> LPN */ super::price_lpn_of(),
        )
        .unwrap(),
        /* LPN -> Lease */ super::price_lpn_of().inv(),
    )
    .unwrap();
    let expected_result = StateResponse::Paid {
        amount: LeaseCoin::into(expected_amount),
        in_progress: None,
    };
    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(query_result, expected_result);
}

#[test]
fn full_repay_with_max_ltd() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;
    let percent = Percent::from_percent(10);
    let borrowed = percent.of(DOWNPAYMENT).unwrap();
    let lease_address = super::open_lease(&mut test_case, downpayment, Some(percent));

    let lease_amount = (Percent::HUNDRED + percent)
        .of(price::total(
            downpayment,
            Price::<PaymentCurrency, LeaseCurrency>::identity(),
        )
        .unwrap())
        .unwrap();
    let expected_result = StateResponse::Opened {
        amount: lease_amount.into(),
        loan_interest_rate: Percent::from_permille(70),
        margin_interest_rate: Percent::from_permille(30),
        principal_due: price::total(percent.of(downpayment).unwrap(), super::price_lpn_of())
            .unwrap()
            .into(),
        overdue_margin: LpnCoin::ZERO.into(),
        overdue_interest: LpnCoin::ZERO.into(),
        overdue_collect_in: LeaserInstantiator::REPAYMENT_PERIOD,
        due_margin: LpnCoin::ZERO.into(),
        due_interest: LpnCoin::ZERO.into(),
        validity: Timestamp::from_nanos(1537237459879305533),
        in_progress: None,
    };
    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(query_result, expected_result);

    repay(&mut test_case, lease_address.clone(), borrowed);

    let expected_amount: LeaseCoin = price::total(
        price::total(
            downpayment + borrowed,
            /* Payment -> LPN */ super::price_lpn_of(),
        )
        .unwrap(),
        /* LPN -> Lease */ super::price_lpn_of().inv(),
    )
    .unwrap();
    let expected_result = StateResponse::Paid {
        amount: LeaseCoin::into(expected_amount),
        in_progress: None,
    };
    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(query_result, expected_result);
}

#[test]
fn full_repay_with_excess() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment: PaymentCoin = DOWNPAYMENT;
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let borrowed: PaymentCoin = price::total(
        super::quote_borrow(&test_case, downpayment),
        super::price_lpn_of().inv(),
    )
    .unwrap();

    let overpayment = super::create_payment_coin(5);
    let payment: PaymentCoin = borrowed + overpayment;

    repay(&mut test_case, lease_address.clone(), payment);

    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(
        test_case
            .app
            .query()
            .query_all_balances(lease_address.clone())
            .unwrap(),
        &[cwcoin::<LpnCurrency, Amount>(overpayment.into())],
    );

    assert_eq!(
        test_case
            .app
            .query()
            .query_all_balances(TestCase::ica_addr(
                lease_address.as_str(),
                TestCase::LEASE_ICA_ID
            ))
            .unwrap(),
        &[to_cosmwasm_on_dex(
            price::total(
                price::total(downpayment + borrowed, price_lpn_of()).unwrap(),
                price_lpn_of::<LeaseCurrency>().inv()
            )
            .unwrap()
        )],
    );

    assert_eq!(
        query_result,
        StateResponse::Paid {
            amount: LeaseCoin::into(
                price::total(
                    price::total(downpayment + borrowed, price_lpn_of()).unwrap(),
                    price_lpn_of().inv(),
                )
                .unwrap()
            ),
            in_progress: None,
        }
    );
}

pub(crate) fn repay_with_hook_on_swap<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Leaser,
    Lpp,
    Oracle,
    SwapHook,
>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Oracle,
        Addr,
    >,
    lease_addr: Addr,
    payment: PaymentCoin,
    swap_hook: SwapHook,
) -> AppResponse
where
    SwapHook: FnOnce(&mut App),
{
    let mut response: ResponseWithInterChainMsgs<'_, ()> =
        send_payment_and_transfer(test_case, lease_addr.clone(), payment);

    let requests: Vec<SwapRequest<PaymentGroup, PaymentGroup>> = swap::expect_swap(
        &mut response,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    () = response.unwrap_response();

    swap_hook(&mut test_case.app);

    let swap_out_lpn: LpnCoin = price::total(payment, price_lpn_of()).unwrap();

    let ica_addr: Addr = TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID);

    let mut response: ResponseWithInterChainMsgs<'_, ()> = swap::do_swap(
        &mut test_case.app,
        lease_addr.clone(),
        ica_addr.clone(),
        requests.into_iter(),
        |amount: Amount, in_denom: DexDenom<'_>, out_denom: DexDenom<'_>| {
            assert_eq!(amount, payment.into());
            assert_eq!(in_denom, PaymentCurrency::dex());
            assert_eq!(out_denom, LpnCurrency::dex());

            swap_out_lpn.into()
        },
    )
    .ignore_response();

    let transfer_amount: CwCoin = ibc::expect_remote_transfer(
        &mut response,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    assert_eq!(transfer_amount, to_cosmwasm_on_dex(swap_out_lpn));

    () = response.unwrap_response();

    ibc::do_transfer(
        &mut test_case.app,
        ica_addr,
        lease_addr,
        true,
        &transfer_amount,
    )
    .unwrap_response()
}

pub(crate) fn repay<ProtocolsRegistry, Treasury, Profit, Reserve, Leaser, Lpp, Oracle>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Oracle,
        Addr,
    >,
    lease_addr: Addr,
    payment: PaymentCoin,
) -> AppResponse {
    repay_with_hook_on_swap(test_case, lease_addr, payment, |_app| {})
}

fn send_payment_and_transfer<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Leaser,
    Lpp,
    Oracle,
    PaymentC,
>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Oracle,
        Addr,
    >,
    lease_addr: Addr,
    payment: Coin<PaymentC>,
) -> ResponseWithInterChainMsgs<'_, ()>
where
    PaymentC: CurrencyDef,
{
    let payment_cw: CwCoin = common::cwcoin(payment);
    let mut response: ResponseWithInterChainMsgs<'_, ()> = test_case
        .app
        .execute(
            Addr::unchecked(USER),
            lease_addr.clone(),
            &ExecuteMsg::Repay {},
            slice::from_ref(&payment_cw),
        )
        .unwrap()
        .ignore_response();

    let ica_addr: Addr = TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID);

    let transfer_amount: CwCoin = ibc::expect_transfer(
        &mut response,
        TestCase::LEASER_IBC_CHANNEL,
        lease_addr.as_str(),
        ica_addr.as_str(),
    );

    assert_eq!(transfer_amount, payment_cw);

    () = response.unwrap_response();

    ibc::do_transfer(
        &mut test_case.app,
        lease_addr,
        ica_addr,
        false,
        &transfer_amount,
    )
    .ignore_response()
}
