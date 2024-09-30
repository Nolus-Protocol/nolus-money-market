use serde::{Deserialize, Serialize};

use finance::{duration::Duration, fraction::Fraction, percent::Percent};
use sdk::{
    cosmwasm_std::Timestamp,
    schemars::{self, JsonSchema},
};

use crate::error::{self, PriceFeedsError};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(try_from = "unchecked::Config")]
#[serde(into = "unchecked::Config")]
pub struct Config {
    min_feeders: Percent,
    sample_period: Duration,
    /// The number of samples to take into account on price calculation
    ///
    /// It is not `usize` due to a generated float instruction in the Wasm32 output.
    samples_number: u16,
    /// transient property equals to `sample_period` * `samples_number`
    feed_validity: Duration,
    discount_factor: Percent,
}

impl Config {
    #[cfg(any(test, feature = "testing"))]
    pub fn new(
        min_feeders: Percent,
        sample_period: Duration,
        samples_number: u16,
        discount_factor: Percent,
    ) -> Self {
        Self::new_private(min_feeders, sample_period, samples_number, discount_factor)
            .expect("reasonable input test data")
    }

    fn new_private(
        min_feeders: Percent,
        sample_period: Duration,
        samples_number: u16,
        discount_factor: Percent,
    ) -> Result<Self, PriceFeedsError> {
        if let Some(feed_validity) = sample_period.checked_mul(samples_number) {
            Self {
                min_feeders,
                sample_period,
                samples_number,
                feed_validity,
                discount_factor,
            }
            .check_invariant()
        } else {
            Err(PriceFeedsError::Configuration(
                "Overflow multiplying sample period by samples number".into(),
            ))
        }
    }

    pub fn min_feeders(&self, total: usize) -> usize {
        self.min_feeders
            .of(total)
            .expect("Expected valid min_feeders")
    }

    pub fn sample_period(&self) -> Duration {
        self.sample_period
    }

    pub fn samples_number(&self) -> u16 {
        self.samples_number
    }

    pub fn feed_valid_since(&self, now: Timestamp) -> Timestamp {
        debug_assert!(now > Timestamp::default());

        let ret = if Timestamp::default() + self.feed_validity <= now {
            now - self.feed_validity
        } else {
            Timestamp::default()
        };
        debug_assert!(ret < now, "{ret} >= {now}");
        ret
    }

    pub fn discount_factor(&self) -> Percent {
        self.discount_factor
    }

    fn check_invariant(self) -> Result<Self, PriceFeedsError> {
        error::config_error_if(
            self.min_feeders == Percent::ZERO || self.min_feeders > Percent::HUNDRED,
            "The minumum feeders should be greater than 0 and less or equal to 100%",
        )?;

        error::config_error_if(
            self.sample_period == Duration::default(),
            "The sample period should be longer than zero",
        )?;

        error::config_error_if(
            self.samples_number == u16::default(),
            "The price feeds validity should be longer than zero",
        )?;

        debug_assert!(self.feed_validity > Duration::default());
        debug_assert!(self.sample_period <= self.feed_validity);

        error::config_error_if(
            self.discount_factor == Percent::ZERO || self.discount_factor > Percent::HUNDRED,
            "The discounting factor should be greater than 0 and less or equal to 100%",
        )?;

        Ok(self)
    }
}

mod unchecked {
    use serde::{Deserialize, Serialize};

    use finance::{duration::Duration, percent::Percent};

    use crate::error::PriceFeedsError;

    use super::Config as ValidatedConfig;

    #[derive(Serialize, Deserialize)]
    pub(super) struct Config {
        min_feeders: Percent,
        sample_period_secs: u32,
        samples_number: u16,
        discount_factor: Percent,
    }

    impl From<ValidatedConfig> for Config {
        fn from(o: ValidatedConfig) -> Self {
            Self {
                min_feeders: o.min_feeders,
                sample_period_secs: expect_u32_secs(
                    o.sample_period,
                    "Programming error! The sample period has been increased!",
                ),
                samples_number: o.samples_number,
                discount_factor: o.discount_factor,
            }
        }
    }

    impl TryFrom<Config> for ValidatedConfig {
        type Error = PriceFeedsError;

        fn try_from(dto: Config) -> Result<Self, Self::Error> {
            Self::new_private(
                dto.min_feeders,
                Duration::from_secs(dto.sample_period_secs),
                dto.samples_number,
                dto.discount_factor,
            )
        }
    }

    fn expect_u32_secs(d: Duration, descr: &str) -> u32 {
        d.secs().try_into().expect(descr)
    }
}

