use currencies::{LeaseGroup, PaymentC5, PaymentGroup};
use currency::CurrencyDef;
use finance::{
    coin::{Amount, Coin},
    price,
    zero::Zero,
};
use lease::{
    api::{
        position::{FullClose, PartialClose, PositionClose},
        query::StateResponse,
        ExecuteMsg,
    },
    error::ContractError,
};
use platform::coin_legacy::to_cosmwasm_on_dex;
use sdk::{
    cosmwasm_std::{Addr, Event},
    cw_multi_test::AppResponse,
};
use swap::testing::SwapRequest;

use crate::common::{
    self, ibc,
    leaser::{self, Instantiator},
    test_case::{response::ResponseWithInterChainMsgs, TestCase},
    CwCoin, ADMIN, USER,
};

use super::{
    LeaseCoin, LeaseCurrency, LeaseTestCase, LpnCoin, LpnCurrency, PaymentCoin, PaymentCurrency,
    DOWNPAYMENT,
};

#[test]
fn close_by_another_user() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let lease = super::open_lease(&mut test_case, DOWNPAYMENT, None);
    assert_unauthorized(
        &mut test_case,
        lease.clone(),
        ExecuteMsg::ClosePosition(PositionClose::FullClose(FullClose {})),
    );
    assert_unauthorized(
        &mut test_case,
        lease,
        ExecuteMsg::ClosePosition(PositionClose::PartialClose(PartialClose {
            amount: LeaseCoin::from(1234414).into(),
        })),
    );
}

#[test]
fn full_close() {
    let lease_amount: LeaseCoin = lease_amount();
    let customer = Addr::unchecked(USER);
    let mut test_case = super::create_test_case::<PaymentCurrency>();

    let exp_loan_close = true;
    let exp_change = price::total(DOWNPAYMENT, super::price_lpn_of()).unwrap();
    let lease = do_close(
        &mut test_case,
        &customer,
        lease_amount,
        PositionClose::FullClose(FullClose {}),
        exp_loan_close,
        exp_change,
        LeaseCoin::ZERO,
    );
    let state = super::state_query(&test_case, lease.as_str());
    assert_eq!(StateResponse::Closed(), state);

    assert_eq!(
        lease_balance(&test_case, lease),
        common::cwcoin_as_balance(LeaseCoin::ZERO),
    );

    leaser::assert_no_leases(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        customer.clone(),
    );

    assert_eq!(
        exp_change,
        user_balance::<LpnCurrency>(&customer, &test_case)
    );
}

#[test]
fn partial_close_loan_not_closed() {
    let lease_amount: LeaseCoin = lease_amount();
    let principal: LpnCoin = price::total(lease_amount, super::price_lpn_of()).unwrap()
        - price::total(DOWNPAYMENT, super::price_lpn_of()).unwrap();
    let close_amount: LeaseCoin =
        price::total(principal - 1234567.into(), super::price_lpn_of().inv()).unwrap();
    let repay_principal = price::total(close_amount, super::price_lpn_of()).unwrap();
    let customer = Addr::unchecked(USER);
    let mut test_case = super::create_test_case::<PaymentCurrency>();

    let exp_loan_close = false;
    let exp_change = LpnCoin::ZERO;
    let lease = do_close(
        &mut test_case,
        &customer,
        close_amount,
        PositionClose::PartialClose(PartialClose {
            amount: close_amount.into(),
        }),
        exp_loan_close,
        exp_change,
        lease_amount - close_amount,
    );
    let state = super::state_query(&test_case, lease.as_str());
    assert_eq!(
        super::expected_open_state(
            &test_case,
            DOWNPAYMENT,
            repay_principal,
            close_amount,
            Instantiator::REPAYMENT_PERIOD,
        ),
        state
    );
    assert_eq!(
        lease_balance(&test_case, lease),
        common::cwcoin_as_balance(exp_change),
    );

    assert_eq!(
        user_balance::<LpnCurrency>(&customer, &test_case),
        LpnCoin::ZERO,
    );
    assert_eq!(
        user_balance::<LeaseCurrency>(&customer, &test_case),
        LeaseCoin::ZERO,
    );
}

