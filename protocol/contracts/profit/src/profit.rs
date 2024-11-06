use currencies::Nls;
use dex::Contract;
use finance::coin::Coin;
use platform::{
    bank::BankAccount,
    batch::{Emit as _, Emitter},
    message::Response as PlatformResponse,
};
use sdk::cosmwasm_std::{Addr, Env, QuerierWrapper, Storage, Timestamp};

use crate::{msg::ConfigResponse, result::ContractResult, state::State};

pub struct Profit;

impl Profit {
    pub const IBC_FEE_RESERVE: Coin<Nls> = Coin::new(100);

    pub(crate) fn transfer_nls<B>(
        from_my_account: B,
        to_treasury: Addr,
        mut amount: Coin<Nls>,
        env: &Env,
    ) -> PlatformResponse
    where
        B: BankAccount,
    {
        amount = amount.saturating_sub(Self::IBC_FEE_RESERVE);

        if amount.is_zero() {
            PlatformResponse::messages_only(from_my_account.into())
        } else {
            PlatformResponse::messages_with_events(
                from_my_account.send(amount, to_treasury).into(),
                Emitter::of_type("tr-profit")
                    .emit_tx_info(env)
                    .emit_coin("profit-amount", amount),
            )
        }
    }

    pub fn query_config(
        storage: &dyn Storage,
        now: Timestamp,
        querier: QuerierWrapper<'_>,
    ) -> ContractResult<ConfigResponse> {
        State::load(storage).map(|state: State| state.state(now, querier))
    }
}
