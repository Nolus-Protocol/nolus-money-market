#[cfg(feature = "skel_testing")]
use serde::Deserialize;
use serde::Serialize;

use finance::percent::Percent;

use crate::api::{LeaseCoin, PaymentCoin};

/// The data transport type of the configured Lease close policy
///
/// Designed for use in query responses only!
#[derive(Serialize)]
#[cfg_attr(
    feature = "skel_testing",
    derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ClosePolicy {
    take_profit: Option<Percent>,
    stop_loss: Option<Percent>,
}

#[derive(Serialize)]
#[cfg_attr(
    feature = "skel_testing",
    derive(Clone, PartialEq, Eq, Debug, Deserialize)
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum OngoingTrx {
    Repayment {
        payment: PaymentCoin,
        in_progress: RepayTrx,
    },
    Liquidation {
        liquidation: LeaseCoin,
        in_progress: PositionCloseTrx,
    },
    Close {
        close: LeaseCoin,
        in_progress: PositionCloseTrx,
    },
}

#[derive(Serialize)]
#[cfg_attr(
    feature = "skel_testing",
    derive(Clone, PartialEq, Eq, Debug, Deserialize)
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum RepayTrx {
    TransferOut,
    Swap,
    TransferInInit,
    TransferInFinish,
}

#[derive(Serialize)]
#[cfg_attr(
    feature = "skel_testing",
    derive(Clone, PartialEq, Eq, Debug, Deserialize)
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum PositionCloseTrx {
    Swap,
    TransferInInit,
    TransferInFinish,
}

impl ClosePolicy {
    #[cfg(feature = "contract")]
    pub(crate) fn new(tp: Option<Percent>, sl: Option<Percent>) -> Self {
        Self {
            take_profit: tp,
            stop_loss: sl,
        }
    }

    #[cfg(feature = "contract_testing")]
    pub fn new_testing(tp: Option<Percent>, sl: Option<Percent>) -> Self {
        Self::new(tp, sl)
    }
}
