use currencies::{LeaseC2, LeaseGroup, Lpn, PaymentC1, PaymentGroup};
use currency::{Currency, CurrencyDef, MemberOf};
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    interest,
    percent::Percent,
    price::{self, Price},
};
use lease::api::query::{StateQuery, StateResponse};
use leaser::msg::QuoteResponse;
use sdk::cosmwasm_std::{coin, Addr, Timestamp};

use crate::common::{
    self, cwcoin, cwcoin_dex,
    leaser::{self as leaser_mod, Instantiator as LeaserInstantiator},
    protocols::Registry,
    test_case::{builder::Builder as TestCaseBuilder, response::RemoteChain, TestCase},
    ADDON_OPTIMAL_INTEREST_RATE, ADMIN, BASE_INTEREST_RATE, USER, UTILIZATION_OPTIMAL,
};

mod close;
mod close_position;
mod compare_with_lpp;
mod heal;
mod liquidation;
mod open;
mod repay;

type LpnCurrency = Lpn;
type LpnCoin = Coin<LpnCurrency>;

type LeaseCurrency = LeaseC2;
type LeaseCoin = Coin<LeaseCurrency>;

type PaymentCurrency = PaymentC1;
type PaymentCoin = Coin<PaymentCurrency>;

const DOWNPAYMENT: PaymentCoin = PaymentCoin::new(1_000_000_000_000);

pub(super) type LeaseTestCase = TestCase<Addr, Addr, Addr, Addr, Addr, Addr, Addr, Addr>;

pub(super) fn create_payment_coin(amount: Amount) -> PaymentCoin {
    PaymentCoin::new(amount)
}

pub(super) fn price_lpn_of<C>() -> Price<C, LpnCurrency>
where
    C: Currency,
{
    Price::identity()
}

pub(super) fn feed_price<ProtocolsRegistry, Treasury, Profit, Reserve, Leaser, Lpp, TimeAlarms>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Addr,
        TimeAlarms,
    >,
) {
    let lease_price = price_lpn_of::<LeaseCurrency>();
    common::oracle::feed_price_pair(test_case, Addr::unchecked(ADMIN), lease_price);

    let payment_price = price_lpn_of::<PaymentCurrency>();
    common::oracle::feed_price_pair(test_case, Addr::unchecked(ADMIN), payment_price);
}

pub(super) fn create_test_case<InitFundsC>() -> LeaseTestCase
where
    InitFundsC: CurrencyDef,
{
    let mut test_case = TestCaseBuilder::<LpnCurrency, _, _, _, _, _, _, _, _>::with_reserve(&[
        cwcoin::<PaymentCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin_dex::<PaymentCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin::<LpnCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin_dex::<LpnCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin::<LeaseCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin_dex::<LeaseCurrency, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin::<InitFundsC, _>(10_000_000_000_000_000_000_000_000_000),
        cwcoin_dex::<InitFundsC, _>(10_000_000_000_000_000_000_000_000_000),
    ])
    .init_lpp_with_funds(
        None,
        &[coin(
            5_000_000_000_000_000_000_000_000_000,
            LpnCurrency::bank(),
        )],
        BASE_INTEREST_RATE,
        UTILIZATION_OPTIMAL,
        ADDON_OPTIMAL_INTEREST_RATE,
        TestCase::DEFAULT_LPP_MIN_UTILIZATION,
    )
    .init_time_alarms()
    .init_protocols_registry(Registry::SingleProtocol)
    .init_oracle(None)
    .init_treasury()
    .init_profit(24)
    .init_reserve()
    .init_leaser()
    .into_generic();

    test_case.send_funds_from_admin(
        Addr::unchecked(USER),
        &[cwcoin::<InitFundsC, _>(1_000_000_000_000_000_000_000_000)],
    );

    common::oracle::add_feeder(&mut test_case, ADMIN);

    feed_price(&mut test_case);

    test_case
}

pub(super) fn calculate_interest(
    principal: Coin<LpnCurrency>,
    interest_rate: Percent,
    duration: Duration,
) -> Coin<LpnCurrency> {
    interest::interest(interest_rate, principal, duration).unwrap()
}

pub(super) fn open_lease<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    DownpaymentC,
>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<DownpaymentC>,
    max_ltd: Option<Percent>,
) -> Addr
where
    DownpaymentC: CurrencyDef,
    DownpaymentC::Group: MemberOf<PaymentGroup>,
{
    let lease = try_init_lease(test_case, downpayment, max_ltd);
    complete_init_lease(test_case, downpayment, max_ltd, &lease);
    lease
}

pub(super) fn try_init_lease<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    D,
>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<D>,
    max_ltd: Option<Percent>,
) -> Addr
where
    D: CurrencyDef,
{
    let downpayment = (!downpayment.is_zero()).then(|| cwcoin::<D, _>(downpayment));

    let mut response = test_case
        .app
        .execute(
            Addr::unchecked(USER),
            test_case.address_book.leaser().clone(),
            &leaser::msg::ExecuteMsg::OpenLease {
                currency: currency::dto::<LeaseCurrency, _>(),
                max_ltd,
            },
            downpayment.as_ref().map_or(&[], std::slice::from_ref),
        )
        .unwrap();

    response.expect_register_ica(TestCase::DEX_CONNECTION_ID, TestCase::LEASE_ICA_ID);
    () = response.ignore_response().unwrap_response();

    leaser_mod::expect_a_lease(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        Addr::unchecked(USER),
    )
}

