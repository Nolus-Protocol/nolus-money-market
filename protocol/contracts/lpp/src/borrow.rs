use serde::{Deserialize, Serialize};

use finance::{
    coin::Coin,
    fraction::Fraction,
    percent::{Percent, Units},
    ratio::Rational,
};
use sdk::schemars::{self, JsonSchema};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "UncheckedInterestRate")]
pub struct InterestRate {
    base_interest_rate: Percent,
    utilization_optimal: Percent,
    addon_optimal_interest_rate: Percent,
}

impl InterestRate {
    #[cfg(any(test, feature = "testing"))]
    pub fn new(
        base_interest_rate: Percent,
        utilization_optimal: Percent,
        addon_optimal_interest_rate: Percent,
    ) -> Option<Self> {
        Self::private_new(
            base_interest_rate,
            utilization_optimal,
            addon_optimal_interest_rate,
        )
    }

    fn private_new(
        base_interest_rate: Percent,
        utilization_optimal: Percent,
        addon_optimal_interest_rate: Percent,
    ) -> Option<Self> {
        let value = Self {
            base_interest_rate,
            utilization_optimal,
            addon_optimal_interest_rate,
        };

        value.validate().then_some(value)
    }

    pub fn base_interest_rate(&self) -> Percent {
        self.base_interest_rate
    }

    pub fn utilization_optimal(&self) -> Percent {
        self.utilization_optimal
    }

    pub fn addon_optimal_interest_rate(&self) -> Percent {
        self.addon_optimal_interest_rate
    }

    pub fn calculate<Lpn>(&self, total_liability: Coin<Lpn>, balance: Coin<Lpn>) -> Option<Percent>
    where
        Lpn: PartialEq,
    {
        Percent::from_ratio(
            self.utilization_optimal.units(),
            (Percent::HUNDRED - self.utilization_optimal).units(),
        )
        .and_then(|utilization_max| {
            let config = Rational::new(
                self.addon_optimal_interest_rate.units(),
                self.utilization_optimal.units(),
            );

            if balance.is_zero() {
                Some(utilization_max)
            } else {
                Percent::from_ratio(total_liability, balance)
                    .map(|utilization| utilization.min(utilization_max))
            }
            .and_then(|utilization| {
                Fraction::<Units>::of(&config, utilization).map(|res| self.base_interest_rate + res)
            })
        })
    }

    fn validate(&self) -> bool {
        self.base_interest_rate <= Percent::HUNDRED
            && self.utilization_optimal > Percent::ZERO
            && self.utilization_optimal < Percent::HUNDRED
            && self.addon_optimal_interest_rate <= Percent::HUNDRED
    }
}

impl TryFrom<UncheckedInterestRate> for InterestRate {
    type Error = &'static str;

