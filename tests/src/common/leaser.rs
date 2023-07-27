use currency::Currency;
use finance::{coin::Coin, duration::Duration, liability::Liability, percent::Percent, test};
use lease::api::InterestPaymentSpec;
use leaser::{
    contract::{execute, instantiate, query, reply, sudo},
    msg::{InstantiateMsg, QueryMsg, QuoteResponse},
};
use sdk::cosmwasm_std::{Addr, Uint64};

use super::{
    test_case::app::{App, Wasm as WasmTrait},
    CwContractWrapper, ADMIN,
};

pub(crate) struct Instantiator;

impl Instantiator {
    pub const INTEREST_RATE_MARGIN: Percent = Percent::from_permille(30);

    pub const REPAYMENT_PERIOD: Duration = Duration::from_days(90);

    pub const GRACE_PERIOD: Duration = Duration::from_days(10);

    pub fn liability() -> Liability {
        Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(5),
            Percent::from_percent(10),
            Percent::from_percent(2),
            Percent::from_percent(3),
            Percent::from_percent(2),
            Duration::from_hours(1),
        )
    }

    #[track_caller]
    pub fn instantiate<Wasm>(
        app: &mut App<Wasm>,
        lease_code_id: u64,
        lpp_addr: Addr,
        time_alarms: Addr,
        market_price_oracle: Addr,
        profit: Addr,
    ) -> Addr
    where
        Wasm: WasmTrait,
    {
        // TODO [Rust 1.70] Convert to static item with OnceCell
        let endpoints = CwContractWrapper::new(execute, instantiate, query)
            .with_reply(reply)
            .with_sudo(sudo);

        let code_id = app.store_code(Box::new(endpoints));

        let msg = InstantiateMsg {
            lease_code_id: Uint64::new(lease_code_id),
            lpp_ust_addr: lpp_addr,
            lease_interest_rate_margin: Self::INTEREST_RATE_MARGIN,
            liability: Self::liability(),
            lease_interest_payment: InterestPaymentSpec::new(
                Self::REPAYMENT_PERIOD,
                Self::GRACE_PERIOD,
            ),
            time_alarms,
            market_price_oracle,
            profit,
        };

        app.instantiate(code_id, Addr::unchecked(ADMIN), &msg, &[], "leaser", None)
            .unwrap()
            .unwrap_response()
    }
}

pub(crate) fn query_quote<Wasm, DownpaymentC, LeaseC>(
    app: &mut App<Wasm>,
    leaser: Addr,
    downpayment: Coin<DownpaymentC>,
    max_ltd: Option<Percent>,
) -> QuoteResponse
where
    Wasm: WasmTrait,
    DownpaymentC: Currency,
    LeaseC: Currency,
{
    app.query()
        .query_wasm_smart(
            leaser,
            &QueryMsg::Quote {
                downpayment: test::funds::<_, DownpaymentC>(downpayment.into()),
                lease_asset: LeaseC::TICKER.into(),
                max_ltd,
            },
        )
        .unwrap()
}
