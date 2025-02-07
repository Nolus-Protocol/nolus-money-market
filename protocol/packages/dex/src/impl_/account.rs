use serde::{Deserialize, Serialize};

use currency::Group;
use oracle::stub::SwapPath;
use platform::{
    batch::Batch as LocalBatch,
    ica::{self, HostAccount},
};
use sdk::cosmwasm_std::{Addr, QuerierWrapper, Timestamp};

use crate::{error::Result, ConnectionParams};

use super::{
    trx::{SwapTrx, TransferInTrx, TransferOutTrx},
    DexConnectable,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Account {
    /// The contract at Nolus that owns the account
    owner: Addr,
    host: HostAccount,
    dex: ConnectionParams,
}

impl Account {
    pub fn owner(&self) -> &Addr {
        &self.owner
    }

    pub(super) fn host(&self) -> &HostAccount {
        &self.host
    }

    pub(super) fn register_request(dex: &ConnectionParams) -> LocalBatch {
        ica::register_account(&dex.connection_id)
    }

    pub(super) fn from_register_response(
        response: &str,
        owner: Addr,
        dex: ConnectionParams,
    ) -> Result<Self> {
        let host = ica::parse_register_response(response)?;
        Ok(Self { owner, host, dex })
    }

    pub(super) fn transfer_to(&self, now: Timestamp) -> TransferOutTrx<'_> {
        TransferOutTrx::new(
            &self.dex.transfer_channel.local_endpoint,
            &self.owner,
            &self.host,
            now,
            format!(
                "Transfer out: {sender} -> {receiver}",
                sender = self.owner,
                receiver = self.host
            ),
        )
    }

    pub(super) fn swap<'a, SwapGroup, SwapPathImpl>(
        &'a self,
        swap_path: &'a SwapPathImpl,
        querier: QuerierWrapper<'a>,
    ) -> SwapTrx<'a, SwapGroup, SwapPathImpl>
    where
        SwapGroup: Group,
        SwapPathImpl: SwapPath<SwapGroup>,
    {
        SwapTrx::new(&self.dex.connection_id, &self.host, swap_path, querier)
    }

    pub(super) fn transfer_from(&self, now: Timestamp) -> TransferInTrx<'_> {
        TransferInTrx::new(
            &self.dex.connection_id,
            &self.dex.transfer_channel.remote_endpoint,
            &self.host,
            &self.owner,
            now,
        )
    }

    #[cfg(feature = "testing")]
    pub fn unchecked(owner: Addr, host: HostAccount, dex: ConnectionParams) -> Self {
        Self { owner, host, dex }
    }
}

impl From<Account> for HostAccount {
    fn from(account: Account) -> Self {
        account.host
    }
}

impl DexConnectable for Account {
    fn dex(&self) -> &ConnectionParams {
        &self.dex
    }
}
