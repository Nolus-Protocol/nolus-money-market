use std::marker::PhantomData;

use currencies::Lpns;
use currency::Currency;
use oracle_platform::OracleRef;
use platform::batch::Batch;
use sdk::cosmwasm_std::{wasm_execute, Addr};

use crate::api::alarms::{Alarm, AlarmCurrencies, Error, ExecuteMsg, Result};

pub trait PriceAlarms
where
    Self: Into<Batch> + Sized,
{
    //TODO use a type-safe Alarm, one with the typed Price
    fn add_alarm(&mut self, alarm: Alarm<AlarmCurrencies, Lpns>) -> Result<()>;
}

pub trait AsAlarms {
    fn as_alarms<OracleBase>(&self) -> impl PriceAlarms
    where
        OracleBase: Currency;
}

impl AsAlarms for OracleRef {
    fn as_alarms<OracleBase>(&self) -> impl PriceAlarms
    where
        OracleBase: Currency,
    {
        self.check_base::<OracleBase>();

        AlarmsStub {
            oracle_ref: self,
            batch: Batch::default(),
            _quote_currency: PhantomData::<OracleBase>,
        }
    }
}

struct AlarmsStub<'a, OracleBase> {
    oracle_ref: &'a OracleRef,
    _quote_currency: PhantomData<OracleBase>,
    batch: Batch,
}

impl<'a, OracleBase> AlarmsStub<'a, OracleBase> {
    fn addr(&self) -> &Addr {
        self.oracle_ref.addr()
    }
}

impl<'a, OracleBase> PriceAlarms for AlarmsStub<'a, OracleBase>
where
    OracleBase: Currency,
{
    fn add_alarm(&mut self, alarm: Alarm<AlarmCurrencies, Lpns>) -> Result<()> {
        self.batch.schedule_execute_no_reply(
            wasm_execute(
                self.addr().clone(),
                &ExecuteMsg::AddPriceAlarm { alarm },
                vec![],
            )
            .map_err(Error::StubAddAlarm)?,
        );

        Ok(())
    }
}

impl<'a, OracleBase> From<AlarmsStub<'a, OracleBase>> for Batch {
    fn from(stub: AlarmsStub<'a, OracleBase>) -> Self {
        stub.batch
    }
}
