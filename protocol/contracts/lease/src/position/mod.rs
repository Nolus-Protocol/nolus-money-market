use currency::{Currency, CurrencyDef, MemberOf};
use finance::{coin::Coin, duration::Duration};

use crate::{
    api::{position::ClosePolicyChange, query::opened::ClosePolicy, LeasePaymentCurrencies},
    finance::Price,
};

pub use close::Strategy as CloseStrategy;
pub use dto::{PositionDTO, WithPosition, WithPositionResult};
pub use error::{Error as PositionError, Result as PositionResult};
pub use interest::{Due as DueTrait, OverdueCollection};
pub use spec::{Spec, SpecDTO};
pub(crate) use status::{Cause, Debt, Liquidation};
pub(crate) use steady::Steadiness;

mod close;
mod dto;
mod error;
mod interest;
mod spec;
mod status;
mod steady;

#[cfg_attr(feature = "contract_testing", derive(Debug))]
pub struct Position<Asset> {
    amount: Coin<Asset>,
    spec: Spec,
}

impl<Asset> Position<Asset>
where
    Asset: Currency,
{
    pub fn new(amount: Coin<Asset>, spec: Spec) -> Self {
        debug_assert!(!amount.is_zero(), "The amount should be positive");
        Self { amount, spec }
    }

    pub(crate) fn amount(&self) -> Coin<Asset> {
        self.amount
    }

    pub fn close(&mut self, asset: Coin<Asset>) {
        debug_assert!(
            asset < self.amount,
            "Liquidated asset {asset} should be less than the available {0}",
            self.amount
        );

        self.amount -= asset
    }

    /// Compute how much time is necessary for the due interest to become collectable
    ///
    /// If it is already enough to be collected then return zero.
    pub fn overdue_collection_in<Due>(&self, due: &Due) -> Duration
    where
        Due: DueTrait,
    {
        self.spec.overdue_collection_in(due)
    }

    /// Determine the debt status of a position
    ///
    /// Pre: `self.check_close(...) == None`
    pub fn debt<Due>(&self, due: &Due, asset_in_lpns: Price<Asset>) -> Debt<Asset>
    where
        Due: DueTrait,
    {
        self.spec.debt(self.amount, due, asset_in_lpns)
    }

    /// Export the close policy state for querying purposes
    ///
    /// Do not use it to implent any business logic!
    pub fn close_policy(&self) -> ClosePolicy {
        self.spec.close_policy()
    }

    /// Check if the position is subject of a full close due to trigerred close policy
    pub fn check_close<Due>(&self, due: &Due, asset_in_lpns: Price<Asset>) -> Option<CloseStrategy>
    where
        Due: DueTrait,
    {
        self.spec.check_close(self.amount, due, asset_in_lpns)
    }

    pub fn change_close_policy<Due>(
        &mut self,
        cmd: ClosePolicyChange,
        due: &Due,
        asset_in_lpns: Price<Asset>,
    ) -> PositionResult<()>
    where
        Asset: Currency,
        Due: DueTrait,
    {
        self.spec
            .change_close_policy(cmd, self.amount, due, asset_in_lpns)
            .map(|spec| {
                self.spec = spec;
            })
    }

    /// Check if the amount can be used for repayment.
    /// Return `error::PositionError::InsufficientTransactionAmount` when the payment amount
    /// is less than the minimum transaction amount.
    pub fn validate_payment<PaymentC>(
        &self,
        payment: Coin<PaymentC>,
        payment_currency_in_lpns: Price<PaymentC>,
    ) -> PositionResult<()>
    where
        PaymentC: CurrencyDef,
        PaymentC::Group: MemberOf<LeasePaymentCurrencies>,
    {
        self.spec
            .validate_payment(payment, payment_currency_in_lpns)
    }

    /// Check if the amount can be used to close the position.
    /// Return `error::PositionError::PositionCloseAmountTooSmall` when a partial close is requested
    /// with amount less than the minimum transaction position parameter sent on lease open. Refer to
    /// `NewLeaseForm::position_spec`.
    ///
    /// Return `error::PositionError::PositionCloseAmountTooBig` when a partial close is requested
    /// with amount that would decrease a position less than the minimum asset parameter sent on
    /// lease open. Refer to `NewLeaseForm::position_spec`.
    pub fn validate_close_amount(
        &self,
        close_amount: Coin<Asset>,
        asset_in_lpns: Price<Asset>,
    ) -> PositionResult<()> {
        self.spec
            .validate_close_amount(self.amount, close_amount, asset_in_lpns)
    }
}
