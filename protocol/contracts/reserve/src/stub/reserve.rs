use currency::{CurrencyDef, MemberOf};
use finance::coin::Coin;
use platform::batch::Batch;

use crate::{
    api::{ExecuteMsg, LpnCurrencies},
    error::Error,
};

use super::Ref;

pub trait Reserve<Lpn>
where
    Self: TryInto<Batch, Error = Error>,
{
    fn cover_liquidation_losses(&mut self, amount: Coin<Lpn>);
}

pub(crate) struct Impl<Lpn> {
    ref_: Ref<Lpn>,
    amount: Option<Coin<Lpn>>,
}

impl<Lpn> Impl<Lpn> {
    pub fn new(ref_: Ref<Lpn>) -> Self {
        Self { ref_, amount: None }
    }
}

impl<Lpn> Reserve<Lpn> for Impl<Lpn>
where
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<LpnCurrencies>,
{
    fn cover_liquidation_losses(&mut self, amount: Coin<Lpn>) {
        debug_assert!(self.amount.is_none());
        self.amount = Some(amount);
    }
}

impl<Lpn> TryFrom<Impl<Lpn>> for Batch
where
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<LpnCurrencies>,
{
    type Error = Error;

    fn try_from(stub: Impl<Lpn>) -> Result<Self, Self::Error> {
        stub.amount.map_or_else(
            || Ok(Batch::default()),
            |losses| {
                Batch::default()
                    .schedule_execute_wasm_no_reply_no_funds(
                        stub.ref_.into(),
                        &ExecuteMsg::CoverLiquidationLosses(losses.into()),
                    )
                    .map_err(Into::into)
            },
        )
    }

    // fn try_from(stub: Impl<Lpn>) -> Result<Self, Self::Error> {
    //     let batch = Batch::default();
    //     if let Some(losses) = stub.amount {
    //         batch
    //             .schedule_execute_wasm_no_reply_no_funds(
    //                 stub.ref_.into(),
    //                 &ExecuteMsg::CoverLiquidationLosses(losses.into()),
    //             )
    //             .map_err(Into::into)
    //     } else {
    //         Ok(batch)
    //     }
    // }
}
