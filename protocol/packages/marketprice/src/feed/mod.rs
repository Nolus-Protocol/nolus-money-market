use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use currency::Currency;
use finance::{error::Error as FinanceError, fraction::Fraction, percent::Percent, price::Price};
use sdk::cosmwasm_std::{Addr, Timestamp};

use crate::{config::Config, error::PriceFeedsError, feed::sample::Sample};

use self::observation::Observation;

mod observation;
mod sample;

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
pub struct PriceFeed<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    observations: Vec<Observation<C, QuoteC>>,
}

impl<C, QuoteC> Default for PriceFeed<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    fn default() -> Self {
        Self {
            observations: vec![],
        }
    }
}

impl<C, QuoteC> PriceFeed<C, QuoteC>
where
    C: Currency,
    QuoteC: Currency,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_observation(
        mut self,
        from: Addr,
        at: Timestamp,
        price: Price<C, QuoteC>,
        valid_since: Timestamp,
    ) -> Self {
        debug_assert!(valid_since < at, "{valid_since} >= {at}");
        self.observations
            .retain(observation::valid_since(valid_since));

        self.observations.push(Observation::new(from, at, price));
        self
    }

    /// Calculate the price of this feed
    ///
    /// Provide no price if there are no observations from at least configurable percentage * <number_of_whitelisted_feeders>.
    /// Observations older than a configurable period are not taken into consideration.
    /// Calculate the price at a sample period as per the formula:
    /// discounting_factor * avg_price_during_the_period + (1 - discounting_factor) * price_at_the_previos_period
    pub fn calc_price(
        &self,
        config: &Config,
        at: Timestamp,
        total_feeders: usize,
    ) -> Result<Price<C, QuoteC>, PriceFeedsError> {
        let valid_since = config.feed_valid_since(at);
        if !self.has_enough_feeders(valid_since, config, total_feeders) {
            return Err(PriceFeedsError::NoPrice {});
        }

        let observations = self.valid_observations(valid_since);

        let samples = sample::from_observations(observations, valid_since, config.sample_period());

        let discount_factor = config.discount_factor();

        let samples_nb = config.samples_number().into();

        let mut sample_prices = samples
            .take(samples_nb)
            .map(Sample::into_maybe_price)
            .skip_while(Option::is_none)
            .map(|price| price.expect("sample prices should keep being present"));

        let first_price = sample_prices.next().ok_or(PriceFeedsError::NoPrice {})?;

        let final_price = sample_prices.try_fold(first_price, |acc, sample_price| {
            let discounted_price =
                discount_factor
                    .of(sample_price)
                    .ok_or(PriceFeedsError::Finance(FinanceError::overflow_err(
                        "in fraction calculation",
                        discount_factor,
                        sample_price,
                    )))?;
            let remaining_percentage = Percent::HUNDRED - discount_factor;
            let acc_part = remaining_percentage
                .of(acc)
                .ok_or(PriceFeedsError::Finance(FinanceError::overflow_err(
                    "in fraction calculation",
                    remaining_percentage,
                    acc,
                )))?;
            Ok(discounted_price + acc_part) as Result<_, PriceFeedsError>
        })?;

        Ok(final_price)
    }

    fn has_enough_feeders(&self, since: Timestamp, config: &Config, total_feeders: usize) -> bool {
        self.count_unique_feeders(since) >= config.min_feeders(total_feeders)
    }

    fn count_unique_feeders(&self, since: Timestamp) -> usize {
        self.valid_observations(since)
            .map(Observation::feeder)
            .collect::<HashSet<_>>()
            .len()
    }

    fn valid_observations(
        &self,
        since: Timestamp,
    ) -> impl Iterator<Item = &Observation<C, QuoteC>> {
        let mut valid_observations = observation::valid_since(since);
        self.observations
            .iter()
            .filter(move |&o| valid_observations(o))
    }
}

#[cfg(test)]
mod test {
    use currency::test::{SuperGroupTestC4, SuperGroupTestC5};
    use finance::{
        coin::{Amount, Coin},
        duration::Duration,
        percent::Percent,
        price::{self, Price},
    };
    use sdk::cosmwasm_std::{Addr, Timestamp};

    use crate::{config::Config, error::PriceFeedsError};

    use super::PriceFeed;

    const ONE_FEEDER: usize = 1;
    const SAMPLE_PERIOD: Duration = Duration::from_secs(5);
    const SAMPLES_NUMBER: u16 = 12;
    const VALIDITY: Duration = Duration::from_secs(60);
    const DISCOUNTING_FACTOR: Percent = Percent::from_permille(750);

