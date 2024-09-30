use ::lease::api::query::StateResponse;
use finance::{coin::Amount, duration::Duration};

use crate::{
    common::{leaser::Instantiator as LeaserInstantiator, lpp::LppQueryMsg},
    lease::{self, LeaseCoin},
};

use super::{LpnCoin, LpnCurrency, PaymentCurrency, DOWNPAYMENT};

#[test]
fn manual_calculation() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;
    let lease_address = super::open_lease(&mut test_case, downpayment, None);
    let quote_result = lease::quote_query(&test_case, downpayment);

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(query_result, expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let query_result = super::state_query(&test_case, &lease_address.into_string());
    let expected_result = StateResponse::Opened {
        amount: LeaseCoin::from(Amount::from(DOWNPAYMENT + 1_857_142_857_142.into())).into(),
        loan_interest_rate: quote_result.annual_interest_rate,
        margin_interest_rate: quote_result.annual_interest_rate_margin,
        principal_due: LpnCoin::new(1_857_142_857_142).into(),
        overdue_margin: LpnCoin::new(13_737_769_080).into(),
        overdue_interest: LpnCoin::new(32_054_794_520).into(),
        overdue_collect_in: Duration::default(),
        due_margin: LpnCoin::new(13_737_769_080).into(),
        due_interest: LpnCoin::new(32_054_794_520).into(),
        validity: super::block_time(&test_case),
        in_progress: None,
    };

    assert_eq!(query_result, expected_result);
}

#[test]
fn lpp_state_implicit_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;
    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(query_result, expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let loan_resp: lpp::msg::LoanResponse<LpnCurrency> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: lease_address.clone(),
            },
        )
        .unwrap();

    let query_result = if let StateResponse::Opened {
        principal_due,
        overdue_interest,
        due_interest,
        ..
    } = super::state_query(&test_case, &lease_address.into_string())
    {
        (
            LpnCoin::try_from(principal_due).unwrap(),
            LpnCoin::try_from(overdue_interest).unwrap() + LpnCoin::try_from(due_interest).unwrap(),
        )
    } else {
        unreachable!();
    };

    assert_eq!(
        query_result,
        (
            loan_resp.principal_due,
            loan_resp
                .interest_due(&(test_case.app.block_info().time))
                .unwrap()
        )
    );
}

#[test]
fn lpp_state_explicit_time() {
    let mut test_case = super::create_test_case::<PaymentCurrency>();
    let downpayment = DOWNPAYMENT;
    let lease_address = super::open_lease(&mut test_case, downpayment, None);

    let query_result = super::state_query(&test_case, lease_address.as_ref());
    let expected_result =
        super::expected_newly_opened_state(&test_case, downpayment, super::create_payment_coin(0));

    assert_eq!(query_result, expected_result);

    test_case.app.time_shift(
        LeaserInstantiator::REPAYMENT_PERIOD + LeaserInstantiator::REPAYMENT_PERIOD
            - Duration::from_nanos(1),
    );

    let loan: lpp::msg::LoanResponse<LpnCurrency> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: lease_address.clone(),
            },
        )
        .unwrap();

    let query_result = if let StateResponse::Opened {
        overdue_interest,
        due_interest,
        ..
    } = super::state_query(&test_case, &lease_address.into_string())
    {
        LpnCoin::try_from(overdue_interest).unwrap() + LpnCoin::try_from(due_interest).unwrap()
    } else {
        unreachable!();
    };

    assert_eq!(
        query_result,
        loan.interest_due(&lease::block_time(&test_case)).unwrap()
    );
}
