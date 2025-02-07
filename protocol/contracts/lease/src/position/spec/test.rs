use currencies::{testing::PaymentC3, Lpn};
use finance::{
    coin::Coin,
    duration::Duration,
    liability::Liability,
    percent::Percent,
    price::{self, Price},
};

use crate::{
    finance::LpnCoin,
    position::{close::Policy as ClosePolicy, DueTrait, OverdueCollection},
};

use super::Spec;

const MAX_DEBT: Percent = Percent::from_permille(800);

type TestCurrency = PaymentC3;
type TestLpn = Lpn;

struct TestDue {
    total_due: LpnCoin,
    overdue: LpnCoin,
}
impl DueTrait for TestDue {
    fn total_due(&self) -> LpnCoin {
        self.total_due
    }

    #[track_caller]
    fn overdue_collection(&self, min_amount: LpnCoin) -> OverdueCollection {
        if self.overdue.is_zero() || self.overdue < min_amount {
            OverdueCollection::StartIn(Duration::from_days(5))
        } else {
            OverdueCollection::Overdue(self.overdue)
        }
    }
}

fn due<StableAmount>(total_due: StableAmount, overdue_collectable: StableAmount) -> TestDue
where
    StableAmount: Into<Coin<TestLpn>>,
{
    TestDue {
        total_due: total_due.into(),
        overdue: overdue_collectable.into(),
    }
}

fn spec<Lpn>(min_asset: Lpn, min_transaction: Lpn) -> Spec
where
    Lpn: Into<LpnCoin>,
{
    let liability = Liability::new(
        Percent::from_percent(65),
        Percent::from_percent(70),
        Percent::from_percent(73),
        Percent::from_percent(75),
        Percent::from_percent(78),
        MAX_DEBT,
        Duration::from_hours(1),
    );
    Spec::new(
        liability,
        ClosePolicy::default(),
        min_asset.into(),
        min_transaction.into(),
    )
}

fn price<Asset, Lpn>(price_asset: Asset, price_lpn: Lpn) -> Price<TestCurrency, TestLpn>
where
    Asset: Into<Coin<TestCurrency>>,
    Lpn: Into<Coin<TestLpn>>,
{
    price::total_of(price_asset.into()).is(price_lpn.into())
}

mod test_calc_borrow {
    use finance::{
        coin::{Amount, Coin},
        percent::Percent,
    };

    use crate::position::PositionError;

    use super::TestLpn;

    #[test]
    fn downpayment_less_than_min() {
        let spec = super::spec(560, 300);

        let downpayment_less = spec.calc_borrow_amount(299.into(), None);
        assert!(matches!(
            downpayment_less,
            Err(PositionError::InsufficientTransactionAmount(_))
        ));

        let borrow = spec.calc_borrow_amount(300.into(), None);
        assert_eq!(coin_lpn(557), borrow.unwrap());
    }

    #[test]
    fn borrow_less_than_min() {
        let spec = super::spec(600, 300);

        let borrow_less = spec.calc_borrow_amount(300.into(), Some(Percent::from_percent(99)));
        assert!(matches!(
            borrow_less,
            Err(PositionError::InsufficientTransactionAmount(_))
        ));

        let borrow = spec.calc_borrow_amount(300.into(), Some(Percent::from_percent(100)));
        assert_eq!(coin_lpn(300), borrow.unwrap());
    }

    #[test]
    fn lease_less_than_min() {
        let spec = super::spec(1_000, 300);

        let borrow_1 = spec.calc_borrow_amount(349.into(), None);
        assert!(matches!(
            borrow_1,
            Err(PositionError::InsufficientAssetAmount(_))
        ));

        let borrow_2 = spec.calc_borrow_amount(350.into(), None);
        assert_eq!(coin_lpn(650), borrow_2.unwrap());

        let borrow_3 = spec.calc_borrow_amount(550.into(), Some(Percent::from_percent(81)));
        assert!(matches!(
            borrow_3,
            Err(PositionError::InsufficientAssetAmount(_))
        ));

        let borrow_3 = spec.calc_borrow_amount(550.into(), Some(Percent::from_percent(82)));
        assert_eq!(coin_lpn(451), borrow_3.unwrap());
    }