    #[test]
    fn old_observations() {
        let block_time = Timestamp::from_seconds(100);
        let config = Config::new(
            Percent::HUNDRED,
            SAMPLE_PERIOD,
            SAMPLES_NUMBER,
            DISCOUNTING_FACTOR,
        );

        let feeder1 = Addr::unchecked("feeder1");
        let feed1_time = block_time - VALIDITY;
        let feed1_price = price(20, 5000);

        let mut feed = PriceFeed::new();
        feed = feed.add_observation(
            feeder1.clone(),
            feed1_time,
            feed1_price,
            config.feed_valid_since(feed1_time),
        );

        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feed.calc_price(&config, block_time, ONE_FEEDER)
        );

        let feed2_time = feed1_time + Duration::from_nanos(1);
        let feed2_price = price(19, 5000);
        feed = feed.add_observation(feeder1, feed2_time, feed2_price, feed1_time);
        assert_eq!(
            Ok(feed2_price),
            feed.calc_price(&config, block_time, ONE_FEEDER)
        );
    }

    #[test]
    fn less_feeders() {
        let validity_period = Duration::from_secs(60);
        let block_time = Timestamp::from_seconds(100);

        let feeder1 = Addr::unchecked("feeder1");
        let feed1_time = block_time;
        let feed1_price = price(20, 5000);

        let mut feed = PriceFeed::new();
        feed = feed.add_observation(
            feeder1,
            feed1_time,
            feed1_price,
            block_time - validity_period,
        );

        let config = Config::new(
            Percent::HUNDRED,
            SAMPLE_PERIOD,
            SAMPLES_NUMBER,
            DISCOUNTING_FACTOR,
        );
        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feed.calc_price(&config, block_time, ONE_FEEDER + ONE_FEEDER)
        );

        assert_eq!(
            Ok(feed1_price),
            feed.calc_price(&config, block_time, ONE_FEEDER)
        );
    }

    #[test]
    fn less_feeders_with_valid_observations() {
        let validity_period = Duration::from_secs(60);
        let block_time = Timestamp::from_seconds(150);

        let feeder1 = Addr::unchecked("feeder1");
        let feed1_time = block_time - validity_period;
        let feed1_price = price(19, 5100);

        let mut feed = PriceFeed::new();
        feed = feed.add_observation(
            feeder1,
            feed1_time,
            feed1_price,
            feed1_time - validity_period,
        );

        let feeder2 = Addr::unchecked("feeder2");
        let feed2_time = block_time - validity_period + Duration::from_nanos(1);
        let feed2_price = price(19, 5000);
        feed = feed.add_observation(
            feeder2,
            feed2_time,
            feed2_price,
            feed2_time - validity_period,
        );

        let config = Config::new(
            Percent::from_percent(50),
            SAMPLE_PERIOD,
            SAMPLES_NUMBER,
            DISCOUNTING_FACTOR,
        );
        assert_eq!(
            Ok(price(19, 5050)),
            feed.calc_price(&config, feed2_time, ONE_FEEDER + ONE_FEEDER)
        );
        assert_eq!(
            Ok(price(19, 5050)),
            feed.calc_price(
                &config,
                block_time - Duration::from_nanos(1),
                ONE_FEEDER + ONE_FEEDER
            )
        );
        assert_eq!(
            Ok(price(19, 5000)),
            feed.calc_price(&config, block_time, ONE_FEEDER + ONE_FEEDER)
        );

        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feed.calc_price(
                &config,
                block_time + Duration::from_nanos(1),
                ONE_FEEDER + ONE_FEEDER
            )
        );
    }

    #[test]
    fn ema_price() {
        let block_time = Timestamp::from_seconds(100);
        let config = Config::new(
            Percent::HUNDRED,
            SAMPLE_PERIOD,
            SAMPLES_NUMBER,
            DISCOUNTING_FACTOR,
        );

        let s1 = block_time - Duration::from_secs(12);
        let s21 = block_time - Duration::from_secs(7);
        let s22 = block_time - Duration::from_secs(6);
        let s3 = block_time - Duration::from_secs(2);

        let feeder1 = Addr::unchecked("feeder1");
        let feeder2 = Addr::unchecked("feeder2");

        let mut feed = PriceFeed::new();
        feed = feed.add_observation(
            feeder1.clone(),
            s1,
            price(19, 5160),
            config.feed_valid_since(s1),
        );
        feed = feed.add_observation(
            feeder1.clone(),
            s21,
            price(19, 5500),
            config.feed_valid_since(s21),
        );
        feed = feed.add_observation(
            feeder1.clone(),
            s22,
            price(19, 5000 + 10),
            config.feed_valid_since(s22),
        );
        feed = feed.add_observation(
            feeder2,
            s22,
            price(19, 5000 - 10),
            config.feed_valid_since(s22),
        );
        feed = feed.add_observation(feeder1, s3, price(19, 5000), config.feed_valid_since(s3));

        assert_eq!(
            Ok(price(19, 5010)),
            feed.calc_price(&config, block_time, ONE_FEEDER)
        );
    }

    fn price(c: Amount, q: Amount) -> Price<SuperGroupTestC4, SuperGroupTestC5> {
        price::total_of(Coin::from(c)).is(Coin::from(q))
    }
}
