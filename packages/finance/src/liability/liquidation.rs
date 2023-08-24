use serde::{Deserialize, Serialize};

use crate::{coin::Coin, percent::Percent, zero::Zero};
use currency::Currency;

use super::Zone;

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub enum Cause {
    Overdue(),
    Liability { ltv: Percent, healthy_ltv: Percent },
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Debug))]
pub enum Status<Asset>
where
    Asset: Currency,
{
    NoDebt,
    No(Zone),
    Liquidation(Liquidation<Asset>),
}

impl<Asset> Status<Asset>
where
    Asset: Currency,
{
    pub fn partial(amount: Coin<Asset>, cause: Cause) -> Self {
        debug_assert!(amount != Coin::ZERO);
        Self::Liquidation(Liquidation::Partial { amount, cause })
    }

    pub fn full(cause: Cause) -> Self {
        Self::Liquidation(Liquidation::Full(cause))
    }

    #[cfg(debug_assertion)]
    fn amount<Lpn, Lpp, Profit, TimeAlarms, Oracle>(
        &self,
        lease: &Lease<Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>,
    ) -> Coin<Asset> {
        match self {
            Self::No(_) => Default::default(),
            Self::Liquidation(liq) => liq.amount(lease),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(test, derive(Debug))]
pub enum Liquidation<Asset>
where
    Asset: Currency,
{
    Partial { amount: Coin<Asset>, cause: Cause },
    Full(Cause),
}

impl<Asset> Liquidation<Asset>
where
    Asset: Currency,
{
    #[cfg(debug_assertion)]
    pub(crate) fn amount<Lpn, Lpp, Profit, TimeAlarms, Oracle>(
        &self,
        lease: &Lease<Lpn, Asset, Lpp, Profit, TimeAlarms, Oracle>,
    ) -> Coin<Asset> {
        match self {
            Self::Partial { amount, cause: _ } => *amount,
            Self::Full(_) => lease.amount,
        }
    }
}

pub fn may_ask_liquidation<Asset>(
    asset: Coin<Asset>,
    cause: Cause,
    liquidation: Coin<Asset>,
    min_liquidation: Coin<Asset>,
    min_asset: Coin<Asset>,
) -> Option<Status<Asset>>
where
    Asset: Currency,
{
    if liquidation.is_zero() || liquidation < min_liquidation {
        None
    } else if asset.saturating_sub(liquidation) <= min_asset {
        Some(Status::full(cause))
    } else {
        Some(Status::partial(liquidation, cause))
    }
}

#[cfg(test)]
mod test_status {
    use crate::percent::Percent;
    use currency::test::Usdc;

    use super::{Cause, Liquidation, Status, Zone};

    #[test]
    fn ord() {
        assert!(
            Status::<Usdc>::No(Zone::no_warnings(Percent::from_permille(1)))
                < Status::No(Zone::first(
                    Percent::from_permille(1),
                    Percent::from_permille(2)
                ))
        );
        assert!(
            Status::<Usdc>::No(Zone::first(
                Percent::from_permille(1),
                Percent::from_permille(2)
            )) < Status::No(Zone::second(
                Percent::from_permille(1),
                Percent::from_permille(2)
            ))
        );
        assert!(
            Status::<Usdc>::No(Zone::first(
                Percent::from_permille(1),
                Percent::from_permille(2)
            )) < Status::No(Zone::first(
                Percent::from_permille(1),
                Percent::from_permille(3)
            ))
        );
        assert!(
            Status::No(Zone::first(
                Percent::from_permille(2),
                Percent::from_permille(3)
            )) < Status::<Usdc>::No(Zone::second(
                Percent::from_permille(1),
                Percent::from_permille(2)
            ))
        );
        assert!(
            Status::No(Zone::third(
                Percent::from_permille(991),
                Percent::from_permille(1000)
            )) < Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 1.into(),
                cause: Cause::Overdue()
            })
        );
        assert!(
            Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 1.into(),
                cause: Cause::Overdue()
            }) < Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 1.into(),
                cause: Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                }
            })
        );
        assert!(
            Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 1.into(),
                cause: Cause::Overdue()
            }) < Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 2.into(),
                cause: Cause::Overdue()
            })
        );
        assert!(
            Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 1.into(),
                cause: Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                }
            }) < Status::<Usdc>::Liquidation(Liquidation::Partial {
                amount: 2.into(),
                cause: Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                }
            })
        );
        assert!(
            Status::<Usdc>::partial(
                1.into(),
                Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                }
            ) < Status::<Usdc>::partial(
                1.into(),
                Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(2)
                }
            )
        );
        assert!(
            Status::<Usdc>::partial(
                1.into(),
                Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                }
            ) < Status::<Usdc>::full(Cause::Liability {
                ltv: Percent::from_permille(1),
                healthy_ltv: Percent::from_permille(2)
            })
        );
        assert!(
            Status::<Usdc>::full(Cause::Liability {
                ltv: Percent::from_permille(1),
                healthy_ltv: Percent::from_permille(1)
            }) < Status::<Usdc>::full(Cause::Liability {
                ltv: Percent::from_permille(1),
                healthy_ltv: Percent::from_permille(2)
            })
        );
        assert!(
            Status::<Usdc>::full(Cause::Overdue())
                < Status::<Usdc>::full(Cause::Liability {
                    ltv: Percent::from_permille(1),
                    healthy_ltv: Percent::from_permille(1)
                })
        );
    }
}