#[test]
fn partial_close_loan_closed() {
    let lease_amount: LeaseCoin = lease_amount();
    let principal: LpnCoin = price::total(lease_amount, super::price_lpn_of()).unwrap()
        - price::total(DOWNPAYMENT, super::price_lpn_of()).unwrap();
    let exp_change: LpnCoin = 345.into();

    let repay_principal = principal + exp_change;
    let close_amount: LeaseCoin =
        price::total(repay_principal, super::price_lpn_of().inv()).unwrap();

    let customer = Addr::unchecked(USER);
    let mut test_case = super::create_test_case::<PaymentCurrency>();

    let exp_loan_close = true;
    let lease = do_close(
        &mut test_case,
        &customer,
        close_amount,
        PositionClose::PartialClose(PartialClose {
            amount: close_amount.into(),
        }),
        exp_loan_close,
        exp_change,
        lease_amount - close_amount,
    );
    let state = super::state_query(&test_case, lease.as_str());
    assert_eq!(
        StateResponse::Paid {
            amount: (lease_amount - close_amount).into(),
            in_progress: None
        },
        state
    );

    assert_eq!(
        lease_balance(&test_case, lease),
        common::cwcoin_as_balance(exp_change),
    );

    assert_eq!(
        LpnCoin::ZERO,
        user_balance::<LpnCurrency>(&customer, &test_case)
    );
    assert_eq!(
        LeaseCoin::ZERO,
        user_balance::<LeaseCurrency>(&customer, &test_case)
    );
}

#[test]
fn partial_close_invalid_currency() {
    let mut test_case: LeaseTestCase = super::create_test_case::<PaymentCurrency>();

    let lease: Addr = super::open_lease(&mut test_case, DOWNPAYMENT, None);

    let err = test_case
        .app
        .execute(
            Addr::unchecked(USER),
            lease,
            &(&ExecuteMsg::ClosePosition(PositionClose::PartialClose(PartialClose {
                amount: Coin::<PaymentC5>::from(12345678).into(),
            }))),
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.root_cause().downcast_ref::<currency::error::Error>(),
        Some(&currency::error::Error::currency_mismatch(
            &currency::dto::<LeaseCurrency, LeaseGroup>(),
            &currency::dto::<PaymentC5, PaymentGroup>()
        ))
    );
}

#[test]
fn partial_close_min_asset() {
    let min_asset_lpn = Instantiator::min_asset().try_into().unwrap();
    let min_asset = price::total(min_asset_lpn, super::price_lpn_of().inv()).unwrap();
    let lease_amount: LeaseCoin = lease_amount();

    let mut test_case = super::create_test_case::<PaymentCurrency>();

    let lease = super::open_lease(&mut test_case, DOWNPAYMENT, None);
    let msg = &ExecuteMsg::ClosePosition(PositionClose::PartialClose(PartialClose {
        amount: (lease_amount - min_asset + 1.into()).into(),
    }));

    let err = test_case
        .app
        .execute(Addr::unchecked(USER), lease, msg, &[])
        .unwrap_err();
    assert_eq!(
        err.root_cause().downcast_ref::<ContractError>(),
        Some(&ContractError::PositionCloseAmountTooBig(
            min_asset_lpn.into()
        ))
    );
}

#[test]
fn partial_close_min_transaction() {
    let min_transaction_lpn = Instantiator::min_transaction().try_into().unwrap();
    let min_transaction: LeaseCoin =
        price::total(min_transaction_lpn, super::price_lpn_of().inv()).unwrap();

    let mut test_case = super::create_test_case::<PaymentCurrency>();

    let lease = super::open_lease(&mut test_case, DOWNPAYMENT, None);
    let msg = &ExecuteMsg::ClosePosition(PositionClose::PartialClose(PartialClose {
        amount: (min_transaction - 1.into()).into(),
    }));

    let err = test_case
        .app
        .execute(Addr::unchecked(USER), lease, msg, &[])
        .unwrap_err();
    assert_eq!(
        err.root_cause().downcast_ref::<ContractError>(),
        Some(&ContractError::PositionCloseAmountTooSmall(
            min_transaction_lpn.into()
        ))
    );
}

