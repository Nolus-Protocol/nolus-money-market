use osmosis_std::types::osmosis::gamm::v1beta1::{
    MsgSwapExactAmountIn, MsgSwapExactAmountInResponse,
};

use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    fraction::Fraction,
    percent::Percent,
    price::{self, Price},
    zero::Zero,
};
use lease::api::{ExecuteMsg, StateResponse};
use sdk::{
    cosmos_sdk_proto::{ibc::applications::transfer::v1::MsgTransfer, traits::TypeUrl as _},
    cosmwasm_std::{Addr, Binary, Coin as CwCoin, Timestamp},
    cw_multi_test::AppResponse,
};

use crate::{
    common::{
        cwcoin,
        leaser::Instantiator as LeaserInstantiator,
        test_case::{
            app::Wasm as WasmTrait,
            response::{RemoteChain as _, ResponseWithInterChainMsgs},
            TestCase,
        },
        ADMIN, USER,
    },
    lease as lease_mod,
};

use super::{LeaseCoin, LeaseCurrency, Lpn, LpnCoin, PaymentCoin, PaymentCurrency, DOWNPAYMENT};

#[test]
fn partial_repay() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = super::create_payment_coin(DOWNPAYMENT);

    let quote_result = super::quote_query(&test_case, downpayment);
    let amount: LpnCoin = quote_result.borrow.try_into().unwrap();
    let partial_payment = super::create_payment_coin(u128::from(amount) / 2);
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, partial_payment);

    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    repay(&mut test_case, lease_address.clone(), partial_payment);

    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(query_result, expected_result);
}

#[test]
fn partial_repay_after_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment: PaymentCoin = super::create_payment_coin(DOWNPAYMENT);

    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    test_case.app.time_shift(Duration::from_nanos(
        LeaserInstantiator::REPAYMENT_PERIOD.nanos() >> 1,
    ));

    let query_result = super::state_query(&test_case, lease_address.as_ref());

    let StateResponse::Opened {
        previous_margin_due,
        previous_interest_due,
        current_margin_due,
        ..
    } = query_result else {
        unreachable!()
    };

    super::feed_price(&mut test_case);

    let current_margin_to_pay: LpnCoin = LpnCoin::try_from(current_margin_due)
        .unwrap()
        .checked_div(2)
        .unwrap();

    repay(
        &mut test_case,
        lease_address.clone(),
        price::total(
            LpnCoin::try_from(previous_margin_due).unwrap()
                + LpnCoin::try_from(previous_interest_due).unwrap()
                + current_margin_to_pay,
            super::price_lpn_of::<PaymentCurrency>().inv(),
        ),
    );

    let query_result = super::state_query(&test_case, lease_address.as_str());

    if let StateResponse::Opened {
        previous_margin_due,
        previous_interest_due,
        ..
    } = query_result
    {
        assert!(
            previous_margin_due.is_zero(),
            "Expected 0 for margin interest due, got {}",
            previous_margin_due.amount()
        );

        assert!(
            previous_interest_due.is_zero(),
            "Expected 0 for interest due, got {}",
            previous_interest_due.amount()
        );
    } else {
        unreachable!()
    }
}

#[test]
fn full_repay() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment: PaymentCoin = super::create_payment_coin(DOWNPAYMENT);
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let borrowed: PaymentCoin = price::total(
        super::quote_borrow(&test_case, downpayment),
        super::price_lpn_of().inv(),
    );

    repay(&mut test_case, lease_address.clone(), borrowed);

    let expected_amount: LeaseCoin = price::total(
        price::total(
            downpayment + borrowed,
            /* Payment -> LPN */ super::price_lpn_of(),
        ),
        /* LPN -> Lease */ super::price_lpn_of().inv(),
    );
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
    let downpayment = super::create_payment_coin(DOWNPAYMENT);
    let percent = Percent::from_percent(10);
    let borrowed = Coin::new(percent.of(DOWNPAYMENT));
    let lease_address = super::open_lease(&mut test_case, downpayment, Some(percent));

    let expected_result = StateResponse::Opened {
        amount: (Percent::HUNDRED + percent)
            .of(price::total(
                downpayment,
                Price::<PaymentCurrency, LeaseCurrency>::identity(),
            ))
            .into(),
        loan_interest_rate: Percent::from_permille(70),
        margin_interest_rate: Percent::from_permille(30),
        principal_due: price::total(percent.of(downpayment), super::price_lpn_of()).into(),
        previous_margin_due: LpnCoin::ZERO.into(),
        previous_interest_due: LpnCoin::ZERO.into(),
        current_margin_due: LpnCoin::ZERO.into(),
        current_interest_due: LpnCoin::ZERO.into(),
        validity: Timestamp::from_nanos(1537237454879305533),
        in_progress: None,
    };
    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(query_result, expected_result);

    repay(&mut test_case, lease_address.clone(), borrowed);

    let expected_amount: LeaseCoin = price::total(
        price::total(
            downpayment + borrowed,
            /* Payment -> LPN */ super::price_lpn_of(),
        ),
        /* LPN -> Lease */ super::price_lpn_of().inv(),
    );
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
    let downpayment: PaymentCoin = super::create_payment_coin(DOWNPAYMENT);
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let borrowed: PaymentCoin = price::total(
        super::quote_borrow(&test_case, downpayment),
        /* LPN -> Payment */ super::price_lpn_of().inv(),
    );

    let overpayment = super::create_payment_coin(5);
    let payment: PaymentCoin = borrowed + overpayment;

    repay(&mut test_case, lease_address.clone(), payment);

    let query_result = super::state_query(&test_case, lease_address.as_str());

    assert_eq!(
        test_case
            .app
            .query()
            .query_all_balances(lease_address)
            .unwrap(),
        &[cwcoin::<Lpn, Amount>(overpayment.into())],
    );

    assert_eq!(
        test_case.app.query().query_all_balances("ica0").unwrap(),
        &[cwcoin::<LeaseCurrency, _>(price::total(
            price::total(downpayment + borrowed, lease_mod::price_lpn_of()),
            lease_mod::price_lpn_of().inv(),
        ))],
    );

    assert_eq!(
        query_result,
        StateResponse::Paid {
            amount: LeaseCoin::into(price::total(
                price::total(downpayment + borrowed, lease_mod::price_lpn_of()),
                lease_mod::price_lpn_of().inv(),
            )),
            in_progress: None,
        }
    );
}

