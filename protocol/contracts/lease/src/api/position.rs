use finance::percent::Percent;
use serde::{Deserialize, Serialize};

use super::LeaseCoin;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum PositionClose {
    FullClose(FullClose),
    PartialClose(PartialClose),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FullClose {}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PartialClose {
    pub amount: LeaseCoin,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ChangeCmd {
    Reset,
    Set(Percent),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ClosePolicyChange {
    pub stop_loss: Option<ChangeCmd>,
    pub take_profit: Option<ChangeCmd>,
}

#[cfg(all(feature = "internal.test.skel", test))]
mod test {
    use finance::percent::Percent;
    use sdk::cosmwasm_std;

    use crate::api::position::{ChangeCmd, ClosePolicyChange};

    #[test]
    fn sl_reset() {
        let msg = ClosePolicyChange {
            stop_loss: Some(ChangeCmd::Reset),
            take_profit: None,
        };
        const CLOSE_JSON: &str = "{ \"stop_loss\": \"reset\" }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }

    #[test]
    fn sl_set() {
        let msg = ClosePolicyChange {
            stop_loss: Some(ChangeCmd::Set(Percent::from_permille(123))),
            take_profit: None,
        };
        const CLOSE_JSON: &str = "{ \"stop_loss\": { \"set\": 123 } }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }

    #[test]
    fn tp_reset() {
        let msg = ClosePolicyChange {
            stop_loss: None,
            take_profit: Some(ChangeCmd::Reset),
        };
        const CLOSE_JSON: &str = "{ \"take_profit\": \"reset\" }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }

    #[test]
    fn tp_set() {
        let msg = ClosePolicyChange {
            stop_loss: None,
            take_profit: Some(ChangeCmd::Set(Percent::from_permille(321))),
        };
        const CLOSE_JSON: &str = "{ \"take_profit\": { \"set\": 321 } }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }

    #[test]
    fn sl_reset_tp_set() {
        let msg = ClosePolicyChange {
            stop_loss: Some(ChangeCmd::Reset),
            take_profit: Some(ChangeCmd::Set(Percent::from_permille(321))),
        };
        const CLOSE_JSON: &str = "{ \"stop_loss\": \"reset\", \"take_profit\": { \"set\": 321 } }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }

    #[test]
    fn sl_set_tp_reset() {
        let msg = ClosePolicyChange {
            stop_loss: Some(ChangeCmd::Set(Percent::from_permille(321))),
            take_profit: Some(ChangeCmd::Reset),
        };
        const CLOSE_JSON: &str = "{ \"stop_loss\": { \"set\": 321 }, \"take_profit\": \"reset\" }";
        assert_eq!(
            cosmwasm_std::from_json::<ClosePolicyChange>(&CLOSE_JSON)
                .expect("deserialization failed"),
            msg
        );
    }
}
