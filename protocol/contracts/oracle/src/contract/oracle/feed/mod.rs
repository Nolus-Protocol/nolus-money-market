use currencies::{Lpn, Lpns};
use currency::{Group, SymbolOwned};
use finance::price::dto::PriceDTO;
use marketprice::{config::Config, market_price::PriceFeeds};
use sdk::cosmwasm_std::{Addr, Storage, Timestamp};

use crate::{
    api::{swap::SwapTarget, SwapLeg},
    error::ContractError,
    state::supported_pairs::SupportedPairs,
};

use super::PriceResult;

use self::{leg_cmd::LegCmd, price_querier::FedPrices};

mod leg_cmd;
mod price_querier;

pub struct Feeds<PriceG> {
    feeds: PriceFeeds<'static, PriceG>,
}

impl<PriceG> Feeds<PriceG>
where
    PriceG: Group,
{
    pub(crate) fn with(config: Config) -> Self {
        Self {
            feeds: PriceFeeds::new("market_price", config),
        }
    }

    pub(crate) fn feed_prices(
        &self,
        storage: &mut dyn Storage,
        block_time: Timestamp,
        sender_raw: &Addr,
        prices: &[PriceDTO<PriceG, PriceG>],
    ) -> Result<(), ContractError> {
        let tree = SupportedPairs::load(storage)?;
        if prices.iter().any(|price| {
            !tree.swap_pairs_df().any(
                |SwapLeg {
                     from,
                     to: SwapTarget { target: to, .. },
                 }| {
                    price.base().ticker() == &from && price.quote().ticker() == &to
                },
            )
        }) {
            return Err(ContractError::UnsupportedDenomPairs {});
        }

        self.feeds.feed(storage, block_time, sender_raw, prices)?;

        Ok(())
    }

    pub fn all_prices_iter<'r, 'self_, 'storage, I>(
        &'self_ self,
        storage: &'storage dyn Storage,
        swap_pairs_df: I,
        at: Timestamp,
        total_feeders: usize,
    ) -> impl Iterator<Item = PriceResult<PriceG>> + 'r
    where
        'self_: 'r,
        'storage: 'r,
        I: Iterator<Item = SwapLeg> + 'r,
    {
        let cmd: LegCmd<PriceG, FedPrices<'_, PriceG>> = LegCmd::new(
            FedPrices::new(storage, &self.feeds, at, total_feeders),
            vec![],
        );

        swap_pairs_df
            .scan(cmd, |cmd, leg| {
                Some(
                    currency::visit_any_on_tickers::<PriceG, PriceG, _>(
                        &leg.from,
                        &leg.to.target,
                        cmd,
                    )
                    .transpose(),
                )
            })
            .flatten()
    }

    pub fn calc_price(
        &self,
        storage: &dyn Storage,
        tree: &SupportedPairs,
        currency: &SymbolOwned,
        at: Timestamp,
        total_feeders: usize,
    ) -> Result<PriceDTO<PriceG, Lpns>, ContractError> {
        self.feeds
            .price::<Lpn, _, _>(storage, at, total_feeders, tree.load_path(currency)?)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use serde::de::DeserializeOwned;

    use currencies::test::{PaymentC1, PaymentC3, PaymentC4, PaymentC5, PaymentC6, PaymentC7};
    use currency::{Currency, SymbolStatic};
    use finance::{
        coin::Amount,
        duration::Duration,
        percent::Percent,
        price::{dto::PriceDTO, Price},
    };
    use price_querier::PriceQuerier;
    use sdk::cosmwasm_std::{
        self,
        testing::{self, MockStorage},
    };
    use tree::HumanReadableTree;

    use crate::{
        api::PriceCurrencies,
        tests::{self, TheCurrency},
    };

    use super::*;

    #[derive(Clone)]
    pub struct TestFeeds(
        pub HashMap<(SymbolStatic, SymbolStatic), PriceDTO<PriceCurrencies, PriceCurrencies>>,
    );

    impl TestFeeds {
        pub fn add<B, Q>(&mut self, total_of: Amount, is: Amount)
        where
            B: Currency,
            Q: Currency,
        {
            self.0.insert(
                (B::TICKER, Q::TICKER),
                tests::dto_price::<B, _, Q, _>(total_of, is),
            );
        }
    }

    impl PriceQuerier for TestFeeds {
        fn price<B, Q>(&self) -> Result<Option<Price<B, Q>>, ContractError>
        where
            B: Currency + DeserializeOwned,
            Q: Currency + DeserializeOwned,
        {
            Ok(self
                .0
                .get(&(B::TICKER, Q::TICKER))
                .map(Price::try_from)
                .transpose()?)
        }
    }

    fn test_case() -> HumanReadableTree<SwapTarget> {
        let base = TheCurrency::TICKER;
        let osmo = PaymentC5::TICKER;
        let nls = PaymentC1::TICKER;
        let weth = PaymentC7::TICKER;
        let atom = PaymentC3::TICKER;
        let axl = PaymentC4::TICKER;
        let cro = PaymentC6::TICKER;

        cosmwasm_std::from_json(format!(
            r#"
            {{
                "value":[0,"{base}"],
                "children":[
                    {{
                        "value":[4,"{atom}"],
                        "children":[
                            {{"value":[3,"{weth}"]}}
                        ]
                    }},
                    {{
                        "value":[2,"{nls}"],
                        "children":[
                            {{
                                "value":[1,"{osmo}"],
                                "children":[
                                    {{"value":[5,"{axl}"]}},
                                    {{"value":[6,"{cro}"]}}
                                ]
                            }}
                        ]
                    }}
                ]
            }}"#
        ))
        .unwrap()
    }

    mod all_prices_iter {
        use finance::price::base::BasePrice;

        use crate::tests::PriceGroup;

        use super::*;

        #[test]
        fn normal() {
            let mut storage = MockStorage::new();
            let env = testing::mock_env();
            let tree = test_case();
            let tree = SupportedPairs::new(tree.into_tree(), TheCurrency::TICKER.into()).unwrap();
            tree.save(&mut storage).unwrap();

            let config = Config::new(
                Percent::HUNDRED,
                Duration::from_secs(5),
                10,
                Percent::from_percent(50),
            );

            let oracle = Feeds::<PriceGroup>::with(config);

            oracle
                .feed_prices(
                    &mut storage,
                    env.block.time,
                    &Addr::unchecked("feeder"),
                    &[
                        tests::dto_price::<PaymentC3, _, TheCurrency, _>(1, 1),
                        tests::dto_price::<PaymentC1, _, TheCurrency, _>(2, 1),
                        tests::dto_price::<PaymentC7, _, PaymentC3, _>(1, 1),
                        tests::dto_price::<PaymentC5, _, PaymentC1, _>(1, 1),
                        tests::dto_price::<PaymentC6, _, PaymentC5, _>(3, 1),
                        tests::dto_price::<PaymentC4, _, PaymentC5, _>(1, 1),
                    ],
                )
                .unwrap();

            let prices: Vec<_> = oracle
                .all_prices_iter(&storage, tree.swap_pairs_df(), env.block.time, 1)
                .flatten()
                .collect();

            let expected: Vec<BasePrice<PriceCurrencies, TheCurrency>> = vec![
                tests::base_price::<PaymentC3>(1, 1),
                tests::base_price::<PaymentC7>(1, 1),
                tests::base_price::<PaymentC1>(2, 1),
                tests::base_price::<PaymentC5>(2, 1),
                tests::base_price::<PaymentC4>(2, 1),
                tests::base_price::<PaymentC6>(6, 1),
            ];

            assert_eq!(expected, prices);
        }

        #[test]
        fn missing_price() {
            let mut storage = MockStorage::new();
            let env = testing::mock_env();
            let tree = test_case();
            let tree = SupportedPairs::new(tree.into_tree(), TheCurrency::TICKER.into()).unwrap();
            tree.save(&mut storage).unwrap();

            let config = Config::new(
                Percent::HUNDRED,
                Duration::from_secs(5),
                10,
                Percent::from_percent(50),
            );

            let oracle = Feeds::<PriceGroup>::with(config);

            oracle
                .feed_prices(
                    &mut storage,
                    env.block.time,
                    &Addr::unchecked("feeder"),
                    &[
                        // tests::dto_price::<PaymentC3, TheCurrency>(1, 1),
                        tests::dto_price::<PaymentC1, _, TheCurrency, _>(2, 1),
                        tests::dto_price::<PaymentC7, _, PaymentC3, _>(1, 1),
                        tests::dto_price::<PaymentC5, _, PaymentC1, _>(1, 1),
                        tests::dto_price::<PaymentC6, _, PaymentC5, _>(3, 1),
                        tests::dto_price::<PaymentC4, _, PaymentC5, _>(1, 1),
                    ],
                )
                .unwrap();

            let expected: Vec<BasePrice<PriceCurrencies, TheCurrency>> = vec![
                tests::base_price::<PaymentC1>(2, 1),
                tests::base_price::<PaymentC5>(2, 1),
                tests::base_price::<PaymentC4>(2, 1),
                tests::base_price::<PaymentC6>(6, 1),
            ];

            let prices: Vec<_> = oracle
                .all_prices_iter(&storage, tree.swap_pairs_df(), env.block.time, 1)
                .collect::<Result<_, _>>()
                .unwrap();

            assert_eq!(prices, expected);
        }
    }
}
