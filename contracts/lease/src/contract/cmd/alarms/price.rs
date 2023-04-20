use serde::Serialize;

use finance::currency::Currency;
use lpp::stub::lender::LppLender as LppLenderTrait;
use oracle::stub::Oracle as OracleTrait;
use platform::message::Response as MessageResponse;
use profit::stub::Profit as ProfitTrait;
use sdk::cosmwasm_std::{Addr, Env, Timestamp};
use timealarms::stub::TimeAlarms as TimeAlarmsTrait;

use crate::{
    error::ContractError,
    lease::{with_lease::WithLease, IntoDTOResult, Lease},
};

pub struct PriceAlarm<'a> {
    env: &'a Env,
    sender: &'a Addr,
    now: Timestamp,
}

impl<'a> PriceAlarm<'a> {
    pub fn new(env: &'a Env, sender: &'a Addr, now: Timestamp) -> Self {
        Self { env, sender, now }
    }
}

impl<'a> WithLease for PriceAlarm<'a> {
    type Output = MessageResponse;

    type Error = ContractError;

    fn exec<Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>(
        self,
        lease: Lease<Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>,
    ) -> Result<Self::Output, Self::Error>
    where
        Lpn: Currency + Serialize,
        Lpp: LppLenderTrait<Lpn>,
        TimeAlarms: TimeAlarmsTrait,
        Oracle: OracleTrait<Lpn>,
        Profit: ProfitTrait,
        Asset: Currency + Serialize,
    {
        if !lease.sent_by_oracle(self.sender) {
            return Err(Self::Error::Unauthorized {});
        }

        //TODO revive once https://github.com/nolus-protocol/nolus-money-market/issues/49 is done
        // let OnAlarmResult {
        //     batch,
        //     liquidation_status,
        // } = lease.on_price_alarm(self.now)?;

        // let resp = super::emit_events(self.env, &liquidation_status).map_or_else(
        //     || MessageResponse::messages_only(batch),
        //     |events| MessageResponse::messages_with_events(batch, events),
        // );

        // response::response_with_messages(
        //     &self.env.contract.address,
        //     resp,
        // )
        let IntoDTOResult { batch, lease: _ } = lease.into_dto();
        Ok(batch.into())
    }
}