fn do_close(
    test_case: &mut LeaseTestCase,
    customer_addr: &Addr,
    close_amount: LeaseCoin,
    close_msg: PositionClose,
    exp_loan_close: bool,
    exp_change: LpnCoin,
    exp_lease_amount_after: LeaseCoin,
) -> Addr {
    let user_balance_before: PaymentCoin = user_balance(customer_addr, test_case);
    let lease_addr: Addr = super::open_lease(test_case, DOWNPAYMENT, None);

    assert!(matches!(
        super::expected_newly_opened_state(test_case, DOWNPAYMENT, Coin::<LpnCurrency>::ZERO),
        StateResponse::Opened { .. }
    ));

    let close_amount_in_lpn: LpnCoin = price::total(close_amount, super::price_lpn_of()).unwrap();
    let mut response_close: ResponseWithInterChainMsgs<'_, ()> = send_close(
        test_case,
        lease_addr.clone(),
        &ExecuteMsg::ClosePosition(close_msg),
    );

    let requests: Vec<SwapRequest<PaymentGroup, PaymentGroup>> = common::swap::expect_swap(
        &mut response_close,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    let mut response_swap: ResponseWithInterChainMsgs<'_, ()> = common::swap::do_swap(
        &mut test_case.app,
        lease_addr.clone(),
        TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID),
        requests.into_iter(),
        |amount: Amount, _, _| {
            assert_eq!(amount, close_amount.into());

            close_amount_in_lpn.into()
        },
    )
    .ignore_response();

    let transfer_amount: CwCoin = ibc::expect_remote_transfer(
        &mut response_swap,
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    assert_eq!(transfer_amount, to_cosmwasm_on_dex(close_amount_in_lpn));

    let response_transfer_in: AppResponse = ibc::do_transfer(
        &mut test_case.app,
        TestCase::ica_addr(lease_addr.as_str(), TestCase::LEASE_ICA_ID),
        lease_addr.clone(),
        true,
        &transfer_amount,
    )
    .unwrap_response();

    response_transfer_in.assert_event(
        &Event::new("wasm-ls-close-position")
            .add_attribute("to", lease_addr.clone())
            .add_attribute(
                "payment-amount",
                Amount::from(close_amount_in_lpn).to_string(),
            )
            .add_attribute("payment-symbol", LpnCurrency::ticker())
            .add_attribute("loan-close", exp_loan_close.to_string())
            .add_attribute(
                "principal",
                Amount::from(close_amount_in_lpn - exp_change).to_string(),
            )
            .add_attribute("change", Amount::from(exp_change).to_string())
            .add_attribute("amount-amount", Amount::from(close_amount).to_string())
            .add_attribute("amount-symbol", LeaseCurrency::ticker()),
    );

    assert_eq!(
        user_balance::<PaymentCurrency>(customer_addr, test_case),
        user_balance_before - DOWNPAYMENT,
    );

    if !exp_lease_amount_after.is_zero() {
        assert_eq!(
            test_case
                .app
                .query()
                .query_all_balances(TestCase::ica_addr(
                    lease_addr.as_str(),
                    TestCase::LEASE_ICA_ID
                ))
                .unwrap()
                .as_slice(),
            &[to_cosmwasm_on_dex(exp_lease_amount_after)],
        );
    }

    lease_addr
}

fn send_close<'r>(
    test_case: &'r mut LeaseTestCase,
    contract_addr: Addr,
    msg: &ExecuteMsg,
) -> ResponseWithInterChainMsgs<'r, ()> {
    test_case
        .app
        .execute(Addr::unchecked(USER), contract_addr, msg, &[])
        .unwrap()
        .ignore_response()
}

fn assert_unauthorized(test_case: &mut LeaseTestCase, lease: Addr, close_msg: ExecuteMsg) {
    let sender = Addr::unchecked(ADMIN);
    {
        let err = test_case
            .app
            .execute(sender, lease, &close_msg, &[])
            .unwrap_err();
        assert_eq!(
            err.root_cause()
                .downcast_ref::<access_control::error::Error>(),
            Some(&access_control::error::Error::Unauthorized {})
        );
    }
}

fn user_balance<C>(customer: &Addr, test_case: &LeaseTestCase) -> Coin<C>
where
    C: CurrencyDef,
{
    platform::bank::balance::<C, C::Group>(customer, test_case.app.query()).unwrap()
}

fn lease_balance(test_case: &LeaseTestCase, lease: Addr) -> Vec<CwCoin> {
    test_case.app.query().query_all_balances(lease).unwrap()
}

fn lease_amount() -> LeaseCoin {
    2857142857142.into()
}
