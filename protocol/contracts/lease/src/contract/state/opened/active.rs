use serde::{Deserialize, Serialize};

use currency::never;
use dex::Enterable;
use finance::{coin::IntoDTO, duration::Duration};
use platform::{bank, batch::Emitter, message::Response as MessageResponse};
use sdk::cosmwasm_std::{Coin as CwCoin, Env, MessageInfo, QuerierWrapper, Timestamp};

use crate::{
    api::{
        position::{ClosePolicyChange, PositionClose},
        query::StateResponse,
        DownpaymentCoin,
    },
    contract::{
        cmd::{
            ChangeClosePolicy, CloseStatusCmd, CloseStatusDTO, ObtainPayment, OpenLoanRespResult,
        },
        state::{Handler, Response},
        Lease,
    },
    error::{ContractError, ContractResult},
    finance::LpnCurrencies,
};

use super::{
    alarm, balance,
    close::{customer_close, liquidation},
    event,
    repay::{
        self,
        buy_lpn::{self, DexState as BuyLpnState},
    },
};

#[derive(Serialize, Deserialize)]
pub struct Active {
    lease: Lease,
}

impl Active {
    pub(in super::super) fn new(lease: Lease) -> Self {
        Self { lease }
    }

    pub(in super::super) fn emit_opened(
        &self,
        env: &Env,
        downpayment: DownpaymentCoin,
        loan: OpenLoanRespResult,
    ) -> Emitter {
        event::emit_lease_opened(env, &self.lease.lease, loan, downpayment)
    }

    fn try_repay(
        self,
        querier: QuerierWrapper<'_>,
        env: &Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        let may_lpn_payment = bank::may_received(&info.funds, IntoDTO::<LpnCurrencies>::new());
        match may_lpn_payment {
            Some(lpn_payment) => {
                let payment = never::safe_unwrap(lpn_payment);
                debug_assert_eq!(payment.currency(), self.lease.lease.loan.lpp().lpn());
                repay::repay(self.lease, payment, env, querier)
            }
            None => self.start_swap(info.funds, env.block.time, querier),
        }
    }

    fn try_on_price_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: &Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        // TODO ref. the TODO in try_on_time_alarm
        if !self.lease.lease.oracle.owned_by(&info.sender) {
            return Err(ContractError::Unauthorized(
                access_control::error::Error::Unauthorized {},
            ));
        }

        self.try_on_alarm(querier, env)
    }

    fn try_on_time_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: &Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        // TODO define a trait 'RestrictedResource' with 'fn owner(&Addr) -> bool'
        // and move this check to the 'access_control' package
        if !self.lease.lease.time_alarms.owned_by(&info.sender) {
            return Err(ContractError::Unauthorized(
                access_control::error::Error::Unauthorized {},
            ));
        }

        self.try_on_alarm(querier, env)
    }

    fn try_on_alarm(self, querier: QuerierWrapper<'_>, env: &Env) -> ContractResult<Response> {
        let time_alarms_ref = self.lease.lease.time_alarms.clone();
        let oracle_ref = self.lease.lease.oracle.clone();
        let close_status = self.lease.lease.clone().execute(
            CloseStatusCmd::new(&env.block.time, &time_alarms_ref, &oracle_ref),
            querier,
        )?;

        match close_status {
            CloseStatusDTO::Paid => {
                unimplemented!("an Active Opened Lease should always have some due amount")
            }
            CloseStatusDTO::None {
                current_liability,
                alarms,
            } => Ok(Response::from(
                alarm::build_resp(&self.lease, current_liability, alarms),
                self,
            )),
            CloseStatusDTO::NeedLiquidation(liquidation) => liquidation::start(
                self.lease,
                liquidation,
                MessageResponse::default(),
                env,
                querier,
            ),
            CloseStatusDTO::CloseAsked(strategy) => {
                customer_close::auto_start(strategy, self.lease, env, querier)
            }
        }
    }

    fn start_swap(
        self,
        cw_amount: Vec<CwCoin>,
        now: Timestamp,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<Response> {
        self.lease
            .lease
            .clone()
            .execute(ObtainPayment::new(cw_amount), querier)
            .and_then(|payment| {
                let buy_lpn = buy_lpn::start(self.lease, payment);
                buy_lpn
                    .enter(now, querier)
                    .map(|batch| Response::from(batch, BuyLpnState::from(buy_lpn)))
                    .map_err(Into::into)
            })
    }
}

impl From<Active> for Lease {
    fn from(value: Active) -> Self {
        value.lease
    }
}

impl Handler for Active {
    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<StateResponse> {
        super::lease_state(self.lease, None, now, due_projection, querier)
    }

    fn repay(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.try_repay(querier, &env, info)
    }

    fn change_close_policy(
        self,
        change: ClosePolicyChange,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        access_control::check(&self.lease.lease.customer, &info.sender)
            .map_err(Into::into)
            .and_then(|()| {
                let profit = self.lease.lease.loan.profit().clone();
                let time_alarms = self.lease.lease.time_alarms.clone();
                let oracle_ref = self.lease.lease.oracle.clone();
                let reserve = self.lease.lease.reserve.clone();
                self.lease
                    .update(
                        ChangeClosePolicy::new(
                            change,
                            &env.block.time,
                            profit,
                            time_alarms,
                            &oracle_ref,
                            reserve,
                        ),
                        querier,
                    )
                    .map(|(lease, batch)| Response::from(batch, Self::new(lease)))
            })
    }

    fn close_position(
        self,
        spec: PositionClose,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        access_control::check(&self.lease.lease.customer, &info.sender)
            .map_err(Into::into)
            .and_then(|()| customer_close::start(spec, self.lease, &env, querier))
    }

    fn on_time_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.try_on_time_alarm(querier, &env, info)
    }

    fn on_price_alarm(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        info: MessageInfo,
    ) -> ContractResult<Response> {
        self.try_on_price_alarm(querier, &env, info)
    }

    fn heal(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        _info: MessageInfo,
    ) -> ContractResult<Response> {
        let lease_addr = self.lease.lease.addr.clone();
        balance::lpn_balance(&lease_addr, querier).and_then(|balance| {
            if balance.is_zero() {
                Err(ContractError::InconsistencyNotDetected())
            } else {
                repay::repay(self.lease, balance, &env, querier)
            }
        })
    }
}
