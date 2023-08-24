use ::lease::api::StateResponse;
use finance::{coin::Coin, duration::Duration};

use crate::{common::leaser::Instantiator as LeaserInstantiator, lease};

use super::{CoinLpn, LeaseCurrency, Lpn, PaymentCurrency, DOWNPAYMENT};

#[test]
fn manual_calculation() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = super::create_payment_coin(DOWNPAYMENT);
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let quote_result = dbg!(lease::quote_query(&test_case, downpayment));

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(dbg!(query_result), expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let query_result = super::state_query(&test_case, &lease_address.into_string());
    let expected_result = StateResponse::Opened {
        amount: Coin::<LeaseCurrency>::new(DOWNPAYMENT + 1_857_142_857_142).into(),
        loan_interest_rate: quote_result.annual_interest_rate,
        margin_interest_rate: quote_result.annual_interest_rate_margin,
        principal_due: CoinLpn::new(1_857_142_857_142).into(),
        previous_margin_due: CoinLpn::new(13_737_769_080).into(),
        previous_interest_due: CoinLpn::new(32_054_794_520).into(),
        current_margin_due: CoinLpn::new(13_737_769_080).into(),
        current_interest_due: CoinLpn::new(32_054_794_520).into(),
        validity: super::block_time(&test_case),
        in_progress: None,
    };

    assert_eq!(dbg!(query_result), expected_result);
}

#[test]
fn lpp_state_implicit_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = super::create_payment_coin(DOWNPAYMENT);
    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(dbg!(query_result), expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let loan_resp: lpp::msg::LoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &lpp::msg::QueryMsg::Loan {
                lease_addr: lease_address.clone(),
            },
        )
        .unwrap();

    let query_result = if let StateResponse::Opened {
        principal_due,
        previous_interest_due,
        current_interest_due,
        ..
    } = super::state_query(&test_case, &lease_address.into_string())
    {
        (
            CoinLpn::try_from(principal_due).unwrap(),
            CoinLpn::try_from(previous_interest_due).unwrap()
                + CoinLpn::try_from(current_interest_due).unwrap(),
        )
    } else {
        unreachable!();
    };

    assert_eq!(
        query_result,
        (
            loan_resp.principal_due,
            loan_resp.interest_due(test_case.app.block_info().time)
        )
    );
}

#[test]
fn lpp_state_explicit_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = super::create_payment_coin(DOWNPAYMENT);
    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(dbg!(query_result), expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let loan: lpp::msg::LoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &lpp::msg::QueryMsg::Loan {
                lease_addr: lease_address.clone(),
            },
        )
        .unwrap();

    let query_result = if let StateResponse::Opened {
        previous_interest_due,
        current_interest_due,
        ..
    } = super::state_query(&test_case, &lease_address.into_string())
    {
        CoinLpn::try_from(previous_interest_due).unwrap()
            + CoinLpn::try_from(current_interest_due).unwrap()
    } else {
        unreachable!();
    };

    assert_eq!(
        query_result,
        loan.interest_due(lease::block_time(&test_case))
    );
}
