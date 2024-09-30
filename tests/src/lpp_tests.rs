use currencies::{LeaseC1, Lpn, Lpns, Native, Nls};
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    fraction::Fraction,
    percent::{Percent, Units as PercentUnits},
    price,
    ratio::Rational,
    test,
};
use lpp::{
    borrow::InterestRate,
    msg::{
        BalanceResponse, LppBalanceResponse, PriceResponse, QueryLoanResponse, QueryQuoteResponse,
        RewardsResponse, SudoMsg,
    },
    state::Config,
};
use platform::{bank, coin_legacy};
use sdk::{
    cosmwasm_std::{Addr, Event, Timestamp},
    cw_multi_test::AppResponse,
};

use crate::common::{
    cwcoin,
    lease::{
        InitConfig as LeaseInitConfig, Instantiator as LeaseInstantiator,
        InstantiatorAddresses as LeaseInstantiatorAddresses,
        InstantiatorConfig as LeaseInstantiatorConfig,
    },
    lpp::{LppExecuteMsg, LppQueryMsg},
    protocols::Registry,
    test_case::{app::App, builder::BlankBuilder as TestCaseBuilder, TestCase},
    CwCoin, ADDON_OPTIMAL_INTEREST_RATE, ADMIN, BASE_INTEREST_RATE, UTILIZATION_OPTIMAL,
};
type LeaseCurrency = LeaseC1;

fn general_interest_rate(
    loan: u32,
    balance: u32,
    base_rate: Percent,
    addon_rate: Percent,
    optimal_rate: Percent,
) -> Percent {
    let utilization_rate = Percent::from_ratio(loan, balance).unwrap().min(
        Percent::from_ratio(
            optimal_rate.units(),
            (Percent::HUNDRED - optimal_rate).units(),
        )
        .unwrap(),
    );

    base_rate
        + Fraction::<PercentUnits>::of(
            &Rational::new(addon_rate.units(), optimal_rate.units()),
            utilization_rate,
        )
        .unwrap()
}

#[test]
fn config_update_parameters() {
    let app_balance = 10_000_000_000u128;

    let base_interest_rate = Percent::from_permille(210);
    let addon_optimal_interest_rate = Percent::from_permille(200);
    let utilization_optimal = Percent::from_permille(550);
    let min_utilization = Percent::from_permille(500).try_into().unwrap();

    assert_ne!(base_interest_rate, BASE_INTEREST_RATE);
    assert_ne!(addon_optimal_interest_rate, ADDON_OPTIMAL_INTEREST_RATE);
    assert_ne!(utilization_optimal, UTILIZATION_OPTIMAL);
    assert_ne!(min_utilization, TestCase::DEFAULT_LPP_MIN_UTILIZATION);

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[
        lpn_cwcoin(app_balance),
        cwcoin::<Nls, _>(app_balance),
    ])
    .init_lpp(
        None,
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .into_generic();

    let response: AppResponse = test_case
        .app
        .sudo(
            test_case.address_book.lpp().clone(),
            &SudoMsg::NewBorrowRate {
                borrow_rate: InterestRate::new(
                    base_interest_rate,
                    utilization_optimal,
                    addon_optimal_interest_rate,
                )
                .expect("Couldn't construct interest rate value!"),
            },
        )
        .unwrap()
        .unwrap_response();

    assert!(response.data.is_none());
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_address", "contract0"),]
    );

    let response: AppResponse = test_case
        .app
        .sudo(
            test_case.address_book.lpp().clone(),
            &SudoMsg::MinUtilization { min_utilization },
        )
        .unwrap()
        .unwrap_response();

    assert!(response.data.is_none());
    assert_eq!(
        &response.events,
        &[Event::new("sudo").add_attribute("_contract_address", "contract0"),]
    );

    let quote: Config = test_case
        .app
        .query()
        .query_wasm_smart(test_case.address_book.lpp().clone(), &LppQueryMsg::Config())
        .unwrap();

    assert_eq!(quote.borrow_rate().base_interest_rate(), base_interest_rate);
    assert_eq!(
        quote.borrow_rate().utilization_optimal(),
        utilization_optimal
    );
    assert_eq!(
        quote.borrow_rate().addon_optimal_interest_rate(),
        addon_optimal_interest_rate
    );
    assert_eq!(quote.min_utilization(), min_utilization);
}

