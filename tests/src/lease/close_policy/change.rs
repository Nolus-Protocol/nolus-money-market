use ::lease::{
    api::{
        position::ChangeCmd,
        query::{ClosePolicy, StateResponse},
    },
    error::{ContractError, PositionError},
    CloseStrategy,
};
use anyhow::Error;
use finance::{coin::Coin, percent::Percent};
use sdk::{cosmwasm_std::Addr, testing};

use crate::{
    common::{oracle, ADMIN},
    lease::{
        self, LeaseCurrency, LeaseTestCase, LeaserInstantiator, LpnCurrency, PaymentCurrency,
        DOWNPAYMENT,
    },
};

#[test]
fn by_another_user() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);
    super::change_unauthorized(&mut test_case, lease.clone());
}

#[test]
fn tp_zero() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);

    let err = super::change_err(
        &mut test_case,
        lease,
        Some(ChangeCmd::Set(Percent::ZERO)),
        Some(ChangeCmd::Reset),
    );

    assert_eq!(
        err.downcast_ref::<ContractError>(),
        Some(&ContractError::PositionError(
            PositionError::ZeroClosePolicy("take profit")
        ))
    );
}

#[test]
fn sl_zero() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);

    let err = super::change_err(
        &mut test_case,
        lease,
        Some(ChangeCmd::Reset),
        Some(ChangeCmd::Set(Percent::ZERO)),
    );

    assert_eq!(
        err.downcast_ref::<ContractError>(),
        Some(&ContractError::PositionError(
            PositionError::ZeroClosePolicy("stop loss")
        ))
    );
}

#[test]
fn tp_trigger() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);

    let init_ltv = LeaserInstantiator::INITIAL_LTV;
    let tp_new = init_ltv + Percent::from_permille(1);
    let err = super::change_err(
        &mut test_case,
        lease,
        Some(ChangeCmd::Set(tp_new)),
        Some(ChangeCmd::Reset),
    );

    assert_trigger_tp_error(err, tp_new);
}

#[test]
fn sl_trigger() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);

    let init_ltv = LeaserInstantiator::INITIAL_LTV;
    let sl_new = init_ltv;
    let err = super::change_err(
        &mut test_case,
        lease,
        Some(ChangeCmd::Reset),
        Some(ChangeCmd::Set(sl_new)),
    );

    let Some(ContractError::PositionError(PositionError::TriggerClose {
        lease_ltv: _,
        strategy: CloseStrategy::StopLoss(sl_trigger),
    })) = err.downcast_ref::<ContractError>()
    else {
        unreachable!()
    };
    assert_eq!(&sl_new, sl_trigger);
}

#[test]
fn liquidation_conflict() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);

    let max_ltv = LeaserInstantiator::MAX_LTV;
    let sl_new = max_ltv;
    let err = super::change_err(
        &mut test_case,
        lease.clone(),
        Some(ChangeCmd::Set(sl_new - Percent::from_permille(1))),
        Some(ChangeCmd::Set(sl_new)),
    );

    let Some(ContractError::PositionError(PositionError::LiquidationConflict {
        strategy: CloseStrategy::StopLoss(sl),
        top_bound,
    })) = err.downcast_ref::<ContractError>()
    else {
        unreachable!()
    };
    assert_eq!(&sl_new, sl);
    assert_eq!(&max_ltv, top_bound);

    assert!(matches!(
        super::change_err(
            &mut test_case,
            lease,
            Some(ChangeCmd::Set(sl_new)),
            Some(ChangeCmd::Reset),
        )
        .downcast_ref::<ContractError>(),
        Some(ContractError::PositionError(
            PositionError::LiquidationConflict {
                strategy: CloseStrategy::TakeProfit(_),
                top_bound: _,
            }
        ))
    ));
}

#[test]
fn tp_set() {
    let mut test_case = lease::create_test_case::<PaymentCurrency>();
    // LeaseCLpnC = 1:1
    let lease = lease::open_lease(&mut test_case, DOWNPAYMENT, None);
    assert_eq!(
        ClosePolicy::default(),
        query_policy(&test_case, lease.clone())
    );

    let tp = Percent::from_percent(28);
    super::change_ok(
        &mut test_case,
        lease.clone(),
        Some(ChangeCmd::Set(tp)),
        Some(ChangeCmd::Reset),
    );
    assert_eq!(
        ClosePolicy::new_testing(Some(tp), None),
        query_policy(&test_case, lease.clone())
    );

    // LeaseC/LpnC = 10/25
    oracle::feed_price(
        &mut test_case,
        testing::user(ADMIN),
        Coin::<LeaseCurrency>::from(10),
        Coin::<LpnCurrency>::from(25),
    );
    let err = super::change_err(
        &mut test_case,
        lease.clone(),
        Some(ChangeCmd::Set(tp)),
        Some(ChangeCmd::Reset),
    );
    assert_trigger_tp_error(err, tp)
}

fn query_policy(test_case: &LeaseTestCase, lease: Addr) -> ClosePolicy {
    let StateResponse::Opened { close_policy, .. } = lease::state_query(test_case, lease) else {
        unreachable!()
    };
    close_policy
}

fn assert_trigger_tp_error(err: Error, exp_tp: Percent) {
    let Some(ContractError::PositionError(PositionError::TriggerClose {
        lease_ltv: _,
        strategy: CloseStrategy::TakeProfit(tp_trigger),
    })) = err.downcast_ref::<ContractError>()
    else {
        unreachable!()
    };
    assert_eq!(&exp_tp, tp_trigger);
}
