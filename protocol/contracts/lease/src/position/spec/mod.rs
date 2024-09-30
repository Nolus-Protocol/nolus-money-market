use std::ops::Add;

use currency::{Currency, CurrencyDef, MemberOf};
use finance::{
    coin::Coin,
    duration::Duration,
    error::Error as FinanceError,
    liability::Liability,
    percent::Percent,
    price::{self},
};

use crate::{
    api::LeasePaymentCurrencies,
    error::{ContractError, ContractResult},
    finance::{LpnCoin, Price},
    position::{Cause, Debt, Liquidation},
};

use super::{interest::OverdueCollection, DueTrait};

mod dto;

#[cfg_attr(test, derive(Debug))]
pub struct Spec {
    liability: Liability,
    min_asset: LpnCoin,
    min_transaction: LpnCoin,
}

impl Spec {
    pub fn new(liability: Liability, min_asset: LpnCoin, min_transaction: LpnCoin) -> Self {
        let obj = Self {
            liability,
            min_asset,
            min_transaction,
        };
        debug_assert_eq!(Ok(()), obj.invariant_held());
        obj
    }

    /// Calculate the borrow amount.
    /// Return 'error::ContractError::InsufficientTransactionAmount' when either the downpayment
    /// or the borrow amount is less than the minimum transaction amount.
    /// Return 'error::ContractError::InsufficientAssetAmount' when the lease (downpayment + borrow)
    /// is less than the minimum asset amount.
    pub fn calc_borrow_amount(
        &self,
        downpayment: LpnCoin,
        may_max_ltd: Option<Percent>,
    ) -> ContractResult<LpnCoin> {
        let one = Price::identity();

        self.valid_transaction(downpayment, one)
            .and_then(|is_valid| {
                if !is_valid {
                    Err(ContractError::InsufficientTransactionAmount(
                        self.min_transaction.into(),
                    ))
                } else {
                    Ok(())
                }
            })
            .and_then(|()| {
                self.liability
                    .init_borrow_amount(downpayment, may_max_ltd)
                    .ok_or(ContractError::FinanceError(FinanceError::Overflow(
                        format!(
                            "Overflow while calculating the borrow amount with downpayment: {:?}",
                            downpayment
                        ),
                    )))
            })
            .and_then(|borrow| {
                self.valid_transaction(borrow, one)
                    .and_then(|is_valid_borrow| {
                        if !is_valid_borrow {
                            Err(ContractError::InsufficientTransactionAmount(
                                self.min_transaction.into(),
                            ))
                        } else {
                            Ok(borrow)
                        }
                    })
            })
            .and_then(|borrow| {
                self.valid_asset(downpayment.add(borrow), one)
                    .and_then(|is_valid_asset| {
                        if !is_valid_asset {
                            Err(ContractError::InsufficientAssetAmount(
                                self.min_asset.into(),
                            ))
                        } else {
                            Ok(borrow)
                        }
                    })
            })
    }

    pub fn overdue_collection_in<Due>(&self, due: &Due) -> Option<Duration>
    where
        Due: DueTrait,
    {
        self.overdue_collection(due)
            .map(|overdue_collection| overdue_collection.start_in())
    }

    pub fn debt<Asset, Due>(
        &self,
        asset: Coin<Asset>,
        due: &Due,
        asset_in_lpns: Price<Asset>,
    ) -> Option<Debt<Asset>>
    where
        Asset: Currency,
        Due: DueTrait,
    {
        let total_due = price::total(due.total_due(), asset_in_lpns.inv())?;

        self.may_ask_liquidation_liability(asset, total_due, asset_in_lpns)
            .max(self.may_ask_liquidation_overdue(asset, due, asset_in_lpns))
            .map(Debt::Bad)
            .or_else(|| {
                let ltv = Percent::from_ratio(total_due, asset)?;
                // The ltv can be above the max percent and due to other circumstances the liquidation may not happen
                self.no_liquidation(due, ltv.min(self.liability.third_liq_warn()))
            })
    }

