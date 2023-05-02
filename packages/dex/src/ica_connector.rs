use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use platform::{
    batch::{Batch, Emit, Emitter},
    ica::HostAccount,
    message,
};
use sdk::cosmwasm_std::{Addr, Deps, Env, QuerierWrapper, Timestamp};

use crate::{
    account::Account,
    connectable::DexConnectable,
    error::Result,
    response::{ContinueResult, Handler},
    Contract, Response,
};

pub trait Enterable {
    fn enter(&self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Result<Batch>;
}

pub const ICS27_MESSAGE_ENTERING_NEXT_STATE: bool = true;
pub const NO_ICS27_MESSAGE_ENTERING_NEXT_STATE: bool = !ICS27_MESSAGE_ENTERING_NEXT_STATE;

/// Entity expecting to be connected to ICA
///
/// Due to the fact that at the time we get the acknowledgement the underlying channel
/// is not yet fully functional, we are not allowed to use it right away.
/// There are usecases that do not use it immediatelly so they are ok to go at
/// this "preconnection" state. The others should be called in a next block to the
/// one that delivers the acknowledgement. Usually that could be done with
/// a time alarm.
pub trait IcaConnectee {
    type State;
    type NextState: Enterable + Into<Self::State>;

    fn connected(self, ica_account: Account) -> Self::NextState;
}

#[derive(Serialize, Deserialize)]
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

impl<Connectee, SwapResult> Enterable for IcaConnector<Connectee, SwapResult>
where
    Connectee: IcaConnectee + DexConnectable,
{
    fn enter(&self, _now: Timestamp, _querier: &QuerierWrapper<'_>) -> Result<Batch> {
        Ok(self.enter())
    }
}

impl<Connectee, SwapResult> Handler for IcaConnector<Connectee, SwapResult>
where
    Connectee: IcaConnectee + DexConnectable,
{
    type Response = Connectee::State;
    type SwapResult = SwapResult;

    fn on_open_ica(
        self,
        counterparty_version: String,
        deps: Deps<'_>,
        env: Env,
    ) -> ContinueResult<Self> {
        let ica = self.build_account(counterparty_version, &env)?;
        let ica_host = ica.host().clone();
        let contract = env.contract.address.clone();
        let next_state = self.connectee.connected(ica);
        next_state
            .enter(env.block.time, &deps.querier)
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

    fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
        self.connectee.state(now, querier)
    }
}