    fn try_from(value: UncheckedInterestRate) -> Result<Self, Self::Error> {
        Self::private_new(
            value.base_interest_rate,
            value.utilization_optimal,
            value.addon_optimal_interest_rate,
        )
        .ok_or("Rates should not be greater than a hundred percent!")
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct UncheckedInterestRate {
    base_interest_rate: Percent,
    utilization_optimal: Percent,
    addon_optimal_interest_rate: Percent,
}

#[cfg(test)]
mod tests {
    use finance::percent::Percent;

    use crate::borrow::InterestRate;

    #[test]
    fn test_constructor() {
        assert!(
            InterestRate::new(Percent::ZERO, Percent::from_percent(1), Percent::ZERO).is_some(),
            ""
        );
        assert!(InterestRate::new(Percent::ZERO, Percent::HUNDRED, Percent::ZERO).is_none());
        assert!(InterestRate::new(
            Percent::from_percent(25),
            Percent::from_percent(50),
            Percent::from_percent(75)
        )
        .is_some());
        assert!(InterestRate::new(Percent::HUNDRED, Percent::HUNDRED, Percent::HUNDRED).is_none());

        assert!(InterestRate::new(Percent::ZERO, Percent::ZERO, Percent::ZERO).is_none());
        assert!(InterestRate::new(
            Percent::from_percent(25),
            Percent::ZERO,
            Percent::from_percent(75)
        )
        .is_none());
        assert!(InterestRate::new(Percent::HUNDRED, Percent::ZERO, Percent::HUNDRED).is_none());
        assert!(InterestRate::new(
            Percent::from_percent(101),
            Percent::HUNDRED,
            Percent::HUNDRED
        )
        .is_none());
        assert!(InterestRate::new(
            Percent::HUNDRED,
            Percent::from_percent(101),
            Percent::HUNDRED
        )
        .is_none());
        assert!(InterestRate::new(
            Percent::HUNDRED,
            Percent::HUNDRED,
            Percent::from_percent(101)
        )
        .is_none());
        assert!(InterestRate::new(
            Percent::from_percent(101),
            Percent::ZERO,
            Percent::from_percent(101)
        )
        .is_none());
        assert!(InterestRate::new(
            Percent::from_percent(101),
            Percent::from_percent(101),
            Percent::from_percent(101)
        )
        .is_none());
    }

    /// Test suit specifically for verifying correctness of [`InterestRate::calculate`](InterestRate::calculate).cargo fmt
    mod calculate {
        use crate::borrow::InterestRate;
        use finance::{
            coin::{Amount, Coin},
            percent::{Percent, Units},
        };
        use lpp_platform::NLpn;

        /// Constructs an instance of [`InterestRate`].
        ///
        /// # Arguments
        ///
        /// Arguments represent rates in per milles.
        ///
        /// returns: [`InterestRate`]
        fn rate(
            base_interest_rate: u32,
            utilization_optimal: u32,
            addon_optimal_interest_rate: u32,
        ) -> InterestRate {
            InterestRate::new(
                Percent::from_permille(base_interest_rate),
                Percent::from_permille(utilization_optimal),
                Percent::from_permille(addon_optimal_interest_rate),
            )
            .expect("Rates should be less or equal to a thousand!")
        }

        fn ratio(n: Units, d: Units) -> Percent {
            Percent::from_ratio(n, d).unwrap()
        }

        #[derive(Copy, Clone)]
        struct InOut((Amount, Amount), (Units, Units));

        fn in_out(InOut((l, b), (n, d)): InOut) -> ((Coin<NLpn>, Coin<NLpn>), Percent) {
            ((Coin::new(l), Coin::new(b)), ratio(n, d))
        }

        fn do_test_calculate(rate: InterestRate, in_out_set: &[InOut]) {
            for ((liability, balance), output) in in_out_set.iter().copied().map(in_out) {
                assert_eq!(
                    rate.calculate(liability, balance).unwrap(),
                    output,
                    "Interest rate: {rate:?}\nLiability: {liability}\nBalance: {balance}",
                );
            }
        }

        #[test]
        /// Verifies that when there is no addon optimal interest rate, result is equal to the base interest rate.
        fn test_set_1() {
            for base_rate in 0..=200 {
                let rate = rate(base_rate, 500, 0);

                do_test_calculate(
                    rate,
                    &(0..=25)
                        .flat_map(|liability| {
                            (0..=25).filter_map(move |balance| {
                                (liability != 0 || balance != 0)
                                    .then_some(InOut((liability, balance), (base_rate, 1000)))
                            })
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }

        #[test]
        /// Verifies that when liability is equal to zero, result is equal to the base interest rate.
        fn test_set_2() {
            for base_rate in 0..=100 {
                let rate = rate(base_rate, 500, 1000);

                do_test_calculate(
                    rate,
                    &(1..=100)
                        .map(move |balance| InOut((0, balance), (base_rate, 1000)))
                        .collect::<Vec<_>>(),
                );
            }
        }

        #[test]
        fn test_corner_set() {
            let rate = rate(1000, 900, 1000);

            let set = [InOut((0, 0), (11, 1)), InOut((10, 0), (11, 1))];
            do_test_calculate(rate, &set);
        }

        #[test]
        /// Verifies correctness of results against manually calculated, thus verified, set.
        fn test_set_4() {
            let rate = rate(100, 655, 250);

            let set = [
                InOut((10, 1), (824, 1000)),
                InOut((10, 2), (824, 1000)),
                InOut((10, 3), (824, 1000)),
                InOut((10, 4), (824, 1000)),
                InOut((10, 5), (824, 1000)),
                InOut((10, 6), (735, 1000)),
                InOut((10, 7), (645, 1000)),
                InOut((10, 8), (577, 1000)),
                InOut((10, 9), (524, 1000)),
                InOut((10, 10), (481, 1000)),
                InOut((10, 11), (446, 1000)),
                InOut((10, 12), (417, 1000)),
                InOut((10, 13), (393, 1000)),
                InOut((10, 14), (372, 1000)),
                InOut((10, 15), (354, 1000)),
            ];

            do_test_calculate(rate, &set);
        }

        #[test]
        /// Verifies correctness of results against manually calculated, thus verified, set.
        fn test_set_5() {
            let rate = rate(120, 700, 20);

            let set = [
                InOut((0, 1), (3, 25)),
                InOut((1, 9), (123, 1000)),
                InOut((3, 7), (132, 1000)),
                InOut((5, 5), (148, 1000)),
                InOut((7, 3), (186, 1000)),
                InOut((8, 2), (186, 1000)),
                InOut((9, 1), (186, 1000)),
                InOut((0, 0), (186, 1000)),
                InOut((1, 0), (186, 1000)),
            ];

            do_test_calculate(rate, &set);
        }

        #[test]
        /// Verifies correctness of results against manually calculated, thus verified, set.
        fn test_set_6() {
            let rate = rate(100, 750, 20);

            let set = [InOut((2584283, 40054 - 18571), (18, 100))];

            do_test_calculate(rate, &set);
        }
    }
}