    /// Check if the amount can be used for repayment.
    /// Return `error::ContractError::InsufficientPayment` when the payment amount
    /// is less than the minimum transaction amount.
    pub fn validate_payment<PaymentC>(
        &self,
        payment: Coin<PaymentC>,
        payment_currency_in_lpns: Price<PaymentC>,
    ) -> ContractResult<()>
    where
        PaymentC: CurrencyDef,
        PaymentC::Group: MemberOf<LeasePaymentCurrencies>,
    {
        self.valid_transaction(payment, payment_currency_in_lpns)
            .and_then(|is_valid_transaction| {
                if is_valid_transaction {
                    Ok(())
                } else {
                    price::total(self.min_transaction, payment_currency_in_lpns.inv())
                        .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                            "while calculating the total",
                            self.min_transaction,
                            payment_currency_in_lpns.inv(),
                        )))
                        .and_then(|amount| Err(ContractError::InsufficientPayment(amount.into())))
                }
            })
    }

    /// Check if the amount can be used to close the position.
    /// Return `error::ContractError::PositionCloseAmountTooSmall` when a partial close is requested
    /// with amount less than the minimum transaction position parameter sent on lease open. Refer to
    /// `NewLeaseForm::position_spec`.
    ///
    /// Return `error::ContractError::PositionCloseAmountTooBig` when a partial close is requested
    /// with amount that would decrease a position less than the minimum asset parameter sent on
    /// lease open. Refer to `NewLeaseForm::position_spec`.
    pub fn validate_close_amount<Asset>(
        &self,
        asset: Coin<Asset>,
        close_amount: Coin<Asset>,
        asset_in_lpns: Price<Asset>,
    ) -> ContractResult<()>
    where
        Asset: Currency,
    {
        self.valid_transaction(close_amount, asset_in_lpns)
            .and_then(|is_valid_transaction| {
                if is_valid_transaction {
                    self.valid_asset(asset.saturating_sub(close_amount), asset_in_lpns)
                        .and_then(|is_valid_asset| {
                            if is_valid_asset {
                                Ok(())
                            } else {
                                Err(ContractError::PositionCloseAmountTooBig(
                                    self.min_asset.into(),
                                ))
                            }
                        })
                } else {
                    Err(ContractError::PositionCloseAmountTooSmall(
                        self.min_transaction.into(),
                    ))
                }
            })
    }

    fn invariant_held(&self) -> ContractResult<()> {
        Self::check(
            !self.min_asset.is_zero(),
            "Min asset amount should be positive",
        )
        .and(Self::check(
            !self.min_transaction.is_zero(),
            "Min transaction amount should be positive",
        ))
    }

    fn check(invariant: bool, msg: &str) -> ContractResult<()> {
        ContractError::broken_invariant_if::<Self>(!invariant, msg)
    }

    fn valid_transaction<TransactionC>(
        &self,
        amount: Coin<TransactionC>,
        transaction_currency_in_lpn: Price<TransactionC>,
    ) -> ContractResult<bool>
    where
        TransactionC: Currency,
    {
        price::total(amount, transaction_currency_in_lpn)
            .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                "while calculating the total",
                amount,
                transaction_currency_in_lpn,
            )))
            .map(|amount| amount >= self.min_transaction)
    }

    fn valid_asset<TransactionC>(
        &self,
        asset_amount: Coin<TransactionC>,
        transaction_currency_in_lpn: Price<TransactionC>,
    ) -> ContractResult<bool>
    where
        TransactionC: Currency,
    {
        price::total(asset_amount, transaction_currency_in_lpn)
            .ok_or(ContractError::FinanceError(FinanceError::overflow_err(
                "while calculating the total",
                asset_amount,
                transaction_currency_in_lpn,
            )))
            .map(|asset_amount| asset_amount >= self.min_asset)
    }

    fn may_ask_liquidation_liability<Asset>(
        &self,
        asset: Coin<Asset>,
        total_due: Coin<Asset>,
        asset_in_lpns: Price<Asset>,
    ) -> Option<Liquidation<Asset>>
    where
        Asset: Currency,
    {
        self.liability
            .amount_to_liquidate(asset, total_due)
            .and_then(|liquidation_amount| {
                self.may_ask_liquidation(
                    asset,
                    Cause::Liability {
                        ltv: self.liability.max(),
                        healthy_ltv: self.liability.healthy_percent(),
                    },
                    liquidation_amount,
                    asset_in_lpns,
                )
            })
    }

    fn may_ask_liquidation_overdue<Asset, Due>(
        &self,
        asset: Coin<Asset>,
        due: &Due,
        asset_in_lpns: Price<Asset>,
    ) -> Option<Liquidation<Asset>>
    where
        Asset: Currency,
        Due: DueTrait,
    {
        self.overdue_collection(due).and_then(|collection| {
            let collectable = collection.amount();
            debug_assert!(collectable <= due.total_due());
            price::total(collectable, asset_in_lpns.inv()).and_then(|to_liquidate| {
                self.may_ask_liquidation(asset, Cause::Overdue(), to_liquidate, asset_in_lpns)
            })
        })
    }

    fn may_ask_liquidation<Asset>(
        &self,
        asset: Coin<Asset>,
        cause: Cause,
        liquidation: Coin<Asset>,
        asset_in_lpns: Price<Asset>,
    ) -> Option<Liquidation<Asset>>
    where
        Asset: Currency,
    {
        match self.validate_close_amount(asset, liquidation, asset_in_lpns) {
            Err(ContractError::PositionCloseAmountTooSmall(_)) => None,
            Err(ContractError::PositionCloseAmountTooBig(_)) => Some(Liquidation::Full(cause)),
            Err(_) => unreachable!(),
            Ok(()) => {
                debug_assert!(liquidation < asset);
                Some(Liquidation::Partial {
                    amount: liquidation,
                    cause,
                })
            }
        }
    }

    fn no_liquidation<Asset, Due>(&self, due: &Due, ltv: Percent) -> Option<Debt<Asset>>
    where
        Asset: Currency,
        Due: DueTrait,
    {
        debug_assert!(ltv < self.liability.max());
        if due.total_due().is_zero() {
            Some(Debt::No)
        } else {
            self.overdue_collection_in(due)
                .map(|overdue_collection_in| Debt::Ok {
                    zone: self.liability.zone_of(ltv),
                    recheck_in: overdue_collection_in.min(self.liability.recalculation_time()),
                })
        }
    }

    fn overdue_collection<Due>(&self, due: &Due) -> Option<OverdueCollection>
    where
        Due: DueTrait,
    {
        due.overdue_collection(self.min_transaction)
    }
}

