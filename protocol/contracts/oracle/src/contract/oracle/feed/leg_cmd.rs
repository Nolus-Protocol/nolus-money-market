use currency::{AnyVisitorPair, Currency, CurrencyDTO, CurrencyDef, Group, MemberOf};
use finance::price::{base::BasePrice, Price};

use crate::error::Error;

use super::price_querier::PriceQuerier;

pub struct LegCmd<PriceG, BaseC, BaseG, Querier>
where
    PriceG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
    BaseG: Group,
    Querier: PriceQuerier,
{
    price_querier: Querier,
    stack: Vec<BasePrice<PriceG, BaseC, BaseG>>,
}

impl<PriceG, BaseC, BaseG, Querier> LegCmd<PriceG, BaseC, BaseG, Querier>
where
    PriceG: Group<TopG = PriceG>,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG>,
    BaseG: Group + MemberOf<PriceG>,
    Querier: PriceQuerier,
{
    pub fn new(price_querier: Querier) -> Self {
        Self {
            price_querier,
            stack: vec![Price::<BaseC, BaseC>::identity().into()],
        }
    }
}

impl<PriceG, BaseC, BaseG, Querier> AnyVisitorPair for &mut LegCmd<PriceG, BaseC, BaseG, Querier>
where
    PriceG: Group<TopG = PriceG>,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG>,
    BaseG: Group + MemberOf<PriceG>,
    Querier: PriceQuerier<CurrencyGroup = PriceG>,
{
    type VisitedG = PriceG;

    type Output = Option<BasePrice<PriceG, BaseC, BaseG>>;
    type Error = Error<PriceG>;

    fn on<B, Q>(
        self,
        dto1: &CurrencyDTO<Self::VisitedG>,
        dto2: &CurrencyDTO<Self::VisitedG>,
    ) -> Result<Self::Output, Self::Error>
    where
        B: Currency + MemberOf<Self::VisitedG>,
        Q: Currency + MemberOf<Self::VisitedG>,
    {
        // tries to find price for non empty stack (in a branch of the tree)
        // covers both normal flow and NoPrice cases
        let idx_price = self
            .stack
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, parent_bprice)| {
                parent_bprice
                    .try_as_specific::<Q, Self::VisitedG>(dto2)
                    .ok()
                    .map(|parent_price| {
                        self.price_querier
                            .price::<B, Q>(dto1, dto2)
                            .map(|res_price| {
                                res_price.map(|price| {
                                    (i + 1, BasePrice::from_price(&(price * parent_price), *dto1))
                                })
                            })
                    })
            })
            .transpose()
            .map(Option::flatten)?;

        if let Some((idx, price)) = idx_price {
            self.stack.truncate(idx);
            self.stack.push(price);
        }

        Ok(idx_price.map(|(_, price)| price))
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod test {
    use std::collections::HashMap;

    use currencies::{
        testing::{PaymentC1, PaymentC3, PaymentC4, PaymentC5, PaymentC6},
        Lpn as BaseCurrency, Lpns as BaseCurrencies, PaymentGroup as PaymentCurrencies,
    };

    use crate::tests;

    use super::{super::test::TestFeeds, *};

    #[test]
    fn leg_cmd_normal_flow() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(1, 1);
        feeds.add::<PaymentC1, BaseCurrency>(2, 1);
        feeds.add::<PaymentC5, PaymentC4>(2, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());
        assert_eq!(
            cmd.on::<PaymentC4, BaseCurrency>(
                &currency::dto::<PaymentC4, PaymentCurrencies>(),
                &currency::dto::<BaseCurrency, _>()
            ),
            Ok(Some(tests::base_price::<PaymentC4>(1, 1)))
        );

        // child
        assert_eq!(
            cmd.on::<PaymentC5, PaymentC4>(
                &currency::dto::<PaymentC5, _>(),
                &currency::dto::<PaymentC4, _>()
            ),
            Ok(Some(tests::base_price::<PaymentC5>(2, 1)))
        );

        // hop to the next branch
        assert_eq!(
            cmd.on::<PaymentC1, BaseCurrency>(
                &currency::dto::<PaymentC1, _>(),
                &currency::dto::<BaseCurrency, _>()
            ),
            Ok(Some(tests::base_price::<PaymentC1>(2, 1)))
        );
    }

    #[test]
    fn no_price_in_stack() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(2, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            cmd.on::<PaymentC6, PaymentC4>(
                &currency::dto::<PaymentC6, _>(),
                &currency::dto::<PaymentC4, _>()
            ),
            Ok(None)
        );

        assert_eq!(
            cmd.on::<PaymentC4, BaseCurrency>(
                &currency::dto::<PaymentC4, _>(),
                &currency::dto::<BaseCurrency, _>()
            ),
            Ok(Some(tests::base_price::<PaymentC4>(2, 1)))
        );
    }

    #[test]
    fn no_parent_price_in_stack() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(2, 1);
        feeds.add::<PaymentC5, PaymentC4>(5, 1);
        feeds.add::<PaymentC3, PaymentC5>(3, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            cmd.on::<PaymentC3, PaymentC5>(
                &currency::dto::<PaymentC3, _>(),
                &currency::dto::<PaymentC5, _>()
            ),
            Ok(None)
        );
        assert_eq!(
            cmd.on::<PaymentC5, PaymentC4>(
                &currency::dto::<PaymentC5, _>(),
                &currency::dto::<PaymentC4, _>(),
            ),
            Ok(None)
        );

        assert_eq!(
            cmd.on::<PaymentC4, BaseCurrency>(
                &currency::dto::<PaymentC4, _>(),
                &currency::dto::<BaseCurrency, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC4>(2, 1)))
        );
        assert_eq!(
            cmd.on::<PaymentC5, PaymentC4>(
                &currency::dto::<PaymentC5, _>(),
                &currency::dto::<PaymentC4, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC5>(10, 1)))
        );
        assert_eq!(
            cmd.on::<PaymentC3, PaymentC5>(
                &currency::dto::<PaymentC3, _>(),
                &currency::dto::<PaymentC5, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC3>(30, 1)))
        );
    }

    #[test]
    fn hop_parent_in_stack() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(2, 1);
        feeds.add::<PaymentC5, PaymentC4>(3, 1);
        feeds.add::<PaymentC6, PaymentC4>(4, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            cmd.on::<PaymentC4, BaseCurrency>(
                &currency::dto::<PaymentC4, _>(),
                &currency::dto::<BaseCurrency, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC4>(2, 1)))
        );

        assert_eq!(
            cmd.on::<PaymentC6, PaymentC4>(
                &currency::dto::<PaymentC6, _>(),
                &currency::dto::<PaymentC4, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC6>(8, 1)))
        );

        assert_eq!(
            cmd.on::<PaymentC5, PaymentC4>(
                &currency::dto::<PaymentC5, _>(),
                &currency::dto::<PaymentC4, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC5>(6, 1)))
        );

        assert_eq!(
            cmd.on::<PaymentC6, PaymentC4>(
                &currency::dto::<PaymentC6, _>(),
                &currency::dto::<PaymentC4, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC6>(8, 1)))
        );
    }

    #[test]
    fn hop_to_root() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(2, 1);
        feeds.add::<PaymentC1, BaseCurrency>(4, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            cmd.on::<PaymentC1, BaseCurrency>(
                &currency::dto::<PaymentC1, _>(),
                &currency::dto::<BaseCurrency, _>(),
            ),
            Ok(Some(tests::base_price::<PaymentC1>(4, 1)))
        );
    }

    #[test]
    fn price_root_with_empty_stack() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC1, BaseCurrency>(4, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            Ok(Some(tests::base_price::<PaymentC1>(4, 1))),
            cmd.on::<PaymentC1, BaseCurrency>(
                &currency::dto::<PaymentC1, _>(),
                &currency::dto::<BaseCurrency, _>(),
            ),
        );
    }

    #[test]
    fn no_price_at_root() {
        let mut feeds = TestFeeds(HashMap::new());
        feeds.add::<PaymentC4, BaseCurrency>(2, 1);

        let mut cmd =
            LegCmd::<PaymentCurrencies, BaseCurrency, BaseCurrencies, _>::new(feeds.clone());

        assert_eq!(
            cmd.on::<PaymentC1, BaseCurrency>(
                &currency::dto::<PaymentC1, _>(),
                &currency::dto::<BaseCurrency, _>(),
            ),
            Ok(None)
        );
    }
}
