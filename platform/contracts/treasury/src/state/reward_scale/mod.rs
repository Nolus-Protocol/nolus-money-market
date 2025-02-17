use currency::Currency;
use serde::{Deserialize, Serialize};

use finance::{
    coin::{Amount, Coin},
    percent::Percent,
};
use sdk::cosmwasm_std::{StdError, StdResult};

mod unchecked;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TotalValueLocked(u32);

impl TotalValueLocked {
    pub const SCALE_FACTOR: Amount = 1_000_000_000;

    pub const fn new(thousands: u32) -> Self {
        Self(thousands)
    }

    pub fn as_coin<StableC>(&self) -> Coin<StableC>
    where
        StableC: Currency,
    {
        Amount::from(self.0)
            .checked_mul(Self::SCALE_FACTOR)
            .expect("Amount goes beyond calculation limits!")
            .into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Bar {
    pub tvl: TotalValueLocked,
    pub apr: Percent,
}

// A list of (minTVL_thousands: u32, APR%o) which defines the APR as per the TVL.
// The list represents intervals of TVL amounts starting from the given min TVL.
// A valid configuration shall include a pair with minTVL_thousands == 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "unchecked::RewardScale", into = "unchecked::RewardScale")]
pub struct RewardScale {
    bars: Vec<Bar>,
}

impl RewardScale {
    pub fn new(initial_apr: Percent) -> Self {
        RewardScale {
            bars: vec![Bar {
                tvl: Default::default(),
                apr: initial_apr,
            }],
        }
    }

    pub fn add_non_overlapping(self, bars: Vec<Bar>) -> StdResult<Self> {
        self.internal_add_non_overlapping::<false>(bars)
    }

    fn internal_add_non_overlapping<const NEW: bool>(
        mut self,
        mut bars: Vec<Bar>,
    ) -> StdResult<Self> {
        self.bars.append(&mut bars);

        self.bars.sort_unstable();

        if self
            .bars
            .iter()
            .zip(self.bars.iter().skip(1))
            .any(|(left, right)| left.tvl == right.tvl)
        {
            return Err(StdError::generic_err(if NEW {
                "Argument vector contains duplicates!"
            } else {
                "Argument vector contains duplicates of already defined bars!"
            }));
        }

        Ok(self)
    }

    pub fn get_apr<StableC>(&self, tvl_total: Coin<StableC>) -> Percent
    where
        StableC: Currency,
    {
        self.bars[self
            .bars
            .partition_point(|bar| bar.tvl.as_coin::<StableC>() <= tvl_total)
            .saturating_sub(1)]
        .apr
    }
}

impl TryFrom<Vec<Bar>> for RewardScale {
    type Error = StdError;

    fn try_from(bars: Vec<Bar>) -> Result<Self, Self::Error> {
        if bars.is_empty() {
            return Err(StdError::generic_err("Argument vector contains no bars!"));
        }

        if !bars.iter().any(|bar| bar.tvl == Default::default()) {
            return Err(StdError::generic_err("No zero TVL reward scale bar found!"));
        }

        Self { bars: vec![] }.internal_add_non_overlapping::<true>(bars)
    }
}

#[cfg(test)]
mod tests {
    use currency::test::SuperGroupTestC1;
    use finance::{
        coin::{Amount, Coin},
        percent::Percent,
    };

    use super::{Bar, RewardScale, TotalValueLocked};

    #[test]
    fn rewards_scale_new() {
        let cfg = RewardScale::new(Percent::from_permille(6));
        let initial = cfg.bars.first().unwrap();
        assert_eq!(initial.tvl, Default::default());
        assert_eq!(initial.apr, Percent::from_permille(6));
        assert_eq!(cfg.bars.len(), 1);
    }

    #[test]
    fn rewards_from() {
        let _ = RewardScale::try_from(vec![]).unwrap_err();

        let _ = RewardScale::try_from(vec![Bar {
            tvl: TotalValueLocked::new(30),
            apr: Percent::from_permille(6),
        }])
        .unwrap_err();

        let res = RewardScale::try_from(vec![
            Bar {
                tvl: Default::default(),
                apr: Percent::from_permille(6),
            },
            Bar {
                tvl: TotalValueLocked::new(30),
                apr: Percent::from_permille(10),
            },
        ])
        .unwrap();

        assert_eq!(res.bars.len(), 2);
        assert_eq!(res.bars[0].tvl, Default::default());
        assert_eq!(res.bars[0].apr, Percent::from_permille(6));
        assert_eq!(res.bars[1].tvl, TotalValueLocked::new(30));
        assert_eq!(res.bars[1].apr, Percent::from_permille(10));
    }

    #[test]
    fn rewards_scale_get_apr() {
        let res = RewardScale::try_from(vec![
            Bar {
                tvl: Default::default(),
                apr: Percent::from_permille(6),
            },
            Bar {
                tvl: TotalValueLocked::new(30),
                apr: Percent::from_permille(10),
            },
            Bar {
                tvl: TotalValueLocked::new(150),
                apr: Percent::from_permille(15),
            },
            Bar {
                tvl: TotalValueLocked::new(300),
                apr: Percent::from_permille(20),
            },
            Bar {
                tvl: TotalValueLocked::new(100),
                apr: Percent::from_permille(12),
            },
        ])
        .unwrap();

        assert_eq!(
            res.get_apr::<SuperGroupTestC1>(0.into()),
            Percent::from_permille(6)
        );
        assert_eq!(
            res.get_apr(coin(TotalValueLocked::SCALE_FACTOR)),
            Percent::from_permille(6)
        );
        assert_eq!(
            res.get_apr(coin(30 * TotalValueLocked::SCALE_FACTOR - 1)),
            Percent::from_permille(6)
        );
        assert_eq!(
            res.get_apr(coin(30 * TotalValueLocked::SCALE_FACTOR)),
            Percent::from_permille(10)
        );
        assert_eq!(
            res.get_apr(coin(30 * TotalValueLocked::SCALE_FACTOR + 1)),
            Percent::from_permille(10)
        );
        assert_eq!(
            res.get_apr(coin(100 * TotalValueLocked::SCALE_FACTOR + 1)),
            Percent::from_permille(12)
        );
        assert_eq!(
            res.get_apr(coin(150 * TotalValueLocked::SCALE_FACTOR - 1)),
            Percent::from_permille(12)
        );
        assert_eq!(
            res.get_apr(coin(150 * TotalValueLocked::SCALE_FACTOR)),
            Percent::from_permille(15)
        );
        assert_eq!(
            res.get_apr(coin(200 * TotalValueLocked::SCALE_FACTOR)),
            Percent::from_permille(15)
        );
        assert_eq!(
            res.get_apr(coin(300 * TotalValueLocked::SCALE_FACTOR)),
            Percent::from_permille(20)
        );
        assert_eq!(
            res.get_apr(coin(300 * TotalValueLocked::SCALE_FACTOR + 1)),
            Percent::from_permille(20)
        );
        assert_eq!(
            res.get_apr(coin(1300 * TotalValueLocked::SCALE_FACTOR + 1)),
            Percent::from_permille(20)
        );
        assert_eq!(res.get_apr(coin(Amount::MAX)), Percent::from_permille(20));
        assert_eq!(res.get_apr(coin(Amount::MIN)), Percent::from_permille(6));
    }

    fn coin(amount: Amount) -> Coin<SuperGroupTestC1> {
        amount.into()
    }
}