#[cfg(test)]
mod test_calc_borrow {
    use currencies::Lpn;
    use finance::{
        coin::{Amount, Coin},
        duration::Duration,
        liability::Liability,
        percent::Percent,
    };

    use crate::error::ContractError;

    use super::Spec;

    type TestLpn = Lpn;

    #[test]
    fn downpayment_less_than_min() {
        let spec = spec(560, 300);

        let downpayment_less = spec.calc_borrow_amount(299.into(), None);
        assert!(matches!(
            downpayment_less,
            Err(ContractError::InsufficientTransactionAmount(_))
        ));

        let borrow = spec.calc_borrow_amount(300.into(), None);
        assert_eq!(coin_lpn(557), borrow.unwrap());
    }

    #[test]
    fn borrow_less_than_min() {
        let spec = spec(600, 300);

        let borrow_less = spec.calc_borrow_amount(300.into(), Some(Percent::from_percent(99)));
        assert!(matches!(
            borrow_less,
            Err(ContractError::InsufficientTransactionAmount(_))
        ));

        let borrow = spec.calc_borrow_amount(300.into(), Some(Percent::from_percent(100)));
        assert_eq!(coin_lpn(300), borrow.unwrap());
    }

    #[test]
    fn lease_less_than_min() {
        let spec = spec(1_000, 300);

        let borrow_1 = spec.calc_borrow_amount(349.into(), None);
        assert!(matches!(
            borrow_1,
            Err(ContractError::InsufficientAssetAmount(_))
        ));

        let borrow_2 = spec.calc_borrow_amount(350.into(), None);
        assert_eq!(coin_lpn(650), borrow_2.unwrap());

        let borrow_3 = spec.calc_borrow_amount(550.into(), Some(Percent::from_percent(81)));
        assert!(matches!(
            borrow_3,
            Err(ContractError::InsufficientAssetAmount(_))
        ));

        let borrow_3 = spec.calc_borrow_amount(550.into(), Some(Percent::from_percent(82)));
        assert_eq!(coin_lpn(451), borrow_3.unwrap());
    }

    #[test]
    fn valid_borrow_amount() {
        let spec = spec(1_000, 300);

        let borrow_1 = spec.calc_borrow_amount(540.into(), None);
        assert_eq!(coin_lpn(1002), borrow_1.unwrap());

        let borrow_2 = spec.calc_borrow_amount(870.into(), Some(Percent::from_percent(100)));
        assert_eq!(coin_lpn(870), borrow_2.unwrap());

        let borrow_3 = spec.calc_borrow_amount(650.into(), Some(Percent::from_percent(150)));
        assert_eq!(coin_lpn(975), borrow_3.unwrap());
    }