    #[test]
    fn valid_borrow_amount() {
        let spec = super::spec(1_000, 300);

        let borrow_1 = spec.calc_borrow_amount(540.into(), None);
        assert_eq!(coin_lpn(1002), borrow_1.unwrap());

        let borrow_2 = spec.calc_borrow_amount(870.into(), Some(Percent::from_percent(100)));
        assert_eq!(coin_lpn(870), borrow_2.unwrap());

        let borrow_3 = spec.calc_borrow_amount(650.into(), Some(Percent::from_percent(150)));
        assert_eq!(coin_lpn(975), borrow_3.unwrap());
    }

    fn coin_lpn(amount: Amount) -> Coin<TestLpn> {
        Coin::<TestLpn>::new(amount)
    }
}

mod test_debt {
    use currencies::Lpn;
    use finance::{
        coin::Coin,
        duration::Duration,
        fraction::Fraction,
        liability::{Liability, Zone},
        percent::Percent,
        price::Price,
        range::RightOpenRange,
    };

    use crate::position::{close::Policy as ClosePolicy, Cause, Debt, Spec, Steadiness};

    use super::TestCurrency;

    type TestLpn = Lpn;

    const RECALC_IN: Duration = Duration::from_hours(1);
    #[test]
    fn no_debt() {
        let warn_ltv = Percent::from_permille(11);
        let spec = spec_with_first(warn_ltv, 1, 1);
        let asset = 100.into();

        assert_eq!(
            spec.debt(asset, &super::due(0, 0), super::price(1, 1)),
            Debt::No,
        );
        assert_eq!(
            spec.debt(asset, &super::due(0, 0), super::price(3, 1)),
            Debt::No,
        );
    }

