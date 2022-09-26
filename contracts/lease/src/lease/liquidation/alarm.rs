use cosmwasm_std::Timestamp;
use serde::Serialize;

use finance::{
    coin::Coin,
    currency::{self, Currency},
    fraction::Fraction,
    percent::Percent,
    price::{dto::PriceDTO, total, total_of, Price},
};
use lpp::stub::lender::LppLender as LppLenderTrait;
use market_price_oracle::stub::Oracle as OracleTrait;
use marketprice::alarms::Alarm;
use platform::bank::BankAccountView;
use profit::stub::Profit as ProfitTrait;
use time_alarms::stub::TimeAlarms as TimeAlarmsTrait;

use crate::{
    error::ContractResult,
    lease::{Lease, LiquidationInfo, OnAlarmResult, Status, WarningLevel},
    loan::LiabilityStatus,
};

impl<'r, Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>
    Lease<'r, Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>
where
    Lpn: Currency + Serialize,
    Lpp: LppLenderTrait<Lpn>,
    TimeAlarms: TimeAlarmsTrait,
    Oracle: OracleTrait<Lpn>,
    Profit: ProfitTrait,
    Asset: Currency + Serialize,
{
    pub(crate) fn on_price_alarm<B>(
        self,
        now: Timestamp,
        account: &B,
    ) -> ContractResult<OnAlarmResult<Lpn, Asset>>
    where
        B: BankAccountView,
    {
        self.on_alarm(Self::act_on_liability, now, account)
    }

    pub(crate) fn on_time_alarm<B>(
        self,
        now: Timestamp,
        account: &B,
    ) -> ContractResult<OnAlarmResult<Lpn, Asset>>
    where
        B: BankAccountView,
    {
        self.on_alarm(Self::act_on_overdue, now, account)
    }

    #[inline]
    pub(in crate::lease) fn initial_alarm_schedule(
        &mut self,
        lease_amount: Coin<Asset>,
        now: &Timestamp,
    ) -> ContractResult<()> {
        self.reschedule(
            lease_amount,
            total(lease_amount, self.price_of_lease_currency()?),
            now,
            &Status::None,
        )
    }

    #[inline]
    pub(in crate::lease) fn reschedule_on_repay(
        &mut self,
        lease_amount: Coin<Asset>,
        now: &Timestamp,
    ) -> ContractResult<()> {
        let lease_lpn = total(lease_amount, self.price_of_lease_currency()?);

        self.reschedule(
            lease_amount,
            lease_lpn,
            now,
            &self.handle_warnings(
                self.loan
                    .liability_status(*now, self.lease_addr.clone(), lease_lpn)?
                    .ltv,
            ),
        )
    }

    fn on_alarm<F, B>(
        mut self,
        handler: F,
        now: Timestamp,
        account: &B,
    ) -> ContractResult<OnAlarmResult<Lpn, Asset>>
    where
        F: FnOnce(
            &mut Self,
            Coin<Lpn>,
            Timestamp,
            Percent,
            Coin<Lpn>,
        ) -> ContractResult<Status<Lpn, Asset>>,
        B: BankAccountView,
    {
        let mut lease_amount = account.balance::<Asset>()?;

        let price_to_lpn = self.price_of_lease_currency()?;

        let lease_lpn = total(lease_amount, price_to_lpn);

        let LiabilityStatus {
            ltv,
            total_lpn: liability_lpn,
            ..
        } = self
            .loan
            .liability_status(now, self.lease_addr.clone(), lease_lpn)?;

        let status = handler(&mut self, lease_lpn, now, ltv, liability_lpn)?;

        if let Status::PartialLiquidation {
            liquidation_info: LiquidationInfo { receipt, .. },
            ..
        } = &status
        {
            lease_amount -= total(receipt.total(), price_to_lpn.inv());
        }

        if !matches!(status, Status::FullLiquidation { .. }) {
            self.reschedule(lease_amount, lease_lpn, &now, &status)?;
        }

        Ok(self.into_on_alarm_result(status))
    }

    fn into_on_alarm_result(
        self,
        liquidation_status: Status<Lpn, Asset>,
    ) -> OnAlarmResult<Lpn, Asset> {
        let (lease_dto, batch) = self.into_dto();

        OnAlarmResult {
            batch,
            lease_dto,
            liquidation_status,
        }
    }

    #[inline]
    fn reschedule(
        &mut self,
        lease_amount: Coin<Asset>,
        lease_lpn: Coin<Lpn>,
        now: &Timestamp,
        liquidation_status: &Status<Lpn, Asset>,
    ) -> ContractResult<()> {
        self.reschedule_time_alarm(now, liquidation_status)?;

        self.reschedule_price_alarm(lease_amount, lease_lpn, now, liquidation_status)
    }

    fn reschedule_time_alarm(
        &mut self,
        now: &Timestamp,
        liquidation_status: &Status<Lpn, Asset>,
    ) -> ContractResult<()> {
        debug_assert!(!matches!(
            liquidation_status,
            Status::FullLiquidation { .. }
        ));

        self.time_alarms
            .add_alarm({
                self.loan
                    .grace_period_end()
                    .min(*now + self.liability.recalculation_time())
            })
            .map_err(Into::into)
    }

    fn reschedule_price_alarm(
        &mut self,
        lease_amount: Coin<Asset>,
        lease_lpn: Coin<Lpn>,
        now: &Timestamp,
        liquidation_status: &Status<Lpn, Asset>,
    ) -> ContractResult<()> {
        if currency::equal::<Asset, Lpn>() {
            return Ok(());
        }

        let (below, above) = match liquidation_status {
            Status::None | Status::PartialLiquidation { .. } => {
                (self.liability.first_liq_warn_percent(), None)
            }
            Status::Warning(_, WarningLevel::First) => (
                self.liability.second_liq_warn_percent(),
                Some(self.liability.first_liq_warn_percent()),
            ),
            Status::Warning(_, WarningLevel::Second) => (
                self.liability.third_liq_warn_percent(),
                Some(self.liability.second_liq_warn_percent()),
            ),
            Status::Warning(_, WarningLevel::Third) => (
                self.liability.max_percent(),
                Some(self.liability.third_liq_warn_percent()),
            ),
            Status::FullLiquidation { .. } => unreachable!(),
        };

        let total_liability = self
            .loan
            .liability_status(
                *now + self.liability.recalculation_time(),
                self.lease_addr.clone(),
                lease_lpn,
            )?
            .total_lpn;

        let below = self.price_alarm_by_percent(lease_amount, total_liability, below)?;

        let above = above
            .map(|above| self.price_alarm_by_percent(lease_amount, total_liability, above))
            .transpose()?;

        self.oracle
            .add_alarm(Alarm::new::<PriceDTO>(
                ToOwned::to_owned(Asset::SYMBOL),
                below.into(),
                above.map(Into::into),
            ))
            .map_err(Into::into)
    }

    fn price_alarm_by_percent(
        &self,
        lease_amount: Coin<Asset>,
        liability: Coin<Lpn>,
        percent: Percent,
    ) -> ContractResult<Price<Asset, Lpn>> {
        assert!(!lease_amount.is_zero(), "Loan already paid!");

        Ok(total_of(percent.of(lease_amount)).is(liability))
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{to_binary, Addr, Timestamp, WasmMsg};

    use finance::{duration::Duration, percent::Percent, price::total_of};
    use lpp::msg::LoanResponse;
    use platform::batch::Batch;
    use time_alarms::msg::ExecuteMsg::AddAlarm;

    use crate::lease::{
        tests::{coin, lease_setup},
        LeaseInfo, Status, WarningLevel,
    };

    #[test]
    fn reschedule_time_alarm_recalc() {
        let interest_rate = Percent::from_permille(50);
        // LPP loan
        let loan = LoanResponse {
            principal_due: coin(300),
            interest_due: coin(0),
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let lease_addr = Addr::unchecked("lease");
        let mut lease = lease_setup(
            &lease_addr,
            Some(loan),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
        );

        lease
            .reschedule_time_alarm(
                &(lease.loan.grace_period_end()
                    - lease.liability.recalculation_time()
                    - lease.liability.recalculation_time()),
                &Status::Warning(
                    LeaseInfo::new(Addr::unchecked(String::new()), Default::default()),
                    WarningLevel::Second,
                ),
            )
            .unwrap();

        assert_eq!(lease.time_alarms.batch, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: String::new(),
                msg: to_binary(&AddAlarm {
                    time: lease.loan.grace_period_end() - lease.liability.recalculation_time(),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    #[test]
    fn reschedule_time_alarm_liquidation() {
        let interest_rate = Percent::from_permille(50);
        // LPP loan
        let loan = LoanResponse {
            principal_due: coin(300),
            interest_due: coin(0),
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let lease_addr = Addr::unchecked("lease");
        let mut lease = lease_setup(
            &lease_addr,
            Some(loan),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
        );

        lease
            .reschedule_time_alarm(
                &(lease.loan.grace_period_end() - lease.liability.recalculation_time()
                    + Duration::from_nanos(1)),
                &Status::Warning(
                    LeaseInfo::new(Addr::unchecked(String::new()), Default::default()),
                    WarningLevel::Second,
                ),
            )
            .unwrap();

        assert_eq!(lease.time_alarms.batch, {
            let mut batch = Batch::default();

            batch.schedule_execute_no_reply(WasmMsg::Execute {
                contract_addr: String::new(),
                msg: to_binary(&AddAlarm {
                    time: lease.loan.grace_period_end(),
                })
                .unwrap(),
                funds: vec![],
            });

            batch
        });
    }

    #[test]
    #[ignore = "No support for same currency prices. Without Price's debug assertion, runs successfully."]
    fn price_alarm_by_percent() {
        let lease_amount = 1000;
        let interest_rate = Percent::from_permille(50);
        // LPP loan
        let loan = LoanResponse {
            principal_due: coin(300),
            interest_due: coin(0),
            annual_interest_rate: interest_rate,
            interest_paid: Timestamp::from_nanos(0),
        };

        let lease_addr = Addr::unchecked("lease");
        let lease = lease_setup(
            &lease_addr,
            Some(loan),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
            Addr::unchecked(String::new()),
        );

        assert_eq!(
            lease
                .price_alarm_by_percent(coin(lease_amount), coin(500), Percent::from_percent(50))
                .unwrap(),
            total_of(coin(5)).is(coin(3))
        );
    }
}