    fn spec<LpnAmount>(min_asset: LpnAmount, min_transaction: LpnAmount) -> Spec
    where
        LpnAmount: Into<Coin<TestLpn>>,
    {
        let liability = Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(70),
            Percent::from_percent(73),
            Percent::from_percent(75),
            Percent::from_percent(78),
            Percent::from_percent(80),
            Duration::from_hours(1),
        );
        Spec::new(liability, min_asset.into(), min_transaction.into())
    }

    fn coin_lpn(amount: Amount) -> Coin<TestLpn> {
        Coin::<TestLpn>::new(amount)
    }
}

#[cfg(test)]
mod test_debt {

    use currencies::{Lpn, PaymentC3};
    use finance::{
        coin::Coin,
        duration::Duration,
        liability::{Liability, Zone},
        percent::Percent,
        price::{self, Price},
    };

    use crate::{
        finance::LpnCoin,
        position::{Cause, Debt, DueTrait, OverdueCollection},
    };

    use super::Spec;

    type TestCurrency = PaymentC3;
    type TestLpn = Lpn;

    const RECALC_IN: Duration = Duration::from_hours(1);
    struct TestDue {
        total_due: LpnCoin,
        overdue: LpnCoin,
    }
    impl DueTrait for TestDue {
        fn total_due(&self) -> LpnCoin {
            self.total_due
        }

        #[track_caller]
        fn overdue_collection(&self, min_amount: LpnCoin) -> Option<OverdueCollection> {
            if self.overdue.is_zero() || self.overdue < min_amount {
                Some(OverdueCollection::StartIn(Duration::from_days(5)))
            } else {
                Some(OverdueCollection::Overdue(self.overdue))
            }
        }
    }

    #[test]
    fn no_debt() {
        let warn_ltv = Percent::from_permille(11);
        let spec = spec_with_first(warn_ltv, 1, 1);
        let asset = 100.into();

        assert_eq!(spec.debt(asset, &due(0, 0), price(1, 1)).unwrap(), Debt::No,);
        assert_eq!(spec.debt(asset, &due(0, 0), price(3, 1)).unwrap(), Debt::No,);
    }