    #[test]
    fn warnings_none_zero_liq() {
        let warn_ltv = Percent::from_percent(51);
        let spec = spec_with_first(warn_ltv, 1, 1);
        let asset = 100.into();

        assert_eq!(
            spec.debt(asset, &super::due(1, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 1)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(1, 0), super::price(5, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 1)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(50, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 50)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(25, 0), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 25)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(51, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 51)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(17, 0), super::price(3, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 17)
            },
        );
    }

    #[test]
    fn warnings_none_min_transaction() {
        let warn_ltv = Percent::from_percent(51);
        let spec = spec_with_first(warn_ltv, 1, 15);
        let asset = 100.into();

        assert_eq!(
            spec.debt(asset, &super::due(50, 14), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 50)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(25, 4), super::price(2, 3)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 25)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(51, 14), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 51)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(17, 4), super::price(3, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 17)
            },
        );
    }

    #[test]
    fn warnings_first() {
        let warn_ltv = Percent::from_permille(712);
        let spec = spec_with_first(warn_ltv, 10, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(711, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 711)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(237, 0), super::price(3, 1)),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                steadiness: steady_to(warn_ltv, asset, 237)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(712, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 712)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(178, 0), super::price(4, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 178)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(712, 1), super::price(1, 1)),
            Debt::partial(1.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(89, 1), super::price(8, 1)),
            Debt::partial(8.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(712, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 712)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(103, 0), super::price(7, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 103)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(722, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv + STEP, warn_ltv + STEP + STEP),
                steadiness: steady_in(warn_ltv + STEP, warn_ltv + STEP + STEP, asset, 722)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(361, 0), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv + STEP, warn_ltv + STEP + STEP),
                steadiness: steady_in(warn_ltv + STEP, warn_ltv + STEP + STEP, asset, 361)
            },
        );
    }

    #[test]
    fn warnings_first_min_transaction() {
        let warn_ltv = Percent::from_permille(712);
        let spec = spec_with_first(warn_ltv, 10, 3);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(712, 2), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 712)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(356, 1), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 356)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(721, 2), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 721)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(721, 5), super::price(1, 1)),
            Debt::partial(5.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(240, 3), super::price(3, 1)),
            Debt::partial(9.into(), Cause::Overdue()),
        );
    }

    #[test]
    fn warnings_second() {
        let warn_ltv = Percent::from_permille(123);
        let spec = spec_with_second(warn_ltv, 10, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(122, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv - STEP, warn_ltv),
                steadiness: steady_in(warn_ltv - STEP, warn_ltv, asset, 122)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(15, 0), super::price(8, 1)),
            Debt::Ok {
                zone: Zone::first(warn_ltv - STEP, warn_ltv),
                steadiness: steady_in(warn_ltv - STEP, warn_ltv, asset, 15)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(123, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 123)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(82, 0), super::price(3, 2)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 82)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(123, 4), super::price(1, 1)),
            Debt::partial(4.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &super::due(132, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 132)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(66, 0), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 66)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(133, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_ltv + STEP, warn_ltv + STEP + STEP),
                steadiness: steady_in(warn_ltv + STEP, warn_ltv + STEP + STEP, asset, 133)
            },
        );
    }

    #[test]
    fn warnings_second_min_transaction() {
        let warn_ltv = Percent::from_permille(123);
        let spec = spec_with_second(warn_ltv, 10, 5);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(128, 4), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 128)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(32, 1), super::price(4, 1)),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                steadiness: steady_in(warn_ltv, warn_ltv + STEP, asset, 32)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(128, 5), super::price(1, 1)),
            Debt::partial(5.into(), Cause::Overdue())
        );
    }

    #[test]
    fn warnings_third() {
        let warn_third_ltv = Percent::from_permille(381);
        let max_ltv = warn_third_ltv + STEP;
        let spec = spec_with_third(warn_third_ltv, 100, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(380, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                steadiness: steady_in(warn_third_ltv - STEP, warn_third_ltv, asset, 380)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(190, 0), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                steadiness: steady_in(warn_third_ltv - STEP, warn_third_ltv, asset, 190)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(381, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 381)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(381, 375), super::price(1, 1)),
            Debt::partial(375.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &super::due(573, 562), super::price(2, 3)),
            Debt::partial(374.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &super::due(390, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 390)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(391, 0), super::price(1, 1)),
            Debt::partial(
                384.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
    }

    #[test]
    fn warnings_third_min_transaction() {
        let warn_third_ltv = Percent::from_permille(381);
        let max_ltv = warn_third_ltv + STEP;
        let spec = spec_with_third(warn_third_ltv, 100, 386);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(380, 1), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                steadiness: steady_in(warn_third_ltv - STEP, warn_third_ltv, asset, 380)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(126, 1), super::price(3, 1)),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                steadiness: steady_in(warn_third_ltv - STEP, warn_third_ltv, asset, 126)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(381, 375), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 381)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(391, 385), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 391)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(391, 386), super::price(1, 1)),
            Debt::partial(386.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(392, 0), super::price(1, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 392)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(364, 0), super::price(2, 1)),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                steadiness: steady_in(warn_third_ltv, max_ltv, asset, 364)
            },
        );
        assert_eq!(
            spec.debt(asset, &super::due(393, 0), super::price(1, 1)),
            Debt::partial(
                386.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(788, 0), super::price(1, 2)),
            Debt::partial(
                387.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
    }

    #[test]
    fn liquidate_partial() {
        let max_ltv = Percent::from_permille(881);
        let spec = spec_with_max(max_ltv, 100, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(880, 1), super::price(1, 1)),
            Debt::partial(1.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(139, 1), super::price(4, 1)),
            Debt::partial(4.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(881, 879), super::price(1, 1)),
            Debt::partial(
                879.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(881, 880), super::price(1, 1)),
            Debt::partial(880.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(294, 294), super::price(1, 3)),
            Debt::partial(98.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(294, 293), super::price(3, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1000, 1), super::price(1, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
    }

    #[test]
    fn liquidate_partial_min_asset() {
        let max_ltv = Percent::from_permille(881);
        let spec = spec_with_max(max_ltv, 100, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(900, 897), super::price(1, 1)),
            Debt::partial(
                898.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(900, 899), super::price(1, 1)),
            Debt::partial(899.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(233, 233), super::price(3, 1)),
            Debt::partial(699.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(901, 889), super::price(1, 1)),
            Debt::partial(
                900.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(902, 889), super::price(1, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
    }

    #[test]
    fn liquidate_full() {
        let max_ltv = Percent::from_permille(768);
        let spec = spec_with_max(max_ltv, 230, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(768, 765), super::price(1, 1)),
            Debt::partial(
                765.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1560, 1552), super::price(1, 2)),
            Debt::partial(
                777.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(768, 768), super::price(1, 1)),
            Debt::partial(768.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1560, 1556), super::price(1, 2)),
            Debt::partial(778.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(788, 768), super::price(1, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
    }

    #[test]
    fn liquidate_full_liability() {
        let max_ltv = Percent::from_permille(673);
        let spec = spec_with_max(max_ltv, 120, 15);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(882, 1), super::price(1, 1)),
            Debt::partial(
                880.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &super::due(883, 1), super::price(1, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &super::due(294, 1), super::price(3, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1000, 1), super::price(1, 1)),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
    }

    #[test]
    fn liquidate_full_overdue() {
        let max_ltv = Percent::from_permille(773);
        let spec = spec_with_max(max_ltv, 326, 15);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &super::due(772, 674), super::price(1, 1)),
            Debt::partial(674.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1674, 1674), super::price(1, 2)),
            Debt::partial(837.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(772, 675), super::price(1, 1)),
            Debt::full(Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &super::due(1676, 1676), super::price(1, 2)),
            Debt::full(Cause::Overdue()),
        );
    }

    const STEP: Percent = Percent::from_permille(10);

    fn spec_with_first<Lpn>(warn: Percent, min_asset: Lpn, min_transaction: Lpn) -> Spec
    where
        Lpn: Into<Coin<TestLpn>>,
    {
        spec_with_max(warn + STEP + STEP + STEP, min_asset, min_transaction)
    }

    fn spec_with_second<Lpn>(warn: Percent, min_asset: Lpn, min_transaction: Lpn) -> Spec
    where
        Lpn: Into<Coin<TestLpn>>,
    {
        spec_with_max(warn + STEP + STEP, min_asset, min_transaction)
    }

    fn spec_with_third<Lpn>(warn: Percent, min_asset: Lpn, min_transaction: Lpn) -> Spec
    where
        Lpn: Into<Coin<TestLpn>>,
    {
        spec_with_max(warn + STEP, min_asset, min_transaction)
    }

    // init = 1%, healthy = 1%, first = max - 3, second = max - 2, third = max - 1
    fn spec_with_max<Lpn>(max: Percent, min_asset: Lpn, min_transaction: Lpn) -> Spec
    where
        Lpn: Into<Coin<TestLpn>>,
    {
        let initial = STEP;
        assert!(initial < max - STEP - STEP - STEP);

        let healthy = initial + Percent::ZERO;
        let third_liquidity_warning = max - STEP;
        let second_liquidity_warning = third_liquidity_warning - STEP;
        let first_liquidity_warning = second_liquidity_warning - STEP;

        let liability = Liability::new(
            initial,
            healthy,
            first_liquidity_warning,
            second_liquidity_warning,
            third_liquidity_warning,
            max,
            RECALC_IN,
        );
        Spec::new(
            liability,
            ClosePolicy::default(),
            min_asset.into(),
            min_transaction.into(),
        )
    }

    fn steady_to<C, D>(ltv: Percent, asset: C, due: D) -> Steadiness<TestCurrency>
    where
        C: Into<Coin<TestCurrency>> + Copy,
        D: Into<Coin<TestLpn>> + Copy,
    {
        Steadiness::new(
            RECALC_IN,
            RightOpenRange::up_to(ltv).invert(ltv_to_price(asset, due)),
        )
    }

    fn steady_in<C, D>(
        ltv_from: Percent,
        ltv_to: Percent,
        asset: C,
        due: D,
    ) -> Steadiness<TestCurrency>
    where
        C: Into<Coin<TestCurrency>> + Copy,
        D: Into<Coin<TestLpn>> + Copy,
    {
        Steadiness::new(
            RECALC_IN,
            RightOpenRange::up_to(ltv_to)
                .cut_to(ltv_from)
                .invert(ltv_to_price(asset, due)),
        )
    }

    fn ltv_to_price<C, D>(asset: C, due: D) -> impl FnMut(Percent) -> Price<TestCurrency, TestLpn>
    where
        C: Into<Coin<TestCurrency>> + Copy,
        D: Into<Coin<TestLpn>> + Copy,
    {
        move |ltv| super::price(ltv.of(asset.into()), due.into())
    }
}

mod test_validate_payment {
    use crate::position::PositionError;

    #[test]
    fn insufficient_payment() {
        let spec = super::spec(65, 16);
        let result_1 = spec.validate_payment(15.into(), super::price(1, 1));
        assert!(matches!(
            result_1,
            Err(PositionError::InsufficientTransactionAmount(_))
        ));
        let result_2 = spec.validate_payment(16.into(), super::price(1, 1));
        assert!(result_2.is_ok());

        let result_3 = spec.validate_payment(45.into(), super::price(3, 1));
        assert!(matches!(
            result_3,
            Err(PositionError::InsufficientTransactionAmount(_))
        ));
        let result_4 = spec.validate_payment(8.into(), super::price(1, 2));
        assert!(result_4.is_ok());
    }
}

mod test_validate_close {
    use crate::position::PositionError;

    #[test]
    fn too_small_amount() {
        let spec = super::spec(75, 15);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 14.into(), super::price(1, 1));
        assert!(matches!(
            result_1,
            Err(PositionError::PositionCloseAmountTooSmall(_))
        ));

        let result_2 = spec.validate_close_amount(asset, 6.into(), super::price(1, 2));
        assert!(matches!(
            result_2,
            Err(PositionError::PositionCloseAmountTooSmall(_))
        ));
    }

    #[test]
    fn amount_as_min_transaction() {
        let spec = super::spec(85, 15);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 15.into(), super::price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 5.into(), super::price(1, 3));
        assert!(result_2.is_ok());
    }

    #[test]
    fn too_big_amount() {
        let spec = super::spec(25, 1);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 76.into(), super::price(1, 1));
        assert!(matches!(
            result_1,
            Err(PositionError::PositionCloseAmountTooBig(_))
        ));

        let result_2 = spec.validate_close_amount(asset, 64.into(), super::price(3, 2));
        assert!(matches!(
            result_2,
            Err(PositionError::PositionCloseAmountTooBig(_))
        ));
    }

    #[test]
    fn amount_as_min_asset() {
        let spec = super::spec(25, 1);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 75.into(), super::price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 62.into(), super::price(3, 2));
        assert!(result_2.is_ok());
    }

    #[test]
    fn valid_amount() {
        let spec = super::spec(40, 10);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 53.into(), super::price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 89.into(), super::price(1, 4));
        assert!(result_2.is_ok());
    }
}

