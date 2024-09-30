use currency::{CurrencyDef, MemberOf};
use finance::error::Error as FinanceError;
use lpp::stub::loan::LppLoan as LppLoanTrait;
use oracle_platform::Oracle as OracleTrait;
use sdk::cosmwasm_std::Timestamp;

use crate::{
    api::{
        query::{opened::OngoingTrx, StateResponse},
        LeaseAssetCurrencies, LeasePaymentCurrencies,
    },
    error::ContractError,
    finance::{LpnCurrencies, LpnCurrency},
    lease::{with_lease::WithLease, Lease},
};

pub struct LeaseState {
    now: Timestamp,
    in_progress: Option<OngoingTrx>,
}

impl LeaseState {
    pub fn new(now: Timestamp, in_progress: Option<OngoingTrx>) -> Self {
        Self { now, in_progress }
    }
}

impl WithLease for LeaseState {
    type Output = StateResponse;

    type Error = ContractError;

    fn exec<Asset, LppLoan, Oracle>(
        self,
        lease: Lease<Asset, LppLoan, Oracle>,
    ) -> Result<Self::Output, Self::Error>
    where
        Asset: CurrencyDef,
        Asset::Group: MemberOf<LeaseAssetCurrencies> + MemberOf<LeasePaymentCurrencies>,
        LppLoan: LppLoanTrait<LpnCurrency, LpnCurrencies>,
        Oracle: OracleTrait<LeasePaymentCurrencies, QuoteC = LpnCurrency, QuoteG = LpnCurrencies>,
    {
        lease
            .state(self.now)
            .ok_or(ContractError::FinanceError(FinanceError::Overflow(
                format!(
                    "Failed to calculate the lease state at the specified time: {:?}",
                    self.now
                ),
            )))
            .map(|open_lease| StateResponse::opened_from(open_lease, self.in_progress))
    }
}
