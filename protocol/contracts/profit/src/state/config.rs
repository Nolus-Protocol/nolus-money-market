use serde::{Deserialize, Serialize};

use currencies::{Lpn as QuoteC, Lpns as QuoteG};
use currency::Group;
use oracle::stub::SwapPath;
use sdk::cosmwasm_std::Addr;
use timealarms::stub::TimeAlarmsRef;

use crate::typedefs::CadenceHours;

type OracleRef = oracle_platform::OracleRef<QuoteC, QuoteG>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Config {
    cadence_hours: CadenceHours,
    treasury: Addr,
    oracle: OracleRef,
    time_alarms: TimeAlarmsRef,
}

impl Config {
    pub fn new(
        cadence_hours: CadenceHours,
        treasury: Addr,
        oracle: OracleRef,
        time_alarms: TimeAlarmsRef,
    ) -> Self {
        Self {
            cadence_hours,
            treasury,
            oracle,
            time_alarms,
        }
    }

    pub fn update(self, cadence_hours: CadenceHours) -> Self {
        Self {
            cadence_hours,
            ..self
        }
    }

    pub fn cadence_hours(&self) -> CadenceHours {
        self.cadence_hours
    }

    pub fn treasury(&self) -> &Addr {
        &self.treasury
    }

    pub fn oracle<GSwap>(&self) -> &impl SwapPath<GSwap>
    where
        GSwap: Group,
    {
        &self.oracle
    }

    pub fn time_alarms(&self) -> &TimeAlarmsRef {
        &self.time_alarms
    }
}
