use platform::{bank::FixedAddressSender, batch::Batch};

use currency::{CurrencyDef, MemberOf};
use finance::{coin::Coin, error::Error as FinanceError};
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle_platform::Oracle as OracleTrait;
use reserve::stub::Reserve as ReserveTrait;
use sdk::cosmwasm_std::Timestamp;

use crate::{
    api::{LeaseAssetCurrencies, LeasePaymentCurrencies},
    error::{ContractError, ContractResult},
    finance::{LpnCoin, LpnCurrencies, LpnCurrency, OracleRef},
    lease::Lease,
    loan::RepayReceipt,
};

pub(crate) struct FullRepayReceipt {
    receipt: RepayReceipt,
    messages: Batch,
}

impl FullRepayReceipt {
    fn new(receipt: RepayReceipt, messages: Batch) -> Self {
        debug_assert!(receipt.close());
        Self { receipt, messages }
    }

    pub(crate) fn decompose(self) -> (RepayReceipt, Batch) {
        (self.receipt, self.messages)
    }
}

impl<Asset, Lpp, Oracle> Lease<Asset, Lpp, Oracle>
where
    Lpp: LppLoanTrait<LpnCurrency, LpnCurrencies>,
    Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>
        + Into<OracleRef>,
    Asset: CurrencyDef,
    Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
{
    pub(crate) fn validate_close(&self, amount: Coin<Asset>) -> ContractResult<()> {
        self.price_of_lease_currency()
            .and_then(|asset_in_lpns| self.position.validate_close_amount(amount, asset_in_lpns))
    }

    pub(crate) fn close_partial<Profit>(
        &mut self,
        asset: Coin<Asset>,
        payment: LpnCoin,
        now: &Timestamp,
        profit: &mut Profit,
    ) -> ContractResult<RepayReceipt>
    where
        Profit: FixedAddressSender,
    {
        self.position.close(asset);
        self.repay(payment, now, profit)
    }

    pub(crate) fn close_full<Profit, Reserve, Change>(
        mut self,
        payment: LpnCoin,
        now: Timestamp,
        mut profit: Profit,
        mut reserve: Reserve,
        mut change_recipient: Change,
    ) -> ContractResult<FullRepayReceipt>
    where
        Profit: FixedAddressSender,
        Change: FixedAddressSender,
        Reserve: ReserveTrait<LpnCurrency>,
        ContractError: From<Reserve::Error>,
    {
        self.state(now)
            .ok_or(ContractError::FinanceError(FinanceError::Overflow(
                format!(
                    "Failed to calculate the lease state at the specified time: {:?}",
                    now
                ),
            )))
            .and_then(|state| {
                let total_due = state.total_due();
                let payment = if total_due > payment {
                    reserve.cover_liquidation_losses(total_due - payment);
                    total_due
                } else {
                    payment
                };
                let receipt = self.repay(payment, &now, &mut profit)?;
                debug_assert!(receipt.close());

                change_recipient.send(receipt.change());

                reserve
                    .try_into()
                    .map_err(Into::into)
                    .and_then(|reserve_messages| {
                        self.try_into_messages().map(|lease_messages| {
                            FullRepayReceipt::new(
                                receipt,
                                reserve_messages
                                    .merge(lease_messages) // these should go *after* any reserve messages as to allow for covering losses
                                    .merge(profit.into())
                                    .merge(change_recipient.into()),
                            )
                        })
                    })
            })
    }
}
