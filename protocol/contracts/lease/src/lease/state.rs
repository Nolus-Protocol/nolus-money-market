use finance::{coin::Coin, duration::Duration, percent::Percent};
use sdk::cosmwasm_std::Timestamp;

use crate::{api::query::opened::ClosePolicy, finance::LpnCoin};

#[cfg_attr(feature = "contract_testing", derive(Debug, Eq, PartialEq))]
pub struct State<Asset> {
    pub amount: Coin<Asset>,
    pub interest_rate: Percent,
    pub interest_rate_margin: Percent,
    pub principal_due: LpnCoin,
    pub overdue_margin: LpnCoin,
    pub overdue_interest: LpnCoin,
    pub overdue_collect_in: Duration,
    pub due_margin: LpnCoin,
    pub due_interest: LpnCoin,
    pub due_projection: Duration,
    // Intentionally not using the internal domain type close::Policy
    pub close_policy: ClosePolicy,
    pub validity: Timestamp,
}

impl<Asset> State<Asset> {
    pub fn total_due(&self) -> LpnCoin {
        self.principal_due
            + self.overdue_margin
            + self.overdue_interest
            + self.due_margin
            + self.due_interest
    }
}
