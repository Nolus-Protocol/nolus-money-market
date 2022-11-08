use finance::duration::Duration;
use sdk::{
    cosmwasm_std::{to_binary, Addr, Api},
    neutron_sdk::bindings::{msg::NeutronMsg, types::ProtobufAny},
};
use serde::Serialize;

use crate::{
    batch::Batch as LocalBatch,
    error::{Error, Result},
};

use self::impl_::OpenAckVersion;

/// Identifier of the ICA account opened by a lease
/// It is unique for a lease and allows the support of multiple accounts per lease
const ICA_ACCOUNT_ID: &str = "0";

pub fn register_account<C>(connection: C) -> LocalBatch
where
    C: Into<String>,
{
    let mut batch = LocalBatch::default();
    batch.schedule_execute_no_reply(NeutronMsg::register_interchain_account(
        connection.into(),
        ICA_ACCOUNT_ID.into(),
    ));
    batch
}

pub fn parse_register_response(api: &dyn Api, response: &str) -> Result<Addr> {
    let open_ack = serde_json_wasm::from_str::<OpenAckVersion>(response)?;
    api.addr_validate(&open_ack.address).map_err(Error::from)
}

pub fn submit_transaction<C, M>(
    connection: C,
    messages: Batch,
    memo: M,
    timeout: Duration,
) -> LocalBatch
where
    C: Into<String>,
    M: Into<String>,
{
    let mut batch = LocalBatch::default();

    batch.schedule_execute_no_reply(NeutronMsg::submit_tx(
        connection.into(),
        ICA_ACCOUNT_ID.into(),
        messages.msgs,
        memo.into(),
        timeout.secs(),
    ));
    batch
}

#[derive(Default)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq))]
pub struct Batch {
    msgs: Vec<ProtobufAny>,
}

impl Batch {
    pub fn add_message<T, M>(&mut self, msg_type: T, msg: M) -> Result<()>
    where
        T: Into<String>,
        M: Serialize,
    {
        self.msgs
            .push(ProtobufAny::new(msg_type.into(), to_binary(&msg)?));
        Ok(())
    }
}

mod impl_ {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "snake_case")]
    pub struct OpenAckVersion {
        pub version: String,
        pub controller_connection_id: String,
        pub host_connection_id: String,
        pub address: String,
        pub encoding: String,
        pub tx_type: String,
    }
}
