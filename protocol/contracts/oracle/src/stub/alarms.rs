use oracle_platform::OracleRef;

use currency::{Currency, CurrencyDef, Group, MemberOf};
use platform::batch::Batch;
use sdk::cosmwasm_std::{wasm_execute, Addr};

use crate::api::alarms::{Alarm, Error, ExecuteMsg, Result};

pub trait PriceAlarms<AlarmCurrencies>
where
    AlarmCurrencies: Group,
    Self: Into<Batch> + Sized,
{
    type BaseC: CurrencyDef;
    // BaseC::Group: MemberOf<Self::BaseG>;
    type BaseG: Group;

    fn add_alarm(self, alarm: Alarm<AlarmCurrencies, Self::BaseC, Self::BaseG>) -> Result<Batch>
    where
        <Self::BaseC as CurrencyDef>::Group:
            MemberOf<Self::BaseG> + MemberOf<AlarmCurrencies::TopG>;
}

pub trait AsAlarms<OracleBase, OracleBaseG>
where
    OracleBase: CurrencyDef + MemberOf<OracleBaseG>,
    OracleBaseG: Group,
{
    fn as_alarms<AlarmCurrencies>(
        &self,
    ) -> impl PriceAlarms<AlarmCurrencies, BaseC = OracleBase, BaseG = OracleBaseG>
    where
        AlarmCurrencies: Group,
        OracleBase::Group: MemberOf<AlarmCurrencies::TopG>;
}

impl<OracleBase, OracleBaseG> AsAlarms<OracleBase, OracleBaseG>
    for OracleRef<OracleBase, OracleBaseG>
where
    OracleBase: CurrencyDef,
    OracleBase::Group: MemberOf<OracleBaseG>,
    OracleBaseG: Group,
{
    fn as_alarms<AlarmCurrencies>(
        &self,
    ) -> impl PriceAlarms<AlarmCurrencies, BaseC = OracleBase, BaseG = OracleBaseG>
    where
        AlarmCurrencies: Group,
        OracleBase::Group: MemberOf<AlarmCurrencies::TopG>,
    {
        AlarmsStub {
            oracle_ref: self,
            batch: Batch::default(),
        }
    }
}

struct AlarmsStub<'a, OracleBase, OracleBaseG>
where
    OracleBase: Currency + MemberOf<OracleBaseG>,
    OracleBaseG: Group,
{
    oracle_ref: &'a OracleRef<OracleBase, OracleBaseG>,
    batch: Batch,
}

impl<'a, OracleBase, OracleBaseG> AlarmsStub<'a, OracleBase, OracleBaseG>
where
    OracleBase: Currency + MemberOf<OracleBaseG>,
    OracleBaseG: Group,
{
    fn addr(&self) -> &Addr {
        self.oracle_ref.addr()
    }
}

impl<'a, AlarmCurrencies, OracleBase, OracleBaseG> PriceAlarms<AlarmCurrencies>
    for AlarmsStub<'a, OracleBase, OracleBaseG>
where
    AlarmCurrencies: Group,
    OracleBase: CurrencyDef,
    OracleBase::Group: MemberOf<OracleBaseG> + MemberOf<AlarmCurrencies::TopG>,
    OracleBaseG: Group,
{
    type BaseC = OracleBase;
    type BaseG = OracleBaseG;

    fn add_alarm(self, alarm: Alarm<AlarmCurrencies, Self::BaseC, Self::BaseG>) -> Result<Batch> {
        let contract_addr = self.addr().clone();
        wasm_execute(contract_addr, &ExecuteMsg::AddPriceAlarm { alarm }, vec![])
            .map_err(Error::StubAddAlarm)
            .map(|execute_msg| self.batch.schedule_execute_no_reply(execute_msg))
    }
}

impl<'a, OracleBase, OracleBaseG> From<AlarmsStub<'a, OracleBase, OracleBaseG>> for Batch
where
    OracleBase: Currency + MemberOf<OracleBaseG>,
    OracleBaseG: Group,
{
    fn from(stub: AlarmsStub<'a, OracleBase, OracleBaseG>) -> Self {
        stub.batch
    }
}
