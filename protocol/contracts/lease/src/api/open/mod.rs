#[cfg(feature = "skel")]
use serde::Deserialize;
use serde::Serialize;

use currency::CurrencyDTO;
pub use dex::{ConnectionParams, Ics20Channel};
use finance::{duration::Duration, liability::Liability, percent::Percent};
use sdk::cosmwasm_std::Addr;

#[cfg(feature = "skel")]
use crate::error_de::ErrorDe;
use crate::finance::LpnCoinDTO;

use super::LeaseAssetCurrencies;

#[cfg(feature = "skel")]
mod unchecked;

#[derive(Serialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel", derive(Deserialize))]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct NewLeaseContract {
    /// An application form for opening a new lease
    pub form: NewLeaseForm,
    /// Connection parameters of a Dex capable to perform currency swaps
    pub dex: ConnectionParams,
    /// A contract to be notified when a lease just went into a final state
    ///
    /// The finalizer API should provide all `FinalizerExecuteMsg` variants.
    pub finalizer: Addr,
}

#[derive(Serialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel", derive(Deserialize))]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct NewLeaseForm {
    /// The customer who wants to open a lease.
    pub customer: Addr,
    /// Ticker of the currency this lease will be about.
    pub currency: CurrencyDTO<LeaseAssetCurrencies>,
    /// Maximum Loan-to-Downpayment percentage of the new lease, optional.
    pub max_ltd: Option<Percent>,
    /// Position parameters
    pub position_spec: PositionSpecDTO,
    /// Loan parameters
    pub loan: LoanForm,
    // TODO[all Addr contract parameters passed on opening] migrate to using their respective *Ref-s
    // Although being external for the contract, this API is internal for the system.
    /// The Reserve contract that would cover losses
    pub reserve: Addr,
    /// The time alarms contract the lease uses to get time notifications
    pub time_alarms: Addr,
    /// The oracle contract that sends market price alerts to the lease
    pub market_price_oracle: Addr,
}

#[derive(Serialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "skel", derive(Deserialize))]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
/// The value remains intact.
pub struct LoanForm {
    /// The Liquidity Provider Pool, LPP, that lends the necessary amount for this lease.
    pub lpp: Addr,
    /// The Profit contract to which the margin interest is sent.
    pub profit: Addr,
    /// The delta added on top of the LPP Loan interest rate.
    ///
    /// The amount, a part of any payment, goes to the Profit contract.
    pub annual_margin_interest: Percent,
    /// How long the accrued interest is due before getting overdue.
    pub due_period: Duration,
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "skel",
    derive(Deserialize),
    serde(deny_unknown_fields, try_from = "unchecked::PositionSpecDTO")
)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(rename_all = "snake_case")]
pub struct PositionSpecDTO {
    /// Liability constraints
    pub liability: Liability,
    ///  The minimum amount that a lease asset should be evaluated past any
    ///  partial liquidation or close. If not, a full liquidation is performed
    pub min_asset: LpnCoinDTO,
    /// The minimum amount to liquidate or close. Any attempt to liquidate a smaller
    /// amount would be postponed until the amount goes above this limit
    pub min_transaction: LpnCoinDTO,
}

#[cfg(feature = "skel")]
impl PositionSpecDTO {
    fn invariant_held(&self) -> Result<(), ErrorDe> {
        Self::check(
            !self.min_asset.is_zero(),
            "Min asset amount should be positive",
        )
        .and(Self::check(
            !self.min_transaction.is_zero(),
            "Min transaction amount should be positive",
        ))
        .and(Self::check(
            self.min_asset.currency() == self.min_transaction.currency(),
            "The currency of min asset should be the same as the currency of min transaction",
        ))
    }

    fn check(invariant: bool, msg: &str) -> Result<(), ErrorDe> {
        ErrorDe::broken_invariant_if::<Self>(!invariant, msg)
    }
}

