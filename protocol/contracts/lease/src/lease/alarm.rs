use currency::Currency;
use finance::liability::Zone;
use lpp::stub::loan::LppLoan as LppLoanTrait;
use marketprice::SpotPrice;
use oracle::{
    alarms::Alarm,
    stub::{AsAlarms, PriceAlarms as PriceAlarmsTrait},
};
use oracle_platform::{Oracle as OracleTrait, OracleRef};
use platform::batch::Batch;
use sdk::cosmwasm_std::Timestamp;
use timealarms::stub::TimeAlarmsRef;

use crate::{error::ContractResult, lease::Lease};

impl<Lpn, Asset, Lpp, Oracle> Lease<Lpn, Asset, Lpp, Oracle>
where
    Lpn: Currency,
    Lpp: LppLoanTrait<Lpn>,
    Oracle: OracleTrait<Lpn>,
    Asset: Currency,
{
    pub(crate) fn reschedule(
        &self,
        now: &Timestamp,
        liquidation_zone: &Zone,
        time_alarms: &TimeAlarmsRef,
        price_alarms: &OracleRef,
    ) -> ContractResult<Batch> {
        self.reschedule_time_alarm(now, time_alarms)
            .and_then(|schedule_time_alarm| {
                let mut price_alarms = price_alarms.as_alarms::<Lpn>();
                self.reschedule_price_alarm(now, liquidation_zone, &mut price_alarms)
                    .map(|_| schedule_time_alarm.merge(price_alarms.into()))
            })
    }

    fn reschedule_time_alarm(
        &self,
        now: &Timestamp,
        time_alarms: &TimeAlarmsRef,
    ) -> ContractResult<Batch> {
        let grace_period_end = self.loan.next_grace_period_end(now);
        debug_assert!(
            now < &grace_period_end,
            "Rescheduling when the lease is in overdue! A liquidation is expected!"
        );
        let recalculation_time = self.recalc_time(now);
        time_alarms
            .setup_alarm(grace_period_end.min(recalculation_time))
            .map_err(Into::into)
    }

    fn reschedule_price_alarm<PriceAlarms>(
        &self,
        now: &Timestamp,
        liquidation_zone: &Zone,
        price_alarms: &mut PriceAlarms,
    ) -> ContractResult<()>
    where
        PriceAlarms: PriceAlarmsTrait,
    {
        debug_assert!(!currency::equal::<Lpn, Asset>());
        let recalculation_time = self.recalc_time(now);
        let total_liability = self.loan.liability_status(recalculation_time).total;
        debug_assert!(!total_liability.is_zero());

        let below = liquidation_zone
            .high()
            .price_alarm(self.position.amount(), total_liability)?;

        let above_or_equal = liquidation_zone
            .low()
            .map(|low| low.price_alarm(self.position.amount(), total_liability))
            .transpose()?;

        price_alarms
            .add_alarm(Alarm::new(
                below.into(),
                above_or_equal.map(Into::<SpotPrice>::into),
            ))
            .map_err(Into::into)
    }

    fn recalc_time(&self, now: &Timestamp) -> Timestamp {
        *now + self.position.liability().recalculation_time()
    }
}

#[cfg(test)]
mod tests {
    use finance::{
        coin::Coin, duration::Duration, fraction::Fraction, liability::Zone, percent::Percent,
        price::total_of,
    };
    use lpp::msg::LoanResponse;
    use marketprice::SpotPrice;
    use oracle::{alarms::Alarm, msg::ExecuteMsg::AddPriceAlarm};
    use oracle_platform::OracleRef;
    use platform::batch::Batch;
    use sdk::cosmwasm_std::{to_json_binary, Timestamp, WasmMsg};
    use timealarms::{msg::ExecuteMsg::AddAlarm, stub::TimeAlarmsRef};

    use crate::{
        api::InterestPaymentSpec,
        lease::{
            self,
            tests::{
                loan, open_lease, open_lease_with_payment_spec, TestLease, TestLpn, LEASE_START,
                RECALC_TIME,
            },
        },
    };

    const TIME_ALARMS_ADDR: &str = "timealarms";
    const ORACLE_ADDR: &str = "oracle";

    #[test]
    fn initial_alarm_schedule() {
        let asset = Coin::from(10);
        let lease = lease::tests::open_lease(asset, loan());
        let recalc_time = LEASE_START + lease.position.liability().recalculation_time();
        let liability_alarm_on = lease.position.liability().first_liq_warn();
        let projected_liability = projected_liability(&lease, recalc_time);
        let alarm_msgs = lease
            .reschedule(
                &LEASE_START,
                &Zone::no_warnings(liability_alarm_on),
                &timealarms(),
                &pricealarms(),
            )
            .unwrap();

        assert_eq!(alarm_msgs, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: TIME_ALARMS_ADDR.into(),
                msg: to_json_binary(&AddAlarm { time: recalc_time }).unwrap(),
                funds: vec![],
            });