#[test]
#[should_panic(expected = "Expecting code id 1 for the contract contract0")]
fn open_loan_unauthorized_contract_id() {
    let mut test_case = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_protocols_registry(Registry::NoProtocol)
        .init_oracle(None)
        .init_treasury()
        .init_profit(24)
        .into_generic();

    () = test_case
        .app
        .execute(
            test_case.address_book.lpp().clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::OpenLoan {
                amount: test::funds::<_, Lpn>(100),
            },
            &[lpn_cwcoin(200)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();
}

#[test]
#[should_panic(expected = "No liquidity")]
fn open_loan_no_liquidity() {
    let mut test_case = TestCaseBuilder::<Lpn>::new()
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_protocols_registry(Registry::NoProtocol)
        .init_oracle(None)
        .init_treasury()
        .init_profit(24)
        .init_reserve()
        .init_leaser()
        .into_generic();

    let lease_addr: Addr = test_case.open_lease::<Lpn>(currency::dto::<LeaseCurrency, _>());

    () = test_case
        .app
        .execute(
            lease_addr,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::OpenLoan {
                amount: test::funds::<_, Lpn>(2500),
            },
            &[lpn_cwcoin(200)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();
}

#[test]
fn deposit_and_withdraw() {
    let app_balance = 10_000_000_000;
    let init_deposit = 20_000;
    let lpp_balance_push = 80_000;
    let pushed_price = (lpp_balance_push + init_deposit) / init_deposit;
    let test_deposit = 10_004;
    let rounding_error = test_deposit % pushed_price; // should be 4 for this setup
    let post_deposit = 1_000_000;
    let loan = 1_000_000;
    let overdraft = 5_000;
    let withdraw_amount_nlpn = 1000u128;
    let rest_nlpn = test_deposit / pushed_price - withdraw_amount_nlpn;

    let admin = Addr::unchecked(ADMIN);

    let lender1 = Addr::unchecked("lender1");
    let lender2 = Addr::unchecked("lender2");
    let lender3 = Addr::unchecked("lender3");

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[lpn_cwcoin(app_balance)])
        .init_lpp_with_funds(
            None,
            &[],
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .init_time_alarms()
        .init_protocols_registry(Registry::NoProtocol)
        .init_oracle(None)
        .init_treasury()
        .init_profit(24)
        .init_reserve()
        .init_leaser()
        .into_generic();

    test_case
        .send_funds_from_admin(lender1.clone(), &[lpn_cwcoin(init_deposit)])
        .send_funds_from_admin(
            lender2.clone(),
            &[lpn_cwcoin(init_deposit.max(test_deposit))],
        )
        .send_funds_from_admin(
            lender3.clone(),
            &[lpn_cwcoin(init_deposit.max(post_deposit))],
        );

    // initial deposit
    let _: AppResponse = test_case
        .app
        .execute(
            lender1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(init_deposit)],
        )
        .unwrap()
        .unwrap_response();

    // push the price from 1, should be allowed as an interest from previous leases for example.
    () = test_case
        .app
        .send_tokens(
            admin,
            test_case.address_book.lpp().clone(),
            &[lpn_cwcoin(lpp_balance_push)],
        )
        .unwrap();

    let price: PriceResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(test_case.address_book.lpp().clone(), &LppQueryMsg::Price())
        .unwrap();
    assert_eq!(
        price::total(Coin::new(1_000), price.0).unwrap(),
        Coin::<Lpn>::new(1_000 * pushed_price)
    );

    // deposit to check,
    let _: AppResponse = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(test_deposit)],
        )
        .unwrap()
        .unwrap_response();

    // got rounding error
    let balance_nlpn: BalanceResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Balance {
                address: lender2.clone(),
            },
        )
        .unwrap();
    let price: PriceResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(test_case.address_book.lpp().clone(), &LppQueryMsg::Price())
        .unwrap();
    assert_eq!(
        price::total(balance_nlpn.balance.into(), price.0).unwrap(),
        Coin::<Lpn>::new(test_deposit - rounding_error)
    );

    // other deposits should not change asserts for lender2
    let _: AppResponse = test_case
        .app
        .execute(
            lender3.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(post_deposit)],
        )
        .unwrap()
        .unwrap_response();

    let balance_nlpn: BalanceResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Balance {
                address: lender2.clone(),
            },
        )
        .unwrap();
    let price: PriceResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(test_case.address_book.lpp().clone(), &LppQueryMsg::Price())
        .unwrap();
    assert_eq!(
        price::total(balance_nlpn.balance.into(), price.0).unwrap(),
        Coin::<Lpn>::new(test_deposit - rounding_error)
    );

    // loans should not change asserts for lender2, the default loan
    let _: Addr = LeaseInstantiator::instantiate::<Lpn>(
        &mut test_case.app,
        test_case.address_book.lease_code(),
        LeaseInstantiatorAddresses {
            lpp: test_case.address_book.lpp().clone(),
            time_alarms: test_case.address_book.time_alarms().clone(),
            oracle: test_case.address_book.oracle().clone(),
            profit: test_case.address_book.profit().clone(),
            reserve: test_case.address_book.reserve().clone(),
            finalizer: test_case.address_book.leaser().clone(),
        },
        LeaseInitConfig::new(currency::dto::<LeaseCurrency, _>(), loan.into(), None),
        LeaseInstantiatorConfig {
            liability_init_percent: Percent::from_percent(50), // simplify case: borrow == downpayment
            ..LeaseInstantiatorConfig::default()
        },
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    let balance_nlpn2: BalanceResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Balance {
                address: lender2.clone(),
            },
        )
        .unwrap();
    let price: PriceResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(test_case.address_book.lpp().clone(), &LppQueryMsg::Price())
        .unwrap();
    assert_eq!(
        price::total(balance_nlpn2.balance.into(), price.0).unwrap(),
        Coin::<Lpn>::new(test_deposit - rounding_error)
    );

    // try to withdraw with overdraft
    let to_burn: u128 = balance_nlpn.balance.u128() - rounding_error + overdraft;
    _ = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Burn {
                amount: to_burn.into(),
            },
            &[],
        )
        .unwrap_err();

    // partial withdraw
    () = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Burn {
                amount: withdraw_amount_nlpn.into(),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let balance_nlpn: BalanceResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Balance {
                address: lender2.clone(),
            },
        )
        .unwrap();
    assert_eq!(balance_nlpn.balance.u128(), rest_nlpn);

    // full withdraw, should close lender's account
    () = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Burn {
                amount: (rest_nlpn).into(),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let balance_nlpn: BalanceResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp(),
            &LppQueryMsg::Balance { address: lender2 },
        )
        .unwrap();
    assert_eq!(balance_nlpn.balance.u128(), 0);
}