pub(super) fn complete_init_lease<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    DownpaymentC,
>(
    test_case: &mut TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<DownpaymentC>,
    max_ltd: Option<Percent>,
    lease: &Addr,
) where
    DownpaymentC: CurrencyDef,
    DownpaymentC::Group: MemberOf<PaymentGroup>,
{
    let quote: QuoteResponse = common::leaser::query_quote::<DownpaymentC, LeaseCurrency>(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        downpayment,
        max_ltd,
    );
    let exp_borrow: LpnCoin = quote.borrow.try_into().unwrap();

    common::lease::complete_initialization(
        &mut test_case.app,
        TestCase::DEX_CONNECTION_ID,
        lease.clone(),
        downpayment,
        exp_borrow,
    );
}

pub(super) fn quote_borrow<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: PaymentCoin,
) -> LpnCoin {
    LpnCoin::try_from(quote_query(test_case, downpayment).borrow).unwrap()
}

pub(super) fn quote_query<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    DownpaymentC,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<DownpaymentC>,
) -> QuoteResponse
where
    DownpaymentC: CurrencyDef,
    DownpaymentC::Group: MemberOf<PaymentGroup>,
{
    common::leaser::query_quote::<_, LeaseCurrency>(
        &test_case.app,
        test_case.address_book.leaser().clone(),
        downpayment,
        None,
    )
}

pub(super) fn state_query<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Leaser,
    Lpp,
    Oracle,
    TimeAlarms,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    contract_addr: &str,
) -> StateResponse {
    test_case
        .app
        .query()
        .query_wasm_smart(contract_addr, &StateQuery {})
        .unwrap()
}

pub(super) fn expected_open_state<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    DownpaymentC,
    PaymentC,
    AssetC,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<DownpaymentC>,
    payments: Coin<PaymentC>,
    closed: Coin<AssetC>,
    max_due: Duration,
) -> StateResponse
where
    DownpaymentC: CurrencyDef,
    DownpaymentC::Group: MemberOf<PaymentGroup>,
    PaymentC: Currency + MemberOf<PaymentGroup>,
    AssetC: CurrencyDef,
    AssetC::Group: MemberOf<LeaseGroup>,
{
    let now = test_case.app.block_info().time;
    let last_paid = now;
    let quote_result = quote_query(test_case, downpayment);
    let total: Coin<AssetC> = Coin::<AssetC>::try_from(quote_result.total).unwrap();
    let total_lpn: LpnCoin = price::total(total, price_lpn_of::<AssetC>()).unwrap();
    let expected_principal: LpnCoin = total_lpn
        - price::total(downpayment, price_lpn_of::<DownpaymentC>()).unwrap()
        - price::total(payments, price_lpn_of::<PaymentC>()).unwrap();
    let due_period_start = (now - max_due).max(last_paid);
    let (overdue, due) = (
        Duration::between(&last_paid, &due_period_start),
        Duration::between(&due_period_start, &now),
    );
    StateResponse::Opened {
        amount: (total - closed).into(),
        loan_interest_rate: quote_result.annual_interest_rate,
        margin_interest_rate: quote_result.annual_interest_rate_margin,
        principal_due: expected_principal.into(),
        overdue_margin: calculate_interest(
            expected_principal,
            quote_result.annual_interest_rate_margin,
            overdue,
        )
        .into(),
        overdue_interest: calculate_interest(
            expected_principal,
            quote_result.annual_interest_rate,
            overdue,
        )
        .into(),
        overdue_collect_in: if overdue == Duration::default() {
            Duration::between(&(now - max_due), &last_paid)
        } else {
            Duration::default()
        },
        due_margin: calculate_interest(
            expected_principal,
            quote_result.annual_interest_rate_margin,
            due,
        )
        .into(),
        due_interest: calculate_interest(
            expected_principal,
            quote_result.annual_interest_rate,
            due,
        )
        .into(),
        validity: block_time(test_case),
        in_progress: None,
    }
}

pub(super) fn expected_newly_opened_state<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Lpp,
    Oracle,
    TimeAlarms,
    DownpaymentC,
    PaymentC,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Addr,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
    downpayment: Coin<DownpaymentC>,
    payments: Coin<PaymentC>,
) -> StateResponse
where
    DownpaymentC: CurrencyDef,
    DownpaymentC::Group: MemberOf<PaymentGroup>,
    PaymentC: Currency + MemberOf<PaymentGroup>,
{
    expected_open_state(
        test_case,
        downpayment,
        payments,
        Coin::<LeaseCurrency>::default(),
        LeaserInstantiator::REPAYMENT_PERIOD,
    )
}

pub(super) fn block_time<
    ProtocolsRegistry,
    Treasury,
    Profit,
    Reserve,
    Leaser,
    Lpp,
    Oracle,
    TimeAlarms,
>(
    test_case: &TestCase<
        ProtocolsRegistry,
        Treasury,
        Profit,
        Reserve,
        Leaser,
        Lpp,
        Oracle,
        TimeAlarms,
    >,
) -> Timestamp {
    test_case.app.block_info().time
}
