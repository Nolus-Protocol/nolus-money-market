use currency::native::Nls;
use finance::coin::Coin;
use platform::{
    bank::BankAccount,
    batch::{Emit as _, Emitter},
    message::Response as PlatformResponse,
};
use sdk::cosmwasm_std::{Addr, Env, Storage};

use crate::{
    msg::ConfigResponse,
    result::ContractResult,
    state::{ConfigManagement as _, State},
};

pub struct Profit;

impl Profit {
    const IBC_FEE_RESERVE: Coin<Nls> = Coin::new(100);

    pub(crate) fn transfer_nls<B>(
        mut from_my_account: B,
        to_treasury: &Addr,
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
            from_my_account.send(amount, to_treasury);

            PlatformResponse::messages_with_events(
                from_my_account.into(),
                Emitter::of_type("tr-profit")
                    .emit_tx_info(env)
                    .emit_coin("profit-amount", amount),
            )
        }
    }

    pub fn query_config(storage: &dyn Storage) -> ContractResult<ConfigResponse> {
        State::load(storage).and_then(|state: State| state.try_query_config())
    }
}