mod test_check_close {
    use finance::percent::Percent;

    use crate::{
        api::position::{ChangeCmd, ClosePolicyChange},
        position::{CloseStrategy, PositionError},
    };

    #[test]
    fn set_higher_than_max() {
        let spec = super::spec(40, 10);

        let stop_loss_trigger = super::MAX_DEBT;
        assert_eq!(
            Err(PositionError::liquidation_conflict(
                super::MAX_DEBT,
                CloseStrategy::StopLoss(super::MAX_DEBT)
            )),
            spec.change_close_policy(
                ClosePolicyChange {
                    take_profit: None,
                    stop_loss: Some(ChangeCmd::Set(stop_loss_trigger)),
                },
                1000.into(),
                &super::due(550, 0),
                super::price(1, 2),
            )
        );
    }

    #[test]
    fn set_reset_stop_loss() {
        let spec = super::spec(40, 10);
        let asset = 100.into();

        assert_eq!(
            None,
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );

        let stop_loss_trigger = Percent::from_percent(46);
        assert_eq!(
            Err(PositionError::trigger_close(
                Percent::from_ratio(920, 1000 * 2),
                CloseStrategy::StopLoss(stop_loss_trigger)
            )),
            spec.change_close_policy(
                ClosePolicyChange {
                    take_profit: None,
                    stop_loss: Some(ChangeCmd::Set(stop_loss_trigger)),
                },
                1000.into(),
                &super::due(920, 0),
                super::price(1, 2),
            )
        );
        let spec = spec
            .change_close_policy(
                ClosePolicyChange {
                    take_profit: None,
                    stop_loss: Some(ChangeCmd::Set(stop_loss_trigger)),
                },
                1000.into(),
                &super::due(550, 0),
                super::price(1, 2),
            )
            .unwrap();

        assert_eq!(
            None,
            // 90 LPNs due = 45 Asset units due, 45/100 = 45% LPN
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );

        assert_eq!(
            Some(CloseStrategy::StopLoss(stop_loss_trigger)),
            // 92 LPNs due = 46 Asset units due, 46/100 = 46% LPN
            spec.check_close(asset, &super::due(92, 0), super::price(1, 2))
        );

        let spec = spec
            .change_close_policy(
                ClosePolicyChange {
                    take_profit: Some(ChangeCmd::Set(stop_loss_trigger)),
                    stop_loss: Some(ChangeCmd::Reset),
                },
                1000.into(),
                &super::due(920, 0),
                super::price(1, 2),
            )
            .unwrap();

        assert_eq!(
            None,
            // 92 LPNs due = 46 Asset units due, 46/100 = 46% LPN
            spec.check_close(asset, &super::due(92, 0), super::price(1, 2))
        );
        assert_eq!(
            Some(CloseStrategy::TakeProfit(stop_loss_trigger)),
            // 90 LPNs due = 45 Asset units due, 45/100 = 45% LPN
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );
    }