#[cfg(feature = "skel")]
impl PositionSpecDTO {
    #[cfg(feature = "contract")]
    pub(crate) fn new_internal(
        liability: Liability,
        min_asset: LpnCoinDTO,
        min_transaction: LpnCoinDTO,
    ) -> Self {
        Self::new_unchecked(liability, min_asset, min_transaction)
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn new(liability: Liability, min_asset: LpnCoinDTO, min_transaction: LpnCoinDTO) -> Self {
        let obj = Self::new_unchecked(liability, min_asset, min_transaction);
        obj.invariant_held()
            .expect("PositionSpecDTO invariant to be held");
        obj
    }

    #[cfg(any(test, feature = "testing", feature = "contract"))]
    fn new_unchecked(
        liability: Liability,
        min_asset: LpnCoinDTO,
        min_transaction: LpnCoinDTO,
    ) -> Self {
        let obj = Self {
            liability,
            min_asset,
            min_transaction,
        };
        debug_assert_eq!(Ok(()), obj.invariant_held());
        obj
    }
}

#[cfg(all(test, feature = "skel"))]
mod test_position_spec {
    use currencies::Lpn;
    use currency::CurrencyDef;
    use finance::{coin::Coin, duration::Duration, liability::Liability, percent::Percent};
    use sdk::cosmwasm_std::{from_json, StdError};

    use super::PositionSpecDTO;

    type LpnCoin = Coin<Lpn>;

    #[test]
    fn new_valid() {
        assert_load_ok(
            spec_dto(),
            format!(
                r#"{{"liability":{{"initial":650,"healthy":700,"first_liq_warn":730,"second_liq_warn":750,"third_liq_warn":780,"max":800,"recalc_time":3600000000000}},"min_asset":{{"amount":"9000000","ticker":"{lpn}"}},"min_transaction":{{"amount":"5000","ticker":"{lpn}"}}}}"#,
                lpn = Lpn::ticker()
            ),
        );
    }

    #[test]
    fn zero_min_asset() {
        let r = from_json(format!(
            r#"{{"liability":{{"initial":650,"healthy":700,"first_liq_warn":730,"second_liq_warn":750,"third_liq_warn":780,"max":800,"recalc_time":3600000000000}},"min_asset":{{"amount":"0","ticker":"{lpn}"}},"min_transaction":{{"amount":"5000","ticker":"{lpn}"}}}}"#,
            lpn = Lpn::ticker()
        ));
        assert_err(r, "should be positive");
    }

    #[test]
    fn zero_min_transaction() {
        let r = from_json(format!(
            r#"{{"liability":{{"initial":650,"healthy":700,"first_liq_warn":730,"second_liq_warn":750,"third_liq_warn":780,"max":800,"recalc_time":3600000000000}},"min_asset":{{"amount":"9000000","ticker":"{lpn}"}},"min_transaction":{{"amount":"0","ticker":"{lpn}"}}}}"#,
            lpn = Lpn::ticker()
        ));
        assert_err(r, "should be positive");
    }

    #[test]
    fn invalid_ticker() {
        let r = from_json(format!(
            r#"{{"liability":{{"initial":650,"healthy":700,"first_liq_warn":730,"second_liq_warn":750,"third_liq_warn":780,"max":800,"recalc_time":3600000000000}},"min_asset":{{"amount":"9000000","ticker":"{lpn}"}},"min_transaction":{{"amount":"5000","ticker":"ATOM"}}}}"#,
            lpn = Lpn::ticker()
        ));
        assert_err(r, "'ATOM' pretending to be");
    }

    fn assert_load_ok<Json>(exp: PositionSpecDTO, json: Json)
    where
        Json: AsRef<[u8]>,
    {
        assert_eq!(Ok(exp), from_json::<PositionSpecDTO>(json));
    }

    fn assert_err(r: Result<PositionSpecDTO, StdError>, msg: &str) {
        assert!(
            matches!( // TODO migrate to using assert_matches!() once stabilized
                r,
                Err(StdError::ParseErr {
                    target_type,
                    msg: real_msg,
                    backtrace: _,
                }) if target_type.contains("PositionSpec") && real_msg.contains(msg)
            )
        );
    }

    fn spec_dto() -> PositionSpecDTO {
        let liability = Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(70),
            Percent::from_percent(73),
            Percent::from_percent(75),
            Percent::from_percent(78),
            Percent::from_percent(80),
            Duration::from_hours(1),
        );
        PositionSpecDTO::new(
            liability,
            LpnCoin::new(9000000).into(),
            LpnCoin::new(5000).into(),
        )
    }
}
