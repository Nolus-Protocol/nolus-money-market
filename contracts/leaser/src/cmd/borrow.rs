use finance::currency::SymbolOwned;
use lease::api::{LoanForm, NewLeaseForm};
use platform::batch::Batch;
use sdk::{
    cosmwasm_ext::Response,
    cosmwasm_std::{Addr, Coin, DepsMut},
};

use crate::{
    error::ContractResult,
    state::{config::Config, leaser::Loans},
    ContractError,
};

use super::Borrow;

impl Borrow {
    pub fn with(
        deps: DepsMut,
        amount: Vec<Coin>,
        sender: Addr,
        currency: SymbolOwned,
    ) -> Result<Response, ContractError> {
        let config = Config::load(deps.storage)?;
        let instance_reply_id = Loans::next(deps.storage, sender.clone())?;

        let mut batch = Batch::default();
        batch.schedule_instantiate_wasm_on_success_reply(
            config.lease_code_id,
            Self::open_lease_msg(sender, config, currency)?,
            Some(amount),
            "lease",
            None,
            instance_reply_id,
        )?;
        Ok(batch.into())
    }

    pub(crate) fn open_lease_msg(
        sender: Addr,
        config: Config,
        currency: SymbolOwned,
    ) -> ContractResult<NewLeaseForm> {
        config
            .dex
            .map(|dex| NewLeaseForm {
                customer: sender.into_string(),
                currency,
                liability: config.liability,
                loan: LoanForm {
                    annual_margin_interest: config.lease_interest_rate_margin,
                    lpp: config.lpp_addr.into_string(),
                    interest_payment: config.lease_interest_payment,
                    profit: config.profit.into_string(),
                },
                time_alarms: config.time_alarms.into_string(),
                market_price_oracle: config.market_price_oracle.into_string(),
                dex,
            })
            .ok_or(ContractError::NoDEXConnectivitySetup {})
    }
}
