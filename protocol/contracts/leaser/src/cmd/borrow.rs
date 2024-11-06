use currency::CurrencyDTO;
use finance::percent::Percent;
use lease::api::open::{LoanForm, NewLeaseContract, NewLeaseForm};
use platform::batch::Batch;
use platform::message::Response as MessageResponse;
use sdk::cosmwasm_std::{Addr, Coin, Storage};

use crate::{
    finance::LeaseCurrencies,
    state::{config::Config, leases::Leases},
    ContractError,
};

pub struct Borrow {}
impl Borrow {
    pub fn with(
        storage: &mut dyn Storage,
        amount: Vec<Coin>,
        customer: Addr,
        admin: Addr,
        finalizer: Addr,
        currency: CurrencyDTO<LeaseCurrencies>,
        max_ltd: Option<Percent>,
    ) -> Result<MessageResponse, ContractError> {
        Leases::cache_open_req(storage, &customer)
            .and_then(|()| Config::load(storage))
            .and_then(|config| {
                Batch::default()
                    .schedule_instantiate_wasm_reply_on_success(
                        config.lease_code,
                        &Self::open_lease_msg(customer, config, currency, max_ltd, finalizer),
                        Some(amount),
                        "lease".into(),
                        Some(admin), // allows lease migrations from this contract
                        Default::default(),
                    )
                    .map_err(Into::into)
            })
            .map(Into::into)
    }

    pub(crate) fn open_lease_msg(
        customer: Addr,
        config: Config,
        currency: CurrencyDTO<LeaseCurrencies>,
        max_ltd: Option<Percent>,
        finalizer: Addr,
    ) -> NewLeaseContract {
        NewLeaseContract {
            form: NewLeaseForm {
                customer,
                currency,
                max_ltd,
                position_spec: config.lease_position_spec,
                loan: LoanForm {
                    lpp: config.lpp,
                    profit: config.profit,
                    annual_margin_interest: config.lease_interest_rate_margin,
                    due_period: config.lease_due_period,
                },
                reserve: config.reserve,
                time_alarms: config.time_alarms,
                market_price_oracle: config.market_price_oracle,
            },
            dex: config.dex,
            finalizer,
        }
    }
}
