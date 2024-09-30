use currency::platform::{PlatformGroup, Stable};
use finance::{duration::Duration, error::Error as FinanceErr, interest, percent::Percent};
use lpp_platform::{CoinStable, Lpp as LppTrait};
use oracle_platform::{convert, Oracle, OracleRef};
use platform::message::Response as MessageResponse;

use crate::ContractError;

use super::Pool as PoolTrait;

pub struct Pool<Lpp, StableOracle> {
    lpp: Lpp,
    oracle: StableOracle,
    balance: CoinStable,
}

impl<Lpp, StableOracle> Pool<Lpp, StableOracle>
where
    Lpp: LppTrait,
    StableOracle: Oracle<PlatformGroup, QuoteC = Stable, QuoteG = PlatformGroup>
        + AsRef<OracleRef<StableOracle::QuoteC, StableOracle::QuoteG>>,
{
    pub fn new(lpp: Lpp, oracle: StableOracle) -> Result<Self, ContractError> {
        lpp.balance(oracle.as_ref().addr().clone())
            .map_err(ContractError::ReadLppBalance)
            .map(|balance| Self {
                lpp,
                oracle,
                balance,
            })
    }
}

impl<Lpp, StableOracle> PoolTrait for Pool<Lpp, StableOracle>
where
    Lpp: LppTrait,
    StableOracle: Oracle<PlatformGroup, QuoteC = Stable, QuoteG = PlatformGroup>,
{
    fn balance(&self) -> CoinStable {
        self.balance
    }

    fn distribute_rewards(
        self,
        apr: Percent,
        period: Duration,
    ) -> Result<MessageResponse, ContractError> {
        interest::interest(apr, self.balance, period)
            .ok_or(ContractError::InterestCalculation(FinanceErr::Overflow(
                format!(
                "Overflow occurred during interest calculation. APR: {}, Balance: {}, Period: {}",
                apr,
                self.balance,
                period
            ),
            )))
            .and_then(|reward_in_stable| {
                convert::from_quote::<_, _, _, _, PlatformGroup>(&self.oracle, reward_in_stable)
                    .map_err(ContractError::ConvertRewardsToNLS)
                    .and_then(|rewards| {
                        self.lpp
                            .distribute(rewards)
                            .map_err(ContractError::DistributeLppReward)
                    })
            })
    }
}

#[cfg(test)]
mod test {
    use currency::platform::Nls;
    use finance::{coin::Coin, duration::Duration, fraction::Fraction, percent::Percent, price};
    use lpp_platform::{test::DummyLpp, CoinStable};
    use oracle_platform::{test::DummyOracle, Oracle};
    use platform::response;

    use crate::{
        pool::{Pool, PoolImpl},
        ContractError,
    };

    #[test]
    fn failing_balance() {
        let oracle = DummyOracle::failing();
        let lpp = DummyLpp::failing_balance();
        assert!(matches!(
            PoolImpl::new(lpp, oracle),
            Err(ContractError::ReadLppBalance(_))
        ));
    }

    #[test]
    fn failing_nls_price() {
        let bar0_apr = Percent::from_percent(20);
        let lpp0_tvl: CoinStable = 15_000.into();

        let lpp = DummyLpp::with_balance(lpp0_tvl, Coin::<Nls>::default());
        let oracle = DummyOracle::failing();

        let pool = PoolImpl::new(lpp, oracle).unwrap();
        assert_eq!(lpp0_tvl, pool.balance());

        assert!(matches!(
            pool.distribute_rewards(bar0_apr, Duration::YEAR),
            Err(ContractError::ConvertRewardsToNLS(_))
        ));
    }

    #[test]
    fn failing_reward_distribution() {
        let bar0_apr = Percent::from_percent(20);
        let lpp0_tvl: CoinStable = 15_000.into();

        let oracle = DummyOracle::with_price(4);
        let exp_reward = price::total(
            bar0_apr.of(lpp0_tvl).unwrap(),
            oracle.price_of().unwrap().inv(),
        )
        .unwrap();
        let lpp = DummyLpp::failing_reward(lpp0_tvl, exp_reward);

        let pool = PoolImpl::new(lpp, oracle).unwrap();
        assert_eq!(lpp0_tvl, pool.balance());

        assert!(matches!(
            pool.distribute_rewards(bar0_apr, Duration::YEAR),
            Err(ContractError::DistributeLppReward(_))
        ));
    }

    #[test]
    fn ok() {
        let bar0_apr = Percent::from_percent(20);
        let lpp0_tvl: CoinStable = 23_000.into();
        let oracle = DummyOracle::with_price(2);
        let exp_reward = price::total(
            bar0_apr.of(lpp0_tvl).unwrap(),
            oracle.price_of().unwrap().inv(),
        )
        .unwrap();

        let pool = PoolImpl::new(DummyLpp::with_balance(lpp0_tvl, exp_reward), oracle).unwrap();
        assert_eq!(lpp0_tvl, pool.balance());

        let resp = response::response_only_messages(
            pool.distribute_rewards(bar0_apr, Duration::YEAR).unwrap(),
        );
        assert_eq!(resp.messages.len(), 1);
        assert_eq!(resp.events.len(), 1);
    }
}
