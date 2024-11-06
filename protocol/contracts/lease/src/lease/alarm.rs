use currency::{CurrencyDef, MemberOf};
use finance::{duration::Duration, liability::Zone};
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle::{
    api::alarms::Alarm,
    stub::{AsAlarms, PriceAlarms as PriceAlarmsTrait},
};
use oracle_platform::Oracle as OracleTrait;
use platform::batch::Batch;
use sdk::cosmwasm_std::Timestamp;
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{LeaseAssetCurrencies, LeasePaymentCurrencies},
    error::ContractResult,
    finance::{LpnCoin, LpnCurrencies, LpnCurrency, OracleRef},
    lease::Lease,
};

impl<Asset, Lpp, Oracle> Lease<Asset, Lpp, Oracle>
where
    Lpp: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>,
    Asset: CurrencyDef,
    Asset::Group: MemberOf<LeaseAssetCurrencies>,
{
    pub(super) fn reschedule(
        &self,
        now: &Timestamp,
        recheck_in: Duration,
        liquidation_zone: &Zone,
        total_due: LpnCoin,
        time_alarms: &TimeAlarmsRef,
        price_alarms: &OracleRef,
    ) -> ContractResult<Batch> {
        let next_recheck = now + recheck_in;

        time_alarms
            .setup_alarm(next_recheck)
            .map_err(Into::into)
            .and_then(|schedule_time_alarm| {
                self.reschedule_price_alarm(
                    liquidation_zone,
                    total_due,
                    price_alarms.as_alarms::<LeaseAssetCurrencies>(),
                )
                .map(|schedule_price_alarm| schedule_time_alarm.merge(schedule_price_alarm))
            })
    }

    fn reschedule_price_alarm<PriceAlarms>(
        &self,
        liquidation_zone: &Zone,
        total_due: LpnCoin,
        price_alarms: PriceAlarms,
    ) -> ContractResult<Batch>
    where
        PriceAlarms:
            PriceAlarmsTrait<LeaseAssetCurrencies, BaseC = LpnCurrency, BaseG = LpnCurrencies>,
    {
        debug_assert!(!currency::equal::<LpnCurrency, Asset>());
        debug_assert!(!total_due.is_zero());

        let below = self.position.price_at(liquidation_zone.high(), total_due)?;

        let above_or_equal = liquidation_zone
            .low()
            .map(|low| self.position.price_at(low, total_due))
            .transpose()?;

        price_alarms
            .add_alarm(Alarm::<LeaseAssetCurrencies, _, _>::new(
                below,
                above_or_equal,
            ))
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use currencies::{LeaseGroup, Lpns};
    use finance::{
        coin::Coin,
        duration::Duration,
        fraction::Fraction,
        liability::Zone,
        percent::Percent,
        price::{self, total_of, Price},
    };
    use lpp::msg::LoanResponse;
    use oracle::{api::alarms::Alarm, api::alarms::ExecuteMsg::AddPriceAlarm};
    use oracle_platform::OracleRef;
    use platform::batch::Batch;
    use sdk::cosmwasm_std::{to_json_binary, Addr, WasmMsg};
    use timealarms::{msg::ExecuteMsg::AddAlarm, stub::TimeAlarmsRef};

    use crate::{
        lease::{
            self,
            tests::{
                loan, open_lease, TestLpn, FIRST_LIQ_WARN, LEASE_START, RECHECK_TIME,
                SECOND_LIQ_WARN, THIRD_LIQ_WARN,
            },
        },
        position::DueTrait,
    };

    const TIME_ALARMS_ADDR: &str = "timealarms";
    const ORACLE_ADDR: &str = "oracle";

    #[test]
    fn initial_alarms() {
        let asset = Coin::from(10);
        let lease = lease::tests::open_lease(asset, loan());
        let now = LEASE_START;
        let recheck_time = now + RECHECK_TIME;
        let liability_alarm_on = FIRST_LIQ_WARN;
        let due = {
            let lease = &lease;
            lease.loan.state(&now)
        };
        let alarm_msgs = lease
            .reschedule(
                &LEASE_START,
                RECHECK_TIME,
                &Zone::no_warnings(liability_alarm_on),
                due.total_due(),
                &timealarms(),
                &pricealarms(),
            )
            .unwrap();

        assert_eq!(alarm_msgs, {
            let below_alarm = total_of(liability_alarm_on.of(asset)).is(due.total_due());

            Batch::default()
                .schedule_execute_no_reply(WasmMsg::Execute {
                    contract_addr: TIME_ALARMS_ADDR.into(),
                    msg: to_json_binary(&AddAlarm { time: recheck_time }).unwrap(),
                    funds: vec![],
                })
                .schedule_execute_no_reply(WasmMsg::Execute {
                    contract_addr: ORACLE_ADDR.into(),
                    msg: to_json_binary(&AddPriceAlarm::<LeaseGroup, TestLpn, Lpns> {
                        alarm: Alarm::new(below_alarm, None),
                    })
                    .unwrap(),
                    funds: vec![],
                })
        });
    }

    #[test]
    fn third_zone_alarms() {
        let principal = 300.into();
        let interest_rate = Percent::from_permille(145);

        let loan = LoanResponse {
            principal_due: principal,
            annual_interest_rate: interest_rate,
            interest_paid: LEASE_START,
        };

        let lease_amount = 1000.into();
        let lease = open_lease(lease_amount, loan);

        let reschedule_at = LEASE_START + Duration::from_days(50);
        let recalc_at = reschedule_at + RECHECK_TIME;

        let zone = Zone::second(SECOND_LIQ_WARN, THIRD_LIQ_WARN);
        let total_due = price::total(
            (SECOND_LIQ_WARN + Percent::from_percent(1)).of(lease_amount),
            Price::identity(),
        );
        let alarm_msgs = lease
            .reschedule(
                &reschedule_at,
                RECHECK_TIME,
                &zone,
                total_due,
                &timealarms(),
                &pricealarms(),
            )
            .unwrap();

        let exp_below = total_of(zone.high().ltv().of(lease_amount)).is(total_due);
        let exp_above = total_of(zone.low().unwrap().ltv().of(lease_amount)).is(total_due);

        assert_eq!(alarm_msgs, {
            Batch::default()
                .schedule_execute_no_reply(WasmMsg::Execute {
                    contract_addr: TIME_ALARMS_ADDR.into(),
                    msg: to_json_binary(&AddAlarm { time: recalc_at }).unwrap(),
                    funds: vec![],
                })
                .schedule_execute_no_reply(WasmMsg::Execute {
                    contract_addr: ORACLE_ADDR.into(),
                    msg: to_json_binary(&AddPriceAlarm::<LeaseGroup, TestLpn, Lpns> {
                        alarm: Alarm::new(exp_below, Some(exp_above)),
                    })
                    .unwrap(),
                    funds: vec![],
                })
        });
    }

    fn timealarms() -> TimeAlarmsRef {
        TimeAlarmsRef::unchecked(TIME_ALARMS_ADDR)
    }

    fn pricealarms() -> OracleRef<TestLpn, Lpns> {
        OracleRef::unchecked(Addr::unchecked(ORACLE_ADDR))
    }
}