pub(crate) fn repay<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle>(
    test_case: &mut TestCase<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle, Addr>,
    contract_addr: Addr,
    payment: PaymentCoin,
) -> AppResponse
where
    Wasm: WasmTrait,
{
    let cw_payment: CwCoin = cwcoin(payment);

    let response: ResponseWithInterChainMsgs<'_, ()> =
        send_payment_and_transfer(test_case, contract_addr.clone(), cw_payment.clone());

    expect_swap(response);

    let swap_out_lpn: LpnCoin = price::total(payment, super::price_lpn_of());

    let response: ResponseWithInterChainMsgs<'_, ()> =
        do_swap(test_case, contract_addr.clone(), &cw_payment, swap_out_lpn);

    expect_remote_ibc_transfer(response);

    do_remote_ibc_transfer(test_case, contract_addr, &cwcoin(swap_out_lpn))
}

fn send_payment_and_transfer<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle>(
    test_case: &mut TestCase<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle, Addr>,
    contract_addr: Addr,
    cw_payment: CwCoin,
) -> ResponseWithInterChainMsgs<'_, ()>
where
    Wasm: WasmTrait,
{
    let mut response: ResponseWithInterChainMsgs<'_, ()> = test_case
        .app
        .execute(
            Addr::unchecked(USER),
            contract_addr.clone(),
            &ExecuteMsg::Repay {},
            std::slice::from_ref(&cw_payment),
        )
        .unwrap()
        .ignore_response();

    response.expect_ibc_transfer(
        "channel-0",
        cw_payment.clone(),
        contract_addr.as_str(),
        "ica0",
    );

    () = response.unwrap_response();

    test_case
        .app
        .send_tokens(
            contract_addr.clone(),
            Addr::unchecked("ica0"),
            &[cw_payment],
        )
        .unwrap();

    test_case
        .app
        .sudo(contract_addr, &super::construct_response(Binary::default()))
        .unwrap()
        .ignore_response()
}

fn expect_swap(mut response: ResponseWithInterChainMsgs<'_, ()>) {
    response.expect_submit_tx(
        TestCase::LEASER_CONNECTION_ID,
        "0",
        &[MsgSwapExactAmountIn::TYPE_URL],
    );

    response.unwrap_response()
}

fn do_swap<'r, Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle>(
    test_case: &'r mut TestCase<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle, Addr>,
    contract_addr: Addr,
    cw_payment: &CwCoin,
    swap_out_lpn: LpnCoin,
) -> ResponseWithInterChainMsgs<'r, ()>
where
    Wasm: WasmTrait,
{
    test_case
        .app
        .send_tokens(
            Addr::unchecked("ica0"),
            Addr::unchecked(ADMIN),
            std::slice::from_ref(cw_payment),
        )
        .unwrap();

    test_case.send_funds_from_admin(Addr::unchecked("ica0"), &[cwcoin(swap_out_lpn)]);

    test_case
        .app
        .sudo(
            contract_addr,
            &super::construct_response(Binary(platform::trx::encode_msg_responses(
                [platform::trx::encode_msg_response(
                    MsgSwapExactAmountInResponse {
                        token_out_amount: Amount::from(swap_out_lpn).to_string(),
                    },
                    MsgSwapExactAmountIn::TYPE_URL,
                )]
                .into_iter(),
            ))),
        )
        .unwrap()
        .ignore_response()
}

fn expect_remote_ibc_transfer(mut response: ResponseWithInterChainMsgs<'_, ()>) {
    response.expect_submit_tx(
        TestCase::LEASER_CONNECTION_ID,
        "0",
        &[MsgTransfer::TYPE_URL],
    );

    response.unwrap_response()
}

fn do_remote_ibc_transfer<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle>(
    test_case: &mut TestCase<Wasm, Dispatcher, Treasury, Profit, Leaser, Lpp, Oracle, Addr>,
    contract_addr: Addr,
    cw_swap_out_lpn: &CwCoin,
) -> AppResponse
where
    Wasm: WasmTrait,
{
    test_case
        .app
        .send_tokens(
            Addr::unchecked("ica0"),
            contract_addr.clone(),
            std::slice::from_ref(cw_swap_out_lpn),
        )
        .unwrap();

    test_case
        .app
        .sudo(contract_addr, &super::construct_response(Binary::default()))
        .unwrap()
        .unwrap_response()
}