#[test]
fn loan_open_wrong_id() {
    let _admin = Addr::unchecked(ADMIN);
    let lender = Addr::unchecked("lender");
    let hacker = Addr::unchecked("Mallory");

    let app_balance = 10_000_000_000u128;
    let hacker_balance = 10_000_000;
    let init_deposit = 20_000_000u128;
    let loan = 10_000u128;

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[lpn_cwcoin(app_balance)])
        .init_lpp(
            None,
            BASE_INTEREST_RATE,
            UTILIZATION_OPTIMAL,
            ADDON_OPTIMAL_INTEREST_RATE,
            TestCase::DEFAULT_LPP_MIN_UTILIZATION,
        )
        .into_generic();

    test_case
        .send_funds_from_admin(lender, &[lpn_cwcoin(init_deposit)])
        .send_funds_from_admin(hacker.clone(), &[lpn_cwcoin(hacker_balance)]);

    _ = test_case
        .app
        .execute(
            hacker,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::OpenLoan {
                amount: Coin::<Lpn>::new(loan).into(),
            },
            &[],
        )
        .unwrap_err();
}

#[test]
fn loan_open_and_repay() {
    const LOCAL_BASE_INTEREST_RATE: Percent = Percent::from_permille(210);
    const LOCAL_ADDON_OPTIMAL_INTEREST_RATE: Percent = Percent::from_permille(200);
    const LOCAL_UTILIZATION_OPTIMAL_RATE: Percent = Percent::from_permille(550);

    fn interest_rate(loan: u32, balance: u32) -> Percent {
        general_interest_rate(
            loan,
            balance,
            LOCAL_BASE_INTEREST_RATE,
            LOCAL_ADDON_OPTIMAL_INTEREST_RATE,
            LOCAL_UTILIZATION_OPTIMAL_RATE,
        )
    }

    const YEAR: u64 = Duration::YEAR.nanos();

    let admin = Addr::unchecked(ADMIN);
    let lender = Addr::unchecked("lender");
    let hacker = Addr::unchecked("Mallory");

    let app_balance = 10_000_000_000u128;
    let hacker_balance = 10_000_000;
    let init_deposit_u32 = 20_000_000u32;
    let init_deposit = Amount::from(init_deposit_u32);
    let loan1_u32 = 10_000_000u32;
    let loan1 = Amount::from(loan1_u32);
    let balance1_u32 = init_deposit_u32 - loan1_u32;
    let loan2_u32 = 5_000_000u32;
    let loan2 = Amount::from(loan2_u32);
    let repay_interest_part = 1_000_000u128;
    let repay_due_part = 1_000_000u128;
    let repay_excess = 1_000_000u128;

    let interest1 = interest_rate(loan1_u32, balance1_u32);

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[
        lpn_cwcoin(app_balance),
        cwcoin::<Nls, _>(app_balance),
    ])
    .init_lpp_with_funds(
        None,
        &[],
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .init_time_alarms()
    .init_protocols_registry(Registry::NoProtocol)
    .init_oracle(None)
    .init_treasury()
    .init_profit(24)
    .init_reserve()
    .init_leaser()
    .into_generic();

    test_case
        .send_funds_from_admin(lender.clone(), &[lpn_cwcoin(init_deposit)])
        .send_funds_from_admin(hacker.clone(), &[lpn_cwcoin(hacker_balance)]);

    let lease_addresses = LeaseInstantiatorAddresses {
        lpp: test_case.address_book.lpp().clone(),
        time_alarms: test_case.address_book.time_alarms().clone(),
        oracle: test_case.address_book.oracle().clone(),
        profit: test_case.address_book.profit().clone(),
        reserve: test_case.address_book.reserve().clone(),
        finalizer: test_case.address_book.leaser().clone(),
    };

    // initial deposit
    () = test_case
        .app
        .execute(
            lender,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(init_deposit)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    () = test_case
        .app
        .sudo(
            test_case.address_book.lpp().clone(),
            &SudoMsg::NewBorrowRate {
                borrow_rate: InterestRate::new(
                    LOCAL_BASE_INTEREST_RATE,
                    LOCAL_UTILIZATION_OPTIMAL_RATE,
                    LOCAL_ADDON_OPTIMAL_INTEREST_RATE,
                )
                .expect("Couldn't construct interest rate value!"),
            },
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let quote: QueryQuoteResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Quote {
                amount: Coin::<Lpn>::new(loan1).into(),
            },
        )
        .unwrap();
    match quote {
        QueryQuoteResponse::QuoteInterestRate(quote) => assert_eq!(quote, interest1),
        _ => panic!("no liquidity"),
    }

    // borrow
    let loan_addr1 = LeaseInstantiator::instantiate::<Lpn>(
        &mut test_case.app,
        test_case.address_book.lease_code(),
        lease_addresses.clone(),
        LeaseInitConfig::new(currency::dto::<LeaseCurrency, _>(), loan1.into(), None),
        LeaseInstantiatorConfig {
            liability_init_percent: Percent::from_percent(50), // simplify case: borrow == downpayment
            ..LeaseInstantiatorConfig::default()
        },
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    // double borrow
    _ = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::OpenLoan {
                amount: Coin::<Lpn>::new(loan1).into(),
            },
            &[],
        )
        .unwrap_err();

    test_case.app.time_shift(Duration::from_nanos(YEAR / 2));

    let total_interest_due_u32 = interest1.of(loan1_u32).unwrap() / 2;
    let total_interest_due = Amount::from(total_interest_due_u32);

    let resp: LppBalanceResponse<Lpns> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::LppBalance(),
        )
        .unwrap();

    assert_eq!(
        resp.total_interest_due,
        Coin::<Lpn>::new(total_interest_due).into()
    );

    let interest2 = interest_rate(loan1_u32 + loan2_u32 + total_interest_due_u32, balance1_u32);

    let quote: QueryQuoteResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Quote {
                amount: Coin::<Lpn>::new(loan2).into(),
            },
        )
        .unwrap();
    match quote {
        QueryQuoteResponse::QuoteInterestRate(quote) => assert_eq!(quote, interest2),
        _ => panic!("no liquidity"),
    }

    // borrow 2
    let loan_addr2 = LeaseInstantiator::instantiate::<Lpn>(
        &mut test_case.app,
        test_case.address_book.lease_code(),
        lease_addresses,
        LeaseInitConfig::new(currency::dto::<LeaseCurrency, _>(), loan2.into(), None),
        LeaseInstantiatorConfig {
            liability_init_percent: Percent::from_percent(50), // simplify case: borrow == downpayment
            ..LeaseInstantiatorConfig::default()
        },
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    test_case.app.time_shift(Duration::from_nanos(YEAR / 2));

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, loan1.into());
    assert_eq!(loan1_resp.annual_interest_rate, interest1);
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        interest1.of(loan1).unwrap().into()
    );

    // repay from other addr
    _ = test_case
        .app
        .execute(
            hacker,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(loan1)],
        )
        .unwrap_err();

    // repay zero
    _ = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(0)],
        )
        .unwrap_err();

    // repay wrong currency
    () = test_case
        .app
        .send_tokens(
            admin,
            loan_addr2.clone(),
            &[coin_legacy::to_cosmwasm::<Nls>(repay_interest_part.into())],
        )
        .unwrap();

    _ = test_case
        .app
        .execute(
            loan_addr2,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[coin_legacy::to_cosmwasm::<Nls>(repay_interest_part.into())],
        )
        .unwrap_err();

    // repay interest part
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(repay_interest_part)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, loan1.into());
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        (interest1.of(loan1).unwrap() - repay_interest_part).into()
    );

    // repay interest + due part
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(
                interest1.of(loan1).unwrap() - repay_interest_part + repay_due_part,
            )],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, (loan1 - repay_due_part).into());
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        Coin::new(0)
    );

    // repay interest + due part, close the loan
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(loan1 - repay_due_part + repay_excess)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    assert!(maybe_loan1.is_none());

    // repay excess is returned
    let balance = bank::balance::<_, Lpns>(&loan_addr1, test_case.app.query()).unwrap();
    assert_eq!(
        balance,
        Coin::<Lpn>::from(loan1 - interest1.of(loan1).unwrap())
    );

    let resp: LppBalanceResponse<Lpns> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::LppBalance(),
        )
        .unwrap();

    // total unpaid interest
    assert_eq!(
        resp.total_interest_due,
        Coin::<Lpn>::from(interest2.of(loan2).unwrap() / 2u128).into()
    );
    assert_eq!(resp.total_principal_due, Coin::<Lpn>::from(loan2).into());
    assert_eq!(
        resp.balance,
        Coin::<Lpn>::from(init_deposit + interest1.of(loan1).unwrap() - loan2).into()
    );
}