    #[test]
    fn set_reset_take_profit() {
        let spec = super::spec(40, 10);
        let asset = 100.into();

        assert_eq!(
            None,
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );

        let take_profit_trigger = Percent::from_percent(46);
        assert_eq!(
            Err(PositionError::trigger_close(
                Percent::from_ratio(919, 1000 * 2),
                CloseStrategy::TakeProfit(take_profit_trigger)
            )),
            spec.change_close_policy(
                ClosePolicyChange {
                    take_profit: Some(ChangeCmd::Set(take_profit_trigger)),
                    stop_loss: None,
                },
                1000.into(),
                &super::due(919, 0),
                super::price(1, 2),
            )
        );
        let spec = spec
            .change_close_policy(
                ClosePolicyChange {
                    take_profit: Some(ChangeCmd::Set(take_profit_trigger)),
                    stop_loss: None,
                },
                1000.into(),
                &super::due(920, 0),
                super::price(1, 2),
            )
            .unwrap();

        assert_eq!(
            None,
            // 92 LPNs due = 46 Asset units due, 46/100 = 46% LPN
            spec.check_close(asset, &super::due(92, 0), super::price(1, 2))
        );

        assert_eq!(
            Some(CloseStrategy::TakeProfit(take_profit_trigger)),
            // 90 LPNs due = 45 Asset units due, 45/100 = 45% LPN
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );

        let spec = spec
            .change_close_policy(
                ClosePolicyChange {
                    take_profit: Some(ChangeCmd::Reset),
                    stop_loss: Some(ChangeCmd::Set(take_profit_trigger)),
                },
                1000.into(),
                &super::due(550, 0),
                super::price(1, 2),
            )
            .unwrap();

        assert_eq!(
            None,
            // 90 LPNs due = 45 Asset units due, 45/100 = 45% LPN
            spec.check_close(asset, &super::due(90, 0), super::price(1, 2))
        );
        assert_eq!(
            Some(CloseStrategy::StopLoss(take_profit_trigger)),
            // 92 LPNs due = 46 Asset units due, 46/100 = 46% LPN
            spec.check_close(asset, &super::due(92, 0), super::price(1, 2))
        );
    }
}
