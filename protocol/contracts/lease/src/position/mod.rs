use currency::{Currency, CurrencyDef, MemberOf};
use finance::{
    coin::Coin, duration::Duration, error::Error as FinanceError, fraction::Fraction,
    liability::Level, price::total_of,
};

use crate::{
    api::LeasePaymentCurrencies,
    error::{ContractError, ContractResult},
    finance::{LpnCoin, Price},
};

pub use dto::{PositionDTO, WithPosition, WithPositionResult};
pub use interest::{Due as DueTrait, OverdueCollection};
pub use spec::Spec;
pub use status::{Cause, Debt, Liquidation};

mod dto;
mod interest;
mod spec;
mod status;

#[cfg_attr(test, derive(Debug))]
pub struct Position<Asset> {
    amount: Coin<Asset>,
    spec: Spec,
}

impl<Asset> Position<Asset>
where
    Asset: Currency,
{
    pub fn new(amount: Coin<Asset>, spec: Spec) -> Self {
        let obj = Self { amount, spec };
        debug_assert_eq!(Ok(()), obj.invariant_held());
        obj
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
    pub fn overdue_collection_in<Due>(&self, due: &Due) -> Option<Duration>
    where
        Due: DueTrait,
    {
        self.spec.overdue_collection_in(due)
    }

    pub fn debt<Due>(&self, due: &Due, asset_in_lpns: Price<Asset>) -> Option<Debt<Asset>>
    where
        Due: DueTrait,
    {
        self.spec.debt(self.amount, due, asset_in_lpns)
    }

    /// Check if the amount can be used for repayment.
    /// Return `error::ContractError::InsufficientPayment` when the payment amount
    /// is less than the minimum transaction amount.
    pub fn validate_payment<PaymentC>(
        &self,
        payment: Coin<PaymentC>,
        payment_currency_in_lpns: Price<PaymentC>,
    ) -> ContractResult<()>
    where
        PaymentC: CurrencyDef,
        PaymentC::Group: MemberOf<LeasePaymentCurrencies>,
    {
        self.spec
            .validate_payment(payment, payment_currency_in_lpns)
    }

    /// Check if the amount can be used to close the position.
    /// Return `error::ContractError::PositionCloseAmountTooSmall` when a partial close is requested
    /// with amount less than the minimum transaction position parameter sent on lease open. Refer to
    /// `NewLeaseForm::position_spec`.
    ///
    /// Return `error::ContractError::PositionCloseAmountTooBig` when a partial close is requested
    /// with amount that would decrease a position less than the minimum asset parameter sent on
    /// lease open. Refer to `NewLeaseForm::position_spec`.
    pub fn validate_close_amount(
        &self,
        close_amount: Coin<Asset>,
        asset_in_lpns: Price<Asset>,
    ) -> ContractResult<()> {
        self.spec
            .validate_close_amount(self.amount, close_amount, asset_in_lpns)
    }

    /// Calculate the price at which the lease reaches given ltv.
    pub(crate) fn price_at(
        &self,
        level: Level,
        total_due: LpnCoin,
    ) -> ContractResult<Price<Asset>> {
        debug_assert!(!total_due.is_zero());
        debug_assert!(!level.ltv().is_zero());

        level
            .ltv()
            .of(self.amount)
            .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                "in fraction calculation",
                level.ltv(),
                self.amount,
            )))
            .map(|amount| total_of(amount).is(total_due))
    }

    fn invariant_held(&self) -> ContractResult<()> {
        Self::check(!self.amount.is_zero(), "The amount should be positive")
    }

    fn check(invariant: bool, msg: &str) -> ContractResult<()> {
        ContractError::broken_invariant_if::<Self>(!invariant, msg)
    }
}
