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
