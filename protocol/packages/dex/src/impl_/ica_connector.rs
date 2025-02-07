use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};

use finance::duration::Duration;
use platform::{
    batch::{Batch, Emit, Emitter},
    ica::HostAccount,
    message,
};
use sdk::cosmwasm_std::{Addr, Env, QuerierWrapper, Timestamp};

use crate::error::Result;
#[cfg(feature = "migration")]
use crate::impl_::{InspectSpec, MigrateSpec};

use super::{
    response::{ContinueResult, Handler},
    Account, Contract, DexConnectable, Response, TimeAlarm,
};

pub trait Enterable {
    fn enter(&self, now: Timestamp, querier: QuerierWrapper<'_>) -> Result<Batch>;
}

pub const ICS27_MESSAGE_ENTERING_NEXT_STATE: bool = true;
pub const NO_ICS27_MESSAGE_ENTERING_NEXT_STATE: bool = !ICS27_MESSAGE_ENTERING_NEXT_STATE;

/// Entity expecting to be connected to ICA
pub trait IcaConnectee {
    type State;
    type NextState: Enterable + Into<Self::State>;

    fn connected(self, ica_account: Account) -> Self::NextState;
}

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "Connectee: Serialize",
    deserialize = "Connectee: Deserialize<'de>",
))]
pub struct IcaConnector<Connectee, SwapResult> {
    connectee: Connectee,
    #[serde(skip)]
    _swap_result: PhantomData<SwapResult>,
}

impl<Connectee, SwapResult> IcaConnector<Connectee, SwapResult>
where
    Connectee: IcaConnectee + DexConnectable,
{
    const STATE_LABEL: &'static str = "register-ica";

    pub fn new(connectee: Connectee) -> Self {
        Self {
            connectee,
            _swap_result: PhantomData,
        }
    }

    pub fn enter(&self) -> Batch {
        Account::register_request(self.connectee.dex())
    }

    fn build_account(&self, counterparty_version: String, env: &Env) -> Result<Account> {
        let contract = env.contract.address.clone();
        Account::from_register_response(
            &counterparty_version,
            contract,
            self.connectee.dex().clone(),
        )
    }

    fn emit_ok(contract: Addr, ica_host: HostAccount) -> Emitter {
        Emitter::of_type(Self::STATE_LABEL)
            .emit("id", contract)
            .emit("ica_host", ica_host)
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, SwapTaskNew, SEnumNew, Connectee, SwapResult>
    MigrateSpec<SwapTask, SwapTaskNew, SEnumNew> for IcaConnector<Connectee, SwapResult>
where
    Connectee: MigrateSpec<SwapTask, SwapTaskNew, SEnumNew>,
    Connectee::Out: IcaConnectee + DexConnectable,
{
    type Out = IcaConnector<Connectee::Out, SwapResult>;

    fn migrate_spec<MigrateFn>(self, migrate_fn: MigrateFn) -> Self::Out
    where
        MigrateFn: FnOnce(SwapTask) -> SwapTaskNew,
    {
        Self::Out::new(self.connectee.migrate_spec(migrate_fn))
    }
}

#[cfg(feature = "migration")]
impl<SwapTask, R, Connectee, SwapResult> InspectSpec<SwapTask, R>
    for IcaConnector<Connectee, SwapResult>
where
    Connectee: InspectSpec<SwapTask, R>,
{
    fn inspect_spec<InspectFn>(&self, inspect_fn: InspectFn) -> R
    where
        InspectFn: FnOnce(&SwapTask) -> R,
    {
        self.connectee.inspect_spec(inspect_fn)
    }
}

impl<Connectee, SwapResult> Enterable for IcaConnector<Connectee, SwapResult>
where
    Connectee: IcaConnectee + DexConnectable,
{
    fn enter(&self, _now: Timestamp, _querier: QuerierWrapper<'_>) -> Result<Batch> {
        Ok(self.enter())
    }
}

impl<Connectee, SwapResult> Handler for IcaConnector<Connectee, SwapResult>
where
    Connectee: IcaConnectee + DexConnectable + Display,
{
    type Response = Connectee::State;
    type SwapResult = SwapResult;

    fn on_open_ica(
        self,
        counterparty_version: String,
        querier: QuerierWrapper<'_>,
        env: Env,
    ) -> ContinueResult<Self> {
        let ica = self.build_account(counterparty_version, &env)?;
        let ica_host = ica.host().clone();
        let contract = env.contract.address.clone();
        let next_state = self.connectee.connected(ica);
        next_state
            .enter(env.block.time, querier)
            .map(|batch| {
                message::Response::messages_with_events(batch, Self::emit_ok(contract, ica_host))
            })
            .map(|cw_resp| Response::<Self>::from(cw_resp, next_state))
    }
}

impl<Connectee, SwapResult> Contract for IcaConnector<Connectee, SwapResult>
where
    Connectee: Contract,
{
    type StateResponse = Connectee::StateResponse;

    fn state(
        self,
        now: Timestamp,
        due_projection: Duration,
        querier: QuerierWrapper<'_>,
    ) -> Self::StateResponse {
        self.connectee.state(now, due_projection, querier)
    }
}

impl<Connectee, SwapResult> Display for IcaConnector<Connectee, SwapResult>
where
    Connectee: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_fmt(format_args!("IcaConnector({})", self.connectee))
    }
}

impl<Connectee, SwapResult> TimeAlarm for IcaConnector<Connectee, SwapResult>
where
    Connectee: TimeAlarm,
{
    fn setup_alarm(&self, forr: Timestamp) -> Result<Batch> {
        self.connectee.setup_alarm(forr).map_err(Into::into)
    }
}