#[test]
fn compare_lpp_states() {
    const LOCAL_BASE_INTEREST_RATE: Percent = Percent::from_permille(210);
    const LOCAL_ADDON_OPTIMAL_INTEREST_RATE: Percent = Percent::from_permille(200);
    const LOCAL_UTILIZATION_OPTIMAL_RATE: Percent = Percent::from_permille(550);

    fn interest_rate(loan: u32, balance: u32) -> Percent {
        general_interest_rate(
            loan,
            balance,
            LOCAL_BASE_INTEREST_RATE,
            LOCAL_ADDON_OPTIMAL_INTEREST_RATE,
            LOCAL_UTILIZATION_OPTIMAL_RATE,
        )
    }

    const YEAR: u64 = Duration::YEAR.nanos();

    let admin = Addr::unchecked(ADMIN);
    let lender = Addr::unchecked("lender");
    let hacker = Addr::unchecked("Mallory");

    let app_balance = 10_000_000_000u128;
    let hacker_balance = 10_000_000;
    let init_deposit_u32 = 20_000_000u32;
    let init_deposit = Amount::from(init_deposit_u32);
    let loan1_u32 = 10_000_000u32;
    let loan1 = Amount::from(loan1_u32);
    let balance1_u32 = init_deposit_u32 - loan1_u32;
    let loan2_u32 = 5_000_000u32;
    let loan2 = Amount::from(loan2_u32);
    let repay_interest_part = 1_000_000u128;
    let repay_due_part = 1_000_000u128;
    let repay_excess = 1_000_000u128;

    let interest1 = interest_rate(loan1_u32, balance1_u32);

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[
        lpn_cwcoin(app_balance),
        coin_legacy::to_cosmwasm::<Nls>(app_balance.into()),
    ])
    .init_lpp_with_funds(
        None,
        &[],
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .init_time_alarms()
    .init_protocols_registry(Registry::NoProtocol)
    .init_oracle(None)
    .init_treasury()
    .init_profit(24)
    .init_reserve()
    .init_leaser()
    .into_generic();

    test_case
        .send_funds_from_admin(lender.clone(), &[lpn_cwcoin(init_deposit)])
        .send_funds_from_admin(hacker.clone(), &[lpn_cwcoin(hacker_balance)]);

    // initial deposit
    () = test_case
        .app
        .execute(
            lender,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(init_deposit)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    () = test_case
        .app
        .sudo(
            test_case.address_book.lpp().clone(),
            &SudoMsg::NewBorrowRate {
                borrow_rate: InterestRate::new(
                    LOCAL_BASE_INTEREST_RATE,
                    LOCAL_UTILIZATION_OPTIMAL_RATE,
                    LOCAL_ADDON_OPTIMAL_INTEREST_RATE,
                )
                .expect("Couldn't construct interest rate value!"),
            },
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let quote: QueryQuoteResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Quote {
                amount: Coin::<Lpn>::new(loan1).into(),
            },
        )
        .unwrap();
    match quote {
        QueryQuoteResponse::QuoteInterestRate(quote) => assert_eq!(quote, interest1),
        _ => panic!("no liquidity"),
    }

    // borrow
    let loan_addr1 = LeaseInstantiator::instantiate::<Lpn>(
        &mut test_case.app,
        test_case.address_book.lease_code(),
        LeaseInstantiatorAddresses {
            lpp: test_case.address_book.lpp().clone(),
            time_alarms: test_case.address_book.time_alarms().clone(),
            oracle: test_case.address_book.oracle().clone(),
            profit: test_case.address_book.profit().clone(),
            reserve: test_case.address_book.reserve().clone(),
            finalizer: test_case.address_book.leaser().clone(),
        },
        LeaseInitConfig::new(currency::dto::<LeaseCurrency, _>(), loan1.into(), None),
        LeaseInstantiatorConfig {
            liability_init_percent: Percent::from_percent(50), // simplify case: borrow == downpayment
            ..LeaseInstantiatorConfig::default()
        },
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    // double borrow
    _ = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::OpenLoan {
                amount: Coin::<Lpn>::new(loan1).into(),
            },
            &[],
        )
        .unwrap_err();

    test_case.app.time_shift(Duration::from_nanos(YEAR / 2));

    let total_interest_due_u32 = interest1.of(loan1_u32).unwrap() / 2;
    let total_interest_due = Amount::from(total_interest_due_u32);

    let resp: LppBalanceResponse<Lpns> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::LppBalance(),
        )
        .unwrap();
    assert_eq!(
        resp.total_interest_due,
        Coin::<Lpn>::new(total_interest_due).into()
    );

    let interest2 = interest_rate(loan1_u32 + loan2_u32 + total_interest_due_u32, balance1_u32);

    let quote: QueryQuoteResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Quote {
                amount: Coin::<Lpn>::new(loan2).into(),
            },
        )
        .unwrap();
    match quote {
        QueryQuoteResponse::QuoteInterestRate(quote) => assert_eq!(quote, interest2),
        _ => panic!("no liquidity"),
    }

    // borrow 2
    let loan_addr2 = LeaseInstantiator::instantiate::<Lpn>(
        &mut test_case.app,
        test_case.address_book.lease_code(),
        LeaseInstantiatorAddresses {
            lpp: test_case.address_book.lpp().clone(),
            time_alarms: test_case.address_book.time_alarms().clone(),
            oracle: test_case.address_book.oracle().clone(),
            profit: test_case.address_book.profit().clone(),
            reserve: test_case.address_book.reserve().clone(),
            finalizer: test_case.address_book.leaser().clone(),
        },
        LeaseInitConfig::new(currency::dto::<LeaseCurrency, _>(), loan2.into(), None),
        LeaseInstantiatorConfig {
            liability_init_percent: Percent::from_percent(50), // simplify case: borrow == downpayment
            ..LeaseInstantiatorConfig::default()
        },
        TestCase::DEX_CONNECTION_ID,
        TestCase::LEASE_ICA_ID,
    );

    test_case.app.time_shift(Duration::from_nanos(YEAR / 2));

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, loan1.into());
    assert_eq!(loan1_resp.annual_interest_rate, interest1);
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        interest1.of(loan1).unwrap().into()
    );

    // repay from other addr
    _ = test_case
        .app
        .execute(
            hacker,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(loan1)],
        )
        .unwrap_err();

    // repay zero
    _ = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(0)],
        )
        .unwrap_err();

    // repay wrong currency
    () = test_case
        .app
        .send_tokens(
            admin,
            loan_addr2.clone(),
            &[coin_legacy::to_cosmwasm::<Nls>(repay_interest_part.into())],
        )
        .unwrap();

    _ = test_case
        .app
        .execute(
            loan_addr2,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[coin_legacy::to_cosmwasm::<Nls>(repay_interest_part.into())],
        )
        .unwrap_err();

    // repay interest part
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(repay_interest_part)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, loan1.into());
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        (interest1.of(loan1).unwrap() - repay_interest_part).into()
    );

    // repay interest + due part
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(
                interest1.of(loan1).unwrap() - repay_interest_part + repay_due_part,
            )],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    let loan1_resp = maybe_loan1.unwrap();
    assert_eq!(loan1_resp.principal_due, (loan1 - repay_due_part).into());
    assert_eq!(
        loan1_resp
            .interest_due(&block_time(&test_case.app))
            .unwrap(),
        Coin::new(0)
    );

    // repay interest + due part, close the loan
    () = test_case
        .app
        .execute(
            loan_addr1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::RepayLoan(),
            &[lpn_cwcoin(loan1 - repay_due_part + repay_excess)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let maybe_loan1: QueryLoanResponse<Lpn> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Loan {
                lease_addr: loan_addr1.clone(),
            },
        )
        .unwrap();
    assert!(maybe_loan1.is_none());

    // repay excess is returned
    let balance = bank::balance::<_, Lpns>(&loan_addr1, test_case.app.query()).unwrap();
    assert_eq!(
        balance,
        Coin::<Lpn>::from(loan1 - interest1.of(loan1).unwrap())
    );

    let resp: LppBalanceResponse<Lpns> = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::LppBalance(),
        )
        .unwrap();

    // total unpaid interest
    assert_eq!(
        resp.total_interest_due,
        Coin::<Lpn>::from(interest2.of(loan2).unwrap() / 2u128).into()
    );
    assert_eq!(resp.total_principal_due, Coin::<Lpn>::from(loan2).into());
    assert_eq!(
        resp.balance,
        Coin::<Lpn>::from(init_deposit + interest1.of(loan1).unwrap() - loan2).into()
    );
}

