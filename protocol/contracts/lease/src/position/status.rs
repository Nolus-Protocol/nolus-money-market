use serde::{Deserialize, Serialize};

use finance::{coin::Coin, liability::Zone, percent::Percent};

use super::steady::Steadiness;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
pub enum Cause {
    Overdue(),
    Liability { ltv: Percent, healthy_ltv: Percent },
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
pub enum Liquidation<Asset> {
    Partial { amount: Coin<Asset>, cause: Cause },
    Full(Cause),
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "contract_testing", derive(Debug))]
pub enum Debt<Asset>
where
    Asset: 'static,
{
    No,
    /// Represent an open position with no immediate close required
    Ok {
        /// The position's debt results to an LTV% within the `liability` zone
        zone: Zone,
        /// The `steadiness`'s range is always a sub-range of the zone's range.
        steadiness: Steadiness<Asset>,
    },
    Bad(Liquidation<Asset>),
}

impl<Asset> Debt<Asset> {
    #[cfg(all(feature = "internal.test.contract", test))]
    pub(crate) fn partial(amount: Coin<Asset>, cause: Cause) -> Self {
        debug_assert!(!amount.is_zero());
        Self::Bad(Liquidation::Partial { amount, cause })
    }

    #[cfg(all(feature = "internal.test.contract", test))]
    pub(crate) fn full(cause: Cause) -> Self {
        Self::Bad(Liquidation::Full(cause))
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod test_status {
    use currencies::Lpn;
    use finance::percent::Percent;

    use crate::position::Liquidation;

    use super::Cause;

    #[test]
    fn ord_liq() {
        assert!(
            Liquidation::<Lpn>::Full(Cause::Overdue())
                < Liquidation::Full(Cause::Liability {
                    ltv: Percent::from_percent(20),
                    healthy_ltv: Percent::from_percent(40)
                })
        );
        assert!(
            Liquidation::<Lpn>::Full(Cause::Liability {
                ltv: Percent::from_percent(19),
                healthy_ltv: Percent::from_percent(40)
            }) < Liquidation::Full(Cause::Liability {
                ltv: Percent::from_percent(20),
                healthy_ltv: Percent::from_percent(40)
            })
        )
    }
}
