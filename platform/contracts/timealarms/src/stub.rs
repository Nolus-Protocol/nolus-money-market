use std::result::Result as StdResult;

use serde::{Deserialize, Serialize};

use platform::{batch::Batch, contract};
use sdk::cosmwasm_std::{wasm_execute, Addr, QuerierWrapper, Timestamp};

use crate::{msg::ExecuteMsg, ContractError};

pub type Result<T> = StdResult<T, ContractError>;

pub trait TimeAlarms {
    fn add_alarm(self, time: Timestamp) -> Result<Batch>;
}

pub trait WithTimeAlarms {
    type Output;
    type Error;

    fn exec<TA>(self, time_alarms: TA) -> StdResult<Self::Output, Self::Error>
    where
        TA: TimeAlarms;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TimeAlarmsRef {
    addr: Addr,
}

impl TimeAlarmsRef {
    pub fn new(addr: Addr, querier: QuerierWrapper<'_>) -> Result<Self> {
        contract::validate_addr(querier, &addr)?;

        Ok(Self { addr })
    }

    pub fn owned_by(&self, addr: &Addr) -> bool {
        self.addr == addr
    }

    pub fn setup_alarm(&self, when: Timestamp) -> Result<Batch> {
        self.as_stub().add_alarm(when)
    }

    /// It would be overengineering to hide the `TimeAlarms` implementation
    fn as_stub(&self) -> TimeAlarmsStub<'_> {
        TimeAlarmsStub {
            time_alarms_ref: self,
            batch: Default::default(),
        }
    }
}

#[cfg(feature = "testing")]
impl TimeAlarmsRef {
    pub fn unchecked<A>(addr: A) -> Self
    where
        A: Into<String>,
    {
        Self {
            addr: Addr::unchecked(addr),
        }
    }
}

struct TimeAlarmsStub<'a> {
    time_alarms_ref: &'a TimeAlarmsRef,
    batch: Batch,
}

impl<'a> TimeAlarmsStub<'a> {
    fn addr(&self) -> &Addr {
        &self.time_alarms_ref.addr
    }
}

impl<'a> TimeAlarms for TimeAlarmsStub<'a> {
    fn add_alarm(self, time: Timestamp) -> Result<Batch> {
        wasm_execute(self.addr(), &ExecuteMsg::AddAlarm { time }, vec![])
            .map_err(Into::into)
            .map(|msg| self.batch.schedule_execute_no_reply(msg))
    }
}

impl<'a> From<TimeAlarmsStub<'a>> for Batch {
    fn from(stub: TimeAlarmsStub<'a>) -> Self {
        stub.batch
    }
}
