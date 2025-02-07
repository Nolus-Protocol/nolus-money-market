use serde::{Deserialize, Serialize};

use currency::CurrencyDTO;
use finance::{
    duration::{Duration, Seconds},
    percent::Percent,
};
use sdk::cosmwasm_std::Timestamp;

use crate::finance::LpnCoinDTO;

use super::{DownpaymentCoin, LeaseAssetCurrencies, LeaseCoin};

pub use self::opened::ClosePolicy;

pub(crate) mod opened;
pub(crate) mod opening;
pub(crate) mod paid;

#[derive(Deserialize)]
#[cfg_attr(
    any(test, feature = "testing"),
    derive(Clone, Debug, PartialEq, Serialize)
)]
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
    any(test, feature = "testing"),
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

#[cfg(test)]
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