#[cfg(test)]
mod test {
    use finance::{duration::Duration, percent::Percent};
    use sdk::cosmwasm_std::{from_json, to_json_vec, StdError, Timestamp};

    use crate::config::Config;

    #[test]
    fn feed_valid_since() {
        let c = Config::new(
            Percent::from_permille(1),
            Duration::from_secs(5),
            12,
            Percent::from_permille(1000),
        );
        assert_eq!(
            Timestamp::from_seconds(0),
            c.feed_valid_since(Timestamp::from_seconds(1))
        );
        assert_eq!(
            Timestamp::from_seconds(0),
            c.feed_valid_since(Timestamp::from_seconds(40))
        );
        assert_eq!(
            Timestamp::from_seconds(0),
            c.feed_valid_since(Timestamp::from_seconds(60))
        );
        assert_eq!(
            Timestamp::from_seconds(1),
            c.feed_valid_since(Timestamp::from_seconds(61))
        );
        assert_eq!(
            Timestamp::from_seconds(40),
            c.feed_valid_since(Timestamp::from_seconds(100))
        );
    }

    fn min_feders_impl(min_feeders: u16, total: usize, exp: usize) {
        let c = Config::new(
            Percent::from_percent(min_feeders),
            Duration::HOUR,
            1,
            Percent::from_percent(75),
        );
        assert_eq!(exp, c.min_feeders(total));
    }
    #[test]
    fn feeders_needed_rounds_properly() {
        min_feders_impl(100, 3, 3);
        min_feders_impl(100, 30, 30);
        min_feders_impl(50, 34, 17);
        min_feders_impl(50, 33, 16);

        min_feders_impl(25, 48, 12);
        min_feders_impl(25, 49, 12);
        min_feders_impl(25, 50, 12);
        min_feders_impl(25, 51, 12);
        min_feders_impl(25, 52, 13);

        min_feders_impl(1, 132, 1);
        min_feders_impl(1, 199, 1);
        min_feders_impl(1, 200, 2);
    }

    #[test]
    fn deserialize_invalid() {
        deserialize_fail(0, 10, 42, 22);
        deserialize_fail(1001, 10, 42, 22);
        deserialize_fail(10, 0, 42, 22);
        deserialize_fail(10, 43, 0, 22);
        deserialize_fail(10, 0, 0, 22);
        deserialize_fail(10, 43, 42, 0);
        deserialize_fail(10, 43, 42, 1001);
        deserialize_fail(1001, 10, 42, 22);
        deserialize_fail(10, 10, 42, 42122);
    }

    #[test]
    fn deserialize_valid() {
        deserialize_pass(1, 1, 1, 1);
        deserialize_pass(1000, 1, 1, 1);
        deserialize_pass(1, 1, 1, 1000);
        deserialize_pass(650, 5, 60, 750);
    }

    #[test]
    fn serde() {
        serde_impl(10, 1, 1, 800);
        serde_impl(10, u32::MAX, 1, 800);
        serde_impl(1, 10, 10, 1);
        serde_impl(1, 10, 40, 123);
        serde_impl(1000, 2, 3, 1000);
        serde_impl(351, 13522, 13522, 750);
    }

    fn serde_impl(min_feeders: u32, sample_period: u32, samples_number: u16, discount_factor: u32) {
        let c = Config::new(
            Percent::from_permille(min_feeders),
            Duration::from_secs(sample_period),
            samples_number,
            Percent::from_permille(discount_factor),
        );
        assert_eq!(from_json(to_json_vec(&c).unwrap()), Ok(c));
    }

    fn deserialize_pass(
        min_feeders: u32,
        sample_period: u32,
        samples_number: u16,
        discount_factor: u32,
    ) {
        assert_eq!(
            Config::new(
                Percent::from_permille(min_feeders),
                Duration::from_secs(sample_period),
                samples_number,
                Percent::from_permille(discount_factor),
            ),
            deserialize(min_feeders, sample_period, samples_number, discount_factor).unwrap()
        );
    }

    fn deserialize_fail(
        min_feeders: u32,
        sample_period: u32,
        samples_number: u16,
        discount_factor: u32,
    ) {
        assert!(matches!(
            deserialize(min_feeders, sample_period, samples_number, discount_factor).unwrap_err(),
            StdError::ParseErr { .. }
        ));
    }

    fn deserialize(
        min_feeders: u32,
        sample_period: u32,
        samples_number: u16,
        discount_factor: u32,
    ) -> Result<Config, StdError> {
        from_json(
            format!("{{\"min_feeders\": {min_feeders}, \"sample_period_secs\": {sample_period},\"samples_number\": {samples_number}, \"discount_factor\": {discount_factor}}}")
                .as_bytes(),
        )
    }
}
