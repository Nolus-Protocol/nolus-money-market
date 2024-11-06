use std::vec;

use serde::Serialize;

use currency::CurrencyDef;
use finance::coin::Coin;
use sdk::{
    cosmwasm_ext::{CosmosMsg, SubMsg},
    cosmwasm_std::{to_json_binary, Addr, Coin as CoinCw, WasmMsg},
};

pub use crate::emit::{Emit, Emitter};
use crate::{coin_legacy::to_cosmwasm_impl, contract::Code, error::Error, result::Result};

pub type ReplyId = u64;

#[must_use]
#[derive(Default)]
#[cfg_attr(
    any(debug_assertions, test, feature = "testing"),
    derive(Debug, PartialEq, Eq)
)]
pub struct Batch {
    msgs: Vec<SubMsg>,
}

impl Batch {
    pub fn schedule_execute_no_reply<M>(self, msg: M) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_no_reply(msg)
    }

    pub fn schedule_execute_reply_on_success<M>(self, msg: M, reply_id: ReplyId) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_reply_on_success(msg, reply_id)
    }

    pub fn schedule_execute_wasm_no_reply_no_funds<M>(self, addr: Addr, msg: &M) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_exec_msg_no_funds(addr, msg).map(|wasm_msg| self.schedule_no_reply(wasm_msg))
    }

    pub fn schedule_execute_wasm_no_reply<M, C>(
        self,
        addr: Addr,
        msg: &M,
        funds: Option<Coin<C>>,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
        C: CurrencyDef,
    {
        Self::wasm_exec_msg(addr, msg, funds).map(|wasm_msg| self.schedule_no_reply(wasm_msg))
    }

    pub fn schedule_execute_wasm_reply_on_success_no_funds<M>(
        self,
        addr: Addr,
        msg: &M,
        reply_id: ReplyId,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_exec_msg_no_funds(addr, msg)
            .map(|wasm_msg| self.schedule_reply_on_success(wasm_msg, reply_id))
    }

    pub fn schedule_execute_wasm_reply_on_success<M, C>(
        self,
        addr: Addr,
        msg: &M,
        funds: Option<Coin<C>>,
        reply_id: ReplyId,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
        C: CurrencyDef,
    {
        Self::wasm_exec_msg(addr, msg, funds)
            .map(|wasm_msg| self.schedule_reply_on_success(wasm_msg, reply_id))
    }

    pub fn schedule_execute_wasm_reply_always_no_funds<M>(
        self,
        addr: Addr,
        msg: &M,
        reply_id: ReplyId,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_exec_msg_no_funds(addr, msg)
            .map(|wasm_msg| self.schedule_reply_always(wasm_msg, reply_id))
    }

    pub fn schedule_execute_wasm_reply_on_error_no_funds<M>(
        self,
        addr: Addr,
        msg: &M,
        reply_id: ReplyId,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_exec_msg_no_funds(addr, msg)
            .map(|wasm_msg| self.schedule_reply_on_error(wasm_msg, reply_id))
    }

    pub fn schedule_instantiate_wasm_reply_on_success<M>(
        self,
        code: Code,
        msg: &M,
        funds: Option<Vec<CoinCw>>,
        label: String,
        admin: Option<Addr>,
        reply_id: ReplyId,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_init_msg(code, msg, funds, label, admin)
            .map(|wasm_msg| self.schedule_reply_on_success(wasm_msg, reply_id))
    }

    pub fn schedule_migrate_wasm_no_reply<M>(
        self,
        addr: Addr,
        msg: &M,
        new_code: Code,
    ) -> Result<Self>
    where
        M: Serialize + ?Sized,
    {
        Self::wasm_migrate_msg(addr, msg, new_code).map(|wasm_msg| self.schedule_no_reply(wasm_msg))
    }

    pub fn merge(mut self, mut other: Batch) -> Self {
        self.msgs.append(&mut other.msgs);

        self
    }

    pub fn len(&self) -> usize {
        self.msgs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.msgs.is_empty()
    }

    fn wasm_exec_msg_no_funds<M>(addr: Addr, msg: &M) -> Result<WasmMsg>
    where
        M: Serialize + ?Sized,
    {
        to_json_binary(msg)
            .map_err(Error::Serialization)
            .map(|raw_msg| WasmMsg::Execute {
                contract_addr: addr.into_string(),
                funds: vec![],
                msg: raw_msg,
            })
    }

    fn wasm_exec_msg<M, C>(addr: Addr, msg: &M, funds: Option<Coin<C>>) -> Result<WasmMsg>
    where
        M: Serialize + ?Sized,
        C: CurrencyDef,
    {
        to_json_binary(msg)
            .map_err(Error::Serialization)
            .map(|msg| WasmMsg::Execute {
                contract_addr: addr.into_string(),
                funds: if let Some(funds) = funds {
                    vec![to_cosmwasm_impl(funds)]
                } else {
                    vec![]
                },
                msg,
            })
            .map_err(Into::into)
    }

    fn wasm_init_msg<M>(
        code: Code,
        msg: &M,
        funds: Option<Vec<CoinCw>>,
        label: String,
        admin: Option<Addr>,
    ) -> Result<WasmMsg>
    where
        M: Serialize + ?Sized,
    {
        to_json_binary(msg)
            .map_err(Error::Serialization)
            .map(|msg| WasmMsg::Instantiate {
                admin: admin.map(Addr::into_string),
                code_id: code.into(),
                funds: funds.unwrap_or_default(),
                label,
                msg,
            })
            .map_err(Into::into)
    }

    fn wasm_migrate_msg<M>(addr: Addr, msg: &M, new_code: Code) -> Result<WasmMsg>
    where
        M: Serialize + ?Sized,
    {
        to_json_binary(msg)
            .map_err(Error::Serialization)
            .map(|msg| WasmMsg::Migrate {
                contract_addr: addr.into_string(),
                new_code_id: new_code.into(),
                msg,
            })
            .map_err(Into::into)
    }

    fn schedule_no_reply<M>(self, msg: M) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_msg(SubMsg::new(msg))
    }

    fn schedule_reply_on_success<M>(self, msg: M, reply_id: ReplyId) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_msg(SubMsg::reply_on_success(msg, reply_id))
    }

    fn schedule_reply_on_error<M>(self, msg: M, reply_id: ReplyId) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_msg(SubMsg::reply_on_error(msg, reply_id))
    }

    fn schedule_reply_always<M>(self, msg: M, reply_id: ReplyId) -> Self
    where
        M: Into<CosmosMsg>,
    {
        self.schedule_msg(SubMsg::reply_always(msg, reply_id))
    }

    #[inline]
    fn schedule_msg(mut self, msg: SubMsg) -> Self {
        self.msgs.push(msg);
        self
    }
}

impl IntoIterator for Batch {
    type Item = SubMsg;

    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.msgs.into_iter()
    }
}

#[cfg(test)]
mod test {
    use sdk::{cosmwasm_ext::CosmosMsg, cosmwasm_std::WasmMsg};

    use super::Batch;

    #[test]
    fn no_events() {
        let b = Batch::default();
        assert_eq!(0, b.len());
        assert!(b.is_empty());

        let b = b.schedule_execute_no_reply(CosmosMsg::Wasm(WasmMsg::ClearAdmin {
            contract_addr: "".to_string(),
        }));
        assert_eq!(1, b.len());
        assert!(!b.is_empty());
    }

    #[test]
    fn msgs_len() {
        let b = Batch::default();
        assert_eq!(0, b.len());
        assert!(b.is_empty());
        let b = b.schedule_execute_no_reply(CosmosMsg::Wasm(WasmMsg::ClearAdmin {
            contract_addr: "".into(),
        }));
        assert_eq!(1, b.len());
        let b = b.schedule_execute_no_reply(CosmosMsg::Wasm(WasmMsg::UpdateAdmin {
            contract_addr: "".into(),
            admin: "".into(),
        }));
        assert_eq!(2, b.len());
        assert!(!b.is_empty());
    }
}
