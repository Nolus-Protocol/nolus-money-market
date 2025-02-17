use serde::{Deserialize, Serialize};

use currencies::{LeaseGroup, PaymentGroup};
use finance::coin::CoinDTO;
use position::ClosePolicyChange;
use sdk::cosmwasm_std::Addr;

use self::position::PositionClose;

pub mod open;
pub mod position;
pub mod query;

pub(crate) type LeasePaymentCurrencies = PaymentGroup;
pub type PaymentCoin = CoinDTO<LeasePaymentCurrencies>;
pub type DownpaymentCoin = PaymentCoin;

// TODO this type is used predominantly in the contract implementation so consider
// deprecating it in favor of crate::finance::PriceG
pub(crate) type LeaseAssetCurrencies = LeaseGroup;
pub type LeaseCoin = CoinDTO<LeaseAssetCurrencies>;

pub type LpnCoinDTO = crate::finance::LpnCoinDTO;

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel_testing", derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Repayment
    ///
    /// The funds should be sent attached to the message
    Repay(),

    /// Change the Lease automatic close policy
    ///
    /// The lease owner can set Stop Loss, SL, or/and TakeProfit, TP, triggers after the lease has been fully opened.
    /// They may be set up and removed individually, or together until the lease is fully paid, fully closed, or fully liquidated.
    /// The trigger of SL and TP is defined as LTV percent such as 0 < TP% <= current LTV% < SL% < Liquidation%.
    /// Note that the Liquidation% is set on the Lease Open time and remains intact over the Lease Lifetime.
    ///
    /// A Full Close of the position occurs if:
    /// - SL is set and current LTV% >= SL% , or
    /// - TP is set and TP% > current LTV% .
    ///
    /// If the Liquidation% and SL% are surpassed simultaneously, and since the higher amount of liquidation and the stop-loss should be closed,
    /// the protocol should take the SL event with precedence and act accordingly.
    ///
    /// The full position close implies that a trigger is consumed and no longer valid once fired.
    ///
    /// It's worth noting that since TP and SL are meant to be triggered on price changes, not past liquidations or payments,
    /// the TP% is reset if a partial liquidation or a payment takes the position LTV below the TP%.
    ChangeClosePolicy(ClosePolicyChange),

    /// Customer initiated position close
    ///
    /// Return `error::ContractError::PositionCloseAmountTooSmall` when a partial close is requested
    /// with amount less than the minimum sell asset position parameter sent on lease open. Refer to
    /// `NewLeaseForm::position_spec`.
    ///
    /// Return `error::ContractError::PositionCloseAmountTooBig` when a partial close is requested
    /// with amount that would decrease a position less than the minimum asset parameter sent on
    /// lease open. Refer to `NewLeaseForm::position_spec`.
    ///
    /// Note that these checks would not be performed on the total position amount if
    /// a `PositionClose::FullClose` is requested. It is executed irrespective of the amount.
    ClosePosition(PositionClose),

    /// Close of a fully paid lease
    Close(),

    PriceAlarm(),
    TimeAlarm {},

    /// An entry point for safe delivery of a Dex response
    ///
    /// Invoked always by the same contract instance.
    DexCallback(),

    /// An entry point for safe delivery of an ICA open response, error or timeout
    ///
    /// Invoked always by the same contract instance.
    DexCallbackContinue(),

    /// Heal a lease past a middleware failure
    ///
    /// It cures a lease in the following cases:
    /// - on the final repay transaction, when an error, usually an out-of-gas, occurs on the Lpp's ExecuteMsg::RepayLoan sub-message
    /// - on the final repay transaction, when an error occurs on the Lease's SudoMsg::Response message
    Heal(),
}

/// The execute message any `Finalizer` should respond to.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "skel_testing", derive(Debug, PartialEq, Eq))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum FinalizerExecuteMsg {
    FinalizeLease { customer: Addr },
}

#[cfg(all(feature = "internal.test.skel", test))]
mod test {
    use sdk::cosmwasm_std::{from_json, to_json_vec};

    use crate::api::{
        position::{FullClose, PositionClose},
        ExecuteMsg,
    };

    #[test]
    fn test_repay_representation() {
        let msg = ExecuteMsg::Repay();
        let repay_bin = to_json_vec(&msg).expect("serialization failed");
        assert_eq!(msg, from_json(&repay_bin).expect("deserialization failed"),);

        assert_eq!(
            msg,
            from_json("{\"repay\":[]}").expect("deserialization failed")
        );
    }

    #[test]
    fn test_close_position_representation() {
        let msg = ExecuteMsg::ClosePosition(PositionClose::FullClose(FullClose {}));
        let close_bin = to_json_vec(&msg).expect("serialization failed");
        assert_eq!(msg, from_json(&close_bin).expect("deserialization failed"),);

        assert_eq!(
            msg,
            from_json("{\"close_position\":{\"full_close\":{}}}").expect("deserialization failed"),
        );
    }
}