#[test]
fn test_rewards() {
    let app_balance = 10_000_000_000;
    let deposit1 = 20_000;
    let lpp_balance_push = 80_000;
    let pushed_price = (lpp_balance_push + deposit1) / deposit1;
    let deposit2 = 10_004;
    let treasury_balance = 100_000_000;
    let tot_rewards0 = 5_000_000;
    let tot_rewards1 = 10_000_000;
    let tot_rewards2 = 22_000_000;
    let lender_reward1 = tot_rewards2 * deposit1 / (deposit1 + deposit2 / pushed_price);
    // brackets are important here to reflect rounding errors
    let lender_reward2 =
        tot_rewards2 * (deposit2 / pushed_price) / (deposit1 + deposit2 / pushed_price);

    let _admin = Addr::unchecked(ADMIN);

    let lender1 = Addr::unchecked("lender1");
    let lender2 = Addr::unchecked("lender2");
    let recipient = Addr::unchecked("recipient");
    // simplified
    // TODO: any checks for the sender of rewards?
    let treasury = Addr::unchecked("treasury");

    let mut test_case = TestCaseBuilder::<Lpn>::with_reserve(&[
        lpn_cwcoin(app_balance),
        cwcoin::<Nls, _>(app_balance),
    ])
    .init_lpp_with_funds(
        None,
        &[],
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .into_generic();

    test_case
        .send_funds_from_admin(lender1.clone(), &[lpn_cwcoin(deposit1)])
        .send_funds_from_admin(lender2.clone(), &[lpn_cwcoin(deposit2)])
        .send_funds_from_admin(
            treasury.clone(),
            &[coin_legacy::to_cosmwasm::<Nls>(treasury_balance.into())],
        );

    // rewards before deposits
    _ = test_case
        .app
        .execute(
            treasury.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::DistributeRewards(),
            &[coin_legacy::to_cosmwasm::<Nls>(tot_rewards0.into())],
        )
        .unwrap_err();

    // initial deposit
    () = test_case
        .app
        .execute(
            lender1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(deposit1)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    // push the price from 1, should be allowed as an interest from previous leases for example.
    test_case.send_funds_from_admin(
        test_case.address_book.lpp().clone(),
        &[lpn_cwcoin(lpp_balance_push)],
    );

    () = test_case
        .app
        .execute(
            treasury.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::DistributeRewards(),
            &[coin_legacy::to_cosmwasm::<Nls>(tot_rewards1.into())],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    // deposit after disributing rewards should not get anything
    () = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Deposit(),
            &[lpn_cwcoin(deposit2)],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards {
                address: lender1.clone(),
            },
        )
        .unwrap();

    assert_eq!(resp.rewards, tot_rewards1.into());

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards {
                address: lender2.clone(),
            },
        )
        .unwrap();

    assert_eq!(resp.rewards, Coin::new(0));

    // claim zero rewards
    _ = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::ClaimRewards {
                other_recipient: None,
            },
            &[],
        )
        .unwrap_err();

    // check reward claim with nonvalid recipient
    _ = test_case
        .app
        .execute(
            lender1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::ClaimRewards {
                other_recipient: Some(Addr::unchecked("-")),
            },
            &[],
        )
        .unwrap_err();

    // check reward claim
    () = test_case
        .app
        .execute(
            lender1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::ClaimRewards {
                other_recipient: None,
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards {
                address: lender1.clone(),
            },
        )
        .unwrap();

    assert_eq!(resp.rewards, Coin::new(0));

    let balance = bank::balance::<_, Native>(&lender1, test_case.app.query()).unwrap();
    assert_eq!(balance, Coin::<Nls>::from(tot_rewards1));

    () = test_case
        .app
        .execute(
            treasury,
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::DistributeRewards(),
            &[coin_legacy::to_cosmwasm::<Nls>(tot_rewards2.into())],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards {
                address: lender1.clone(),
            },
        )
        .unwrap();

    assert_eq!(resp.rewards, lender_reward1.into());

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards {
                address: lender2.clone(),
            },
        )
        .unwrap();

    assert_eq!(resp.rewards, lender_reward2.into());

    // full withdraw, should send rewards to the lender
    () = test_case
        .app
        .execute(
            lender1.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::Burn {
                amount: deposit1.into(),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let balance = bank::balance::<_, Native>(&lender1, test_case.app.query()).unwrap();
    assert_eq!(balance, Coin::<Nls>::from(tot_rewards1 + lender_reward1));

    // lender account is removed
    let resp: Result<RewardsResponse, _> = test_case.app.query().query_wasm_smart(
        test_case.address_book.lpp().clone(),
        &LppQueryMsg::Rewards { address: lender1 },
    );

    assert!(resp.is_err());

    // claim rewards to other recipient
    () = test_case
        .app
        .execute(
            lender2.clone(),
            test_case.address_book.lpp().clone(),
            &LppExecuteMsg::ClaimRewards {
                other_recipient: Some(recipient.clone()),
            },
            &[],
        )
        .unwrap()
        .ignore_response()
        .unwrap_response();

    let resp: RewardsResponse = test_case
        .app
        .query()
        .query_wasm_smart(
            test_case.address_book.lpp().clone(),
            &LppQueryMsg::Rewards { address: lender2 },
        )
        .unwrap();

    assert_eq!(resp.rewards, Coin::new(0));
    let balance = bank::balance::<_, Native>(&recipient, test_case.app.query()).unwrap();
    assert_eq!(balance, Coin::<Nls>::from(lender_reward2));
}

fn lpn_cwcoin<A>(amount: A) -> CwCoin
where
    A: Into<Coin<Lpn>>,
{
    cwcoin(amount)
}

fn block_time(app: &App) -> Timestamp {
    app.block_info().time
}