    #[test]
    fn warnings_none_zero_liq() {
        let warn_ltv = Percent::from_percent(51);
        let spec = spec_with_first(warn_ltv, 1, 1);
        let asset = 100.into();

        assert_eq!(
            spec.debt(asset, &due(1, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(1, 0), price(5, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(50, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(25, 0), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(51, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(17, 0), price(3, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
    }

    #[test]
    fn warnings_none_min_transaction() {
        let warn_ltv = Percent::from_percent(51);
        let spec = spec_with_first(warn_ltv, 1, 15);
        let asset = 100.into();

        assert_eq!(
            spec.debt(asset, &due(50, 14), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(25, 4), price(2, 3)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(51, 14), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(17, 4), price(3, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
    }

    #[test]
    fn warnings_first() {
        let warn_ltv = Percent::from_permille(712);
        let spec = spec_with_first(warn_ltv, 10, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &due(711, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(237, 0), price(3, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::no_warnings(warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(712, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(178, 0), price(4, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(712, 1), price(1, 1)).unwrap(),
            Debt::partial(1.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(89, 1), price(8, 1)).unwrap(),
            Debt::partial(8.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(712, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(103, 0), price(7, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(722, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv + STEP, warn_ltv + STEP + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(361, 0), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv + STEP, warn_ltv + STEP + STEP),
                recheck_in: RECALC_IN
            },
        );
    }

    #[test]
    fn warnings_first_min_transaction() {
        let warn_ltv = Percent::from_permille(712);
        let spec = spec_with_first(warn_ltv, 10, 3);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &due(712, 2), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(356, 1), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(721, 2), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(721, 5), price(1, 1)).unwrap(),
            Debt::partial(5.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(240, 3), price(3, 1)).unwrap(),
            Debt::partial(9.into(), Cause::Overdue()),
        );
    }

    #[test]
    fn warnings_second() {
        let warn_ltv = Percent::from_permille(123);
        let spec = spec_with_second(warn_ltv, 10, 1);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &due(122, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv - STEP, warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(15, 0), price(8, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::first(warn_ltv - STEP, warn_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(123, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(82, 0), price(3, 2)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(123, 4), price(1, 1)).unwrap(),
            Debt::partial(4.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &due(132, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(66, 0), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(133, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_ltv + STEP, warn_ltv + STEP + STEP),
                recheck_in: RECALC_IN
            },
        );
    }

    #[test]
    fn warnings_second_min_transaction() {
        let warn_ltv = Percent::from_permille(123);
        let spec = spec_with_second(warn_ltv, 10, 5);
        let asset = 1000.into();

        assert_eq!(
            spec.debt(asset, &due(128, 4), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(32, 1), price(4, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_ltv, warn_ltv + STEP),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(128, 5), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(380, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(190, 0), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(381, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(381, 375), price(1, 1)).unwrap(),
            Debt::partial(375.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &due(573, 562), price(2, 3)).unwrap(),
            Debt::partial(374.into(), Cause::Overdue())
        );
        assert_eq!(
            spec.debt(asset, &due(390, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(391, 0), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(380, 1), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(126, 1), price(3, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::second(warn_third_ltv - STEP, warn_third_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(381, 375), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(391, 385), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(391, 386), price(1, 1)).unwrap(),
            Debt::partial(386.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(392, 0), price(1, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(364, 0), price(2, 1)).unwrap(),
            Debt::Ok {
                zone: Zone::third(warn_third_ltv, max_ltv),
                recheck_in: RECALC_IN
            },
        );
        assert_eq!(
            spec.debt(asset, &due(393, 0), price(1, 1)).unwrap(),
            Debt::partial(
                386.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(788, 0), price(1, 2)).unwrap(),
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
            spec.debt(asset, &due(880, 1), price(1, 1)).unwrap(),
            Debt::partial(1.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(139, 1), price(4, 1)).unwrap(),
            Debt::partial(4.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(881, 879), price(1, 1)).unwrap(),
            Debt::partial(
                879.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(881, 880), price(1, 1)).unwrap(),
            Debt::partial(880.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(294, 294), price(1, 3)).unwrap(),
            Debt::partial(98.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(294, 293), price(3, 1)).unwrap(),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &due(1000, 1), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(900, 897), price(1, 1)).unwrap(),
            Debt::partial(
                898.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(900, 899), price(1, 1)).unwrap(),
            Debt::partial(899.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(233, 233), price(3, 1)).unwrap(),
            Debt::partial(699.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(901, 889), price(1, 1)).unwrap(),
            Debt::partial(
                900.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(902, 889), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(768, 765), price(1, 1)).unwrap(),
            Debt::partial(
                765.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(1560, 1552), price(1, 2)).unwrap(),
            Debt::partial(
                777.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(768, 768), price(1, 1)).unwrap(),
            Debt::partial(768.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(1560, 1556), price(1, 2)).unwrap(),
            Debt::partial(778.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(788, 768), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(882, 1), price(1, 1)).unwrap(),
            Debt::partial(
                880.into(),
                Cause::Liability {
                    ltv: max_ltv,
                    healthy_ltv: STEP
                }
            ),
        );
        assert_eq!(
            spec.debt(asset, &due(883, 1), price(1, 1)).unwrap(),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &due(294, 1), price(3, 1)).unwrap(),
            Debt::full(Cause::Liability {
                ltv: max_ltv,
                healthy_ltv: STEP
            }),
        );
        assert_eq!(
            spec.debt(asset, &due(1000, 1), price(1, 1)).unwrap(),
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
            spec.debt(asset, &due(772, 674), price(1, 1)).unwrap(),
            Debt::partial(674.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(1674, 1674), price(1, 2)).unwrap(),
            Debt::partial(837.into(), Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(772, 675), price(1, 1)).unwrap(),
            Debt::full(Cause::Overdue()),
        );
        assert_eq!(
            spec.debt(asset, &due(1676, 1676), price(1, 2)).unwrap(),
            Debt::full(Cause::Overdue()),
        );
    }

    const STEP: Percent = Percent::from_permille(10);

    fn price<Asset, Lpn>(price_asset: Asset, price_lpn: Lpn) -> Price<TestCurrency, TestLpn>
    where
        Asset: Into<Coin<TestCurrency>>,
        Lpn: Into<Coin<TestLpn>>,
    {
        price::total_of(price_asset.into()).is(price_lpn.into())
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
        let max = healthy + max - initial;
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
        Spec::new(liability, min_asset.into(), min_transaction.into())
    }
}

#[cfg(test)]
mod test_validate_payment {
    use currencies::{LeaseC1, Lpn};
    use finance::{
        coin::Coin,
        duration::Duration,
        liability::Liability,
        percent::Percent,
        price::{self, Price},
    };

    use crate::error::ContractError;

    use super::Spec;

    type TestLpn = Lpn;
    type TestPaymentC = LeaseC1;

    #[test]
    fn insufficient_payment() {
        let spec = spec(65, 16);
        let result_1 = spec.validate_payment(15.into(), price(1, 1));
        assert!(matches!(
            result_1,
            Err(ContractError::InsufficientPayment(_))
        ));
        let result_2 = spec.validate_payment(16.into(), price(1, 1));
        assert!(result_2.is_ok());

        let result_3 = spec.validate_payment(45.into(), price(3, 1));
        assert!(matches!(
            result_3,
            Err(ContractError::InsufficientPayment(_))
        ));
        let result_4 = spec.validate_payment(8.into(), price(1, 2));
        assert!(result_4.is_ok());
    }

    fn spec<LpnAmount>(min_asset: LpnAmount, min_transaction: LpnAmount) -> Spec
    where
        LpnAmount: Into<Coin<TestLpn>>,
    {
        let liability = Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(70),
            Percent::from_percent(73),
            Percent::from_percent(75),
            Percent::from_percent(78),
            Percent::from_percent(80),
            Duration::from_hours(1),
        );
        Spec::new(liability, min_asset.into(), min_transaction.into())
    }

    fn price<PaymentC, Lpn>(
        price_payment_currency: PaymentC,
        price_lpn: Lpn,
    ) -> Price<TestPaymentC, TestLpn>
    where
        PaymentC: Into<Coin<TestPaymentC>>,
        Lpn: Into<Coin<TestLpn>>,
    {
        price::total_of(price_payment_currency.into()).is(price_lpn.into())
    }
}

#[cfg(test)]
mod test_validate_close {
    use currencies::{Lpn, PaymentC3};
    use finance::{
        coin::Coin,
        duration::Duration,
        liability::Liability,
        percent::Percent,
        price::{self, Price},
    };

    use crate::{error::ContractError, finance::LpnCoin, position::Spec};

    type TestCurrency = PaymentC3;
    type TestLpn = Lpn;

    #[test]
    fn too_small_amount() {
        let spec = spec(75, 15);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 14.into(), price(1, 1));
        assert!(matches!(
            result_1,
            Err(ContractError::PositionCloseAmountTooSmall(_))
        ));

        let result_2 = spec.validate_close_amount(asset, 6.into(), price(1, 2));
        assert!(matches!(
            result_2,
            Err(ContractError::PositionCloseAmountTooSmall(_))
        ));
    }

    #[test]
    fn amount_as_min_transaction() {
        let spec = spec(85, 15);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 15.into(), price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 5.into(), price(1, 3));
        assert!(result_2.is_ok());
    }

    #[test]
    fn too_big_amount() {
        let spec = spec(25, 1);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 76.into(), price(1, 1));
        assert!(matches!(
            result_1,
            Err(ContractError::PositionCloseAmountTooBig(_))
        ));

        let result_2 = spec.validate_close_amount(asset, 64.into(), price(3, 2));
        assert!(matches!(
            result_2,
            Err(ContractError::PositionCloseAmountTooBig(_))
        ));
    }

    #[test]
    fn amount_as_min_asset() {
        let spec = spec(25, 1);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 75.into(), price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 62.into(), price(3, 2));
        assert!(result_2.is_ok());
    }

    #[test]
    fn valid_amount() {
        let spec = spec(40, 10);
        let asset = 100.into();

        let result_1 = spec.validate_close_amount(asset, 53.into(), price(1, 1));
        assert!(result_1.is_ok());

        let result_2 = spec.validate_close_amount(asset, 89.into(), price(1, 4));
        assert!(result_2.is_ok());
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
            Percent::from_percent(80),
            Duration::from_hours(1),
        );
        Spec::new(liability, min_asset.into(), min_transaction.into())
    }

    fn price<Asset, Lpn>(price_asset: Asset, price_lpn: Lpn) -> Price<TestCurrency, TestLpn>
    where
        Asset: Into<Coin<TestCurrency>>,
        Lpn: Into<Coin<TestLpn>>,
    {
        price::total_of(price_asset.into()).is(price_lpn.into())
    }
}