            let below_alarm: SpotPrice = total_of(liability_alarm_on.of(asset))
                .is(projected_liability)
                .into();
            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: ORACLE_ADDR.into(),
                msg: to_json_binary(&AddPriceAlarm {
                    alarm: Alarm::new(below_alarm, None),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    #[test]
    fn reschedule_time_alarm_recalc() {
        let lease_amount = 20.into();
        let lease = open_lease(lease_amount, loan());
        let now = lease.loan.grace_period_end()
            - lease.position.liability().recalculation_time()
            - lease.position.liability().recalculation_time();
        let recalc_time = now + lease.position.liability().recalculation_time();
        let up_to = lease.position.liability().first_liq_warn();
        let no_warnings = Zone::no_warnings(up_to);
        let alarm_msgs = lease
            .reschedule(&now, &no_warnings, &timealarms(), &pricealarms())
            .unwrap();
        let exp_below: SpotPrice = total_of(no_warnings.high().ltv().of(lease_amount))
            .is(projected_liability(&lease, recalc_time))
            .into();

        assert_eq!(alarm_msgs, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: TIME_ALARMS_ADDR.into(),
                msg: to_json_binary(&AddAlarm { time: recalc_time }).unwrap(),
                funds: vec![],
            });

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: ORACLE_ADDR.into(),
                msg: to_json_binary(&AddPriceAlarm {
                    alarm: Alarm::new(exp_below, None),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    #[test]
    fn reschedule_time_alarm_liquidation() {
        test_setup_alarm_grace_period_end(
            RECALC_TIME - Duration::from_nanos(1),
            Duration::from_nanos(0),
        );

        test_setup_alarm_grace_period_end(Duration::from_nanos(1), Duration::from_nanos(0));

        test_setup_alarm_grace_period_end(Duration::from_nanos(0), RECALC_TIME);
    }

    #[test]
    fn reschedule_time_alarm_past_grace_period() {
        let lease_amount = 300.into();
        let due_period = Duration::from_nanos(RECALC_TIME.nanos() / 5);
        let grace_period = Duration::from_nanos(due_period.nanos() / 2);
        let interest_spec = InterestPaymentSpec::new(due_period, grace_period);
        let lease = open_lease_with_payment_spec(lease_amount, loan(), interest_spec);

        let now = lease.loan.grace_period_end() + due_period + due_period + Duration::from_nanos(1);
        let recalc_time = now + lease.position.liability().recalculation_time();
        let exp_alarm_at = lease.loan.grace_period_end() + due_period + due_period + due_period;
        let up_to = lease.position.liability().first_liq_warn();
        let no_warnings = Zone::no_warnings(up_to);
        let alarm_msgs = lease
            .reschedule(&now, &no_warnings, &timealarms(), &pricealarms())
            .unwrap();
        let exp_below: SpotPrice = total_of(no_warnings.high().ltv().of(lease_amount))
            .is(projected_liability(&lease, recalc_time))
            .into();

        assert_eq!(alarm_msgs, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: TIME_ALARMS_ADDR.into(),
                msg: to_json_binary(&AddAlarm { time: exp_alarm_at }).unwrap(),
                funds: vec![],
            });

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: ORACLE_ADDR.into(),
                msg: to_json_binary(&AddPriceAlarm {
                    alarm: Alarm::new(exp_below, None),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    #[test]
    fn price_alarm_by_percent() {
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
        let recalc_at = reschedule_at + lease.position.liability().recalculation_time();
        let projected_liability = projected_liability(&lease, recalc_at);

        let zone = Zone::second(
            lease.position.liability().second_liq_warn(),
            lease.position.liability().third_liq_warn(),
        );
        let alarm_msgs = lease
            .reschedule(&reschedule_at, &zone, &timealarms(), &pricealarms())
            .unwrap();

        let exp_below: SpotPrice = total_of(zone.high().ltv().of(lease_amount))
            .is(projected_liability)
            .into();
        let exp_above: SpotPrice = total_of(zone.low().unwrap().ltv().of(lease_amount))
            .is(projected_liability)
            .into();

        assert_eq!(alarm_msgs, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: TIME_ALARMS_ADDR.into(),
                msg: to_json_binary(&AddAlarm { time: recalc_at }).unwrap(),
                funds: vec![],
            });

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: ORACLE_ADDR.into(),
                msg: to_json_binary(&AddPriceAlarm {
                    alarm: Alarm::new(exp_below, Some(exp_above)),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    fn timealarms() -> TimeAlarmsRef {
        TimeAlarmsRef::unchecked(TIME_ALARMS_ADDR)
    }

    fn pricealarms() -> OracleRef {
        OracleRef::unchecked::<_, TestLpn>(ORACLE_ADDR)
    }

    fn projected_liability(lease: &TestLease, at: Timestamp) -> Coin<TestLpn> {
        let l = lease.loan.state(at);
        l.principal_due
            + l.previous_interest_due
            + l.previous_margin_interest_due
            + l.current_interest_due
            + l.current_margin_interest_due
    }

    fn test_setup_alarm_grace_period_end(
        offset_from_this: Duration,
        exp_alarm_past_this: Duration,
    ) {
        let lease_amount = 300.into();
        let lease = open_lease(lease_amount, loan());

        let now = lease.loan.grace_period_end() - offset_from_this;
        let recalc_time = now + lease.position.liability().recalculation_time();
        let exp_alarm_at = lease.loan.grace_period_end() + exp_alarm_past_this;
        let up_to = lease.position.liability().first_liq_warn();
        let no_warnings = Zone::no_warnings(up_to);
        let alarm_msgs = lease
            .reschedule(&now, &no_warnings, &timealarms(), &pricealarms())
            .unwrap();
        let exp_below: SpotPrice = total_of(no_warnings.high().ltv().of(lease_amount))
            .is(projected_liability(&lease, recalc_time))
            .into();

        assert_eq!(alarm_msgs, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: TIME_ALARMS_ADDR.into(),
                msg: to_json_binary(&AddAlarm { time: exp_alarm_at }).unwrap(),
                funds: vec![],
            });

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: ORACLE_ADDR.into(),
                msg: to_json_binary(&AddPriceAlarm {
                    alarm: Alarm::new(exp_below, None),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }
}
