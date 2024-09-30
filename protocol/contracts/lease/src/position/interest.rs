use std::fmt::{Display, Formatter, Result};

use finance::{coin::Coin, duration::Duration, zero::Zero};

use crate::finance::LpnCoin;

/// Represent the due of a position
pub trait Due {
    /// The total due amount
    ///
    /// Includes the principal and due interest.
    /// The position stays open until there is some due amount.
    fn total_due(&self) -> LpnCoin;

    /// When and how much due interest to collect
    ///
    /// Usually, the interest is accrued as per a fixed interest rate.
    /// If the accrued interest is not paid until some configured amount of time elapses it becomes overdue.
    /// When overdue interest amount goes above a configured minimum then the interest becomes collectable.
    fn overdue_collection(&self, min_amount: LpnCoin) -> Option<OverdueCollection>;
}

#[derive(PartialEq, Debug)]
pub enum OverdueCollection {
    /// No collectable overdue interest yet
    ///
    /// The period specifies in how much time the overdue will become collectable.
    /// Non-zero value.
    StartIn(Duration),

    /// The overdue amount to be collected
    ///
    /// The amounts accrued since the overdue period has started.
    Overdue(LpnCoin),
}

impl OverdueCollection {
    pub fn start_in(&self) -> Duration {
        match self {
            OverdueCollection::StartIn(collect_in) => *collect_in,
            OverdueCollection::Overdue(_) => Duration::default(),
        }
    }

    pub fn amount(&self) -> LpnCoin {
        match self {
            OverdueCollection::StartIn(_) => Coin::ZERO,
            OverdueCollection::Overdue(amount) => *amount,
        }
    }
}

impl Display for OverdueCollection {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            OverdueCollection::StartIn(duration) => {
                write!(f, "Start in: {}", duration)
            }
            OverdueCollection::Overdue(coin) => {
                write!(f, "Overdue amount: {}", coin)
            }
        }
    }
}
