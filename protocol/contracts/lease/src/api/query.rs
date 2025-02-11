use currency::CurrencyDTO;
use serde::{Deserialize, Serialize};

use finance::{
    duration::{Duration, Seconds},
    percent::Percent,
};
use sdk::cosmwasm_std::Timestamp;

use crate::finance::LpnCoinDTO;

use super::{DownpaymentCoin, LeaseAssetCurrencies, LeaseCoin};

pub use opened::ClosePolicy;

#[derive(Deserialize)]
#[cfg_attr(feature = "skel_testing", derive(Clone, Debug, PartialEq, Serialize))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum QueryMsg {
    /// Ask for estimation of the due and overdue amounts and periods in that point of time
    ///
    /// Return a [StateResponse]
    ///
    /// The value is meaningfull only if the lease is in Opened state.
    State {
        #[serde(default, rename = "due_projection_secs")]
        due_projection: Seconds,
    },
    /// Implementation of [versioning::query::ProtocolPackage::Release]
    ProtocolPackageRelease {},
}

#[derive(Serialize)]
#[cfg_attr(
    feature = "skel_testing",
    derive(Clone, PartialEq, Eq, Debug, Deserialize)
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum StateResponse {
    Opening {
        currency: CurrencyDTO<LeaseAssetCurrencies>,
        downpayment: DownpaymentCoin,
        loan: LpnCoinDTO,
        loan_interest_rate: Percent,
        in_progress: opening::OngoingTrx,
    },
    Opened {
        amount: LeaseCoin,
        loan_interest_rate: Percent,
        margin_interest_rate: Percent,
        principal_due: LpnCoinDTO,
        overdue_margin: LpnCoinDTO,
        overdue_interest: LpnCoinDTO,
        overdue_collect_in: Duration,
        due_margin: LpnCoinDTO,
        due_interest: LpnCoinDTO,
        /// Time offset ahead, past the `validity`, at which the due and overdue amounts and periods are estimated for.
        ///
        /// It always corresponds to the requested `StateQuery::due_projection` or 0 if not present.
        #[serde(rename = "due_projection_ns")]
        due_projection: Duration,
        close_policy: ClosePolicy,
        validity: Timestamp,
        in_progress: Option<opened::OngoingTrx>,
    },
    Paid {
        amount: LeaseCoin,
        in_progress: Option<paid::ClosingTrx>,
    },
    Closed(),
    Liquidated(),
}

pub(crate) mod opening {
    #[cfg(feature = "skel_testing")]
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Serialize)]
    #[cfg_attr(
        feature = "skel_testing",
        derive(Clone, PartialEq, Eq, Deserialize, Debug)
    )]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub enum OngoingTrx {
        OpenIcaAccount,
        TransferOut { ica_account: String },
        BuyAsset { ica_account: String },
    }
}

pub(crate) mod opened {
    use finance::percent::Percent;
    #[cfg(feature = "skel_testing")]
    use serde::Deserialize;
    use serde::Serialize;

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

    #[cfg(feature = "contract")]
    impl ClosePolicy {
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
}

pub(crate) mod paid {
    #[cfg(feature = "skel_testing")]
    use serde::Deserialize;
    use serde::Serialize;

    #[derive(Serialize)]
    #[cfg_attr(
        feature = "skel_testing",
        derive(Clone, PartialEq, Eq, Debug, Deserialize)
    )]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub enum ClosingTrx {
        TransferInInit,
        TransferInFinish,
    }
}

#[cfg(all(feature = "internal.test.skel", test))]
mod test {
    use platform::tests as platform_tests;

    use super::QueryMsg;
    #[test]
    fn release() {
        assert_eq!(
            Ok(QueryMsg::ProtocolPackageRelease {}),
            platform_tests::ser_de(&versioning::query::ProtocolPackage::Release {}),
        );

        platform_tests::ser_de::<_, QueryMsg>(&versioning::query::PlatformPackage::Release {})
            .unwrap_err();
    }
}
