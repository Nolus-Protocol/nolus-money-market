use currency::{
    self, AnyVisitor, AnyVisitorResult, Currency, GroupVisit, SymbolOwned, SymbolSlice, Tickers,
};
use finance::price::{
    dto::{with_price, WithPrice},
    Price,
};
use sdk::{
    cosmwasm_std::{Addr, Storage, Timestamp},
    cw_storage_plus::Map,
};

use crate::{config::Config, error::PriceFeedsError, feed::PriceFeed, CurrencyGroup, SpotPrice};

pub type PriceFeedBin = Vec<u8>;
pub struct PriceFeeds<'m> {
    storage: Map<'m, (SymbolOwned, SymbolOwned), PriceFeedBin>,
    config: Config,
}

impl<'m> PriceFeeds<'m> {
    pub const fn new(namespace: &'m str, config: Config) -> Self {
        Self {
            storage: Map::new(namespace),
            config,
        }
    }

    pub fn feed(
        &self,
        storage: &mut dyn Storage,
        at: Timestamp,
        sender_raw: &Addr,
        prices: &[SpotPrice],
    ) -> Result<(), PriceFeedsError> {
        for price in prices {
            self.storage.update(
                storage,
                (
                    price.base().ticker().to_string(),
                    price.quote().ticker().to_string(),
                ),
                |feed: Option<PriceFeedBin>| -> Result<PriceFeedBin, PriceFeedsError> {
                    add_observation(
                        feed,
                        sender_raw,
                        at,
                        price,
                        self.config.feed_valid_since(at),
                    )
                },
            )?;
        }

        Ok(())
    }

    pub fn price<'a, QuoteC, Iter>(
        &'m self,
        storage: &'a dyn Storage,
        at: Timestamp,
        total_feeders: usize,
        leaf_to_root: Iter,
    ) -> Result<SpotPrice, PriceFeedsError>
    where
        'm: 'a,
        QuoteC: Currency,
        Iter: DoubleEndedIterator<Item = &'a SymbolSlice>,
    {
        let mut root_to_leaf = leaf_to_root.rev();
        let _root = root_to_leaf.next();
        debug_assert_eq!(_root, Some(QuoteC::TICKER));
        PriceCollect::do_collect(
            root_to_leaf,
            self,
            storage,
            at,
            total_feeders,
            Price::<QuoteC, QuoteC>::identity(),
        )
    }

    pub fn price_of_feed<C, QuoteC>(
        &self,
        storage: &dyn Storage,
        at: Timestamp,
        total_feeders: usize,
    ) -> Result<Price<C, QuoteC>, PriceFeedsError>
    where
        C: Currency,
        QuoteC: Currency,
    {
        let feed_bin = self
            .storage
            .may_load(storage, (C::TICKER.into(), QuoteC::TICKER.into()))?;
        load_feed(feed_bin).and_then(|feed| feed.calc_price(&self.config, at, total_feeders))
    }
}

fn load_feed<BaseC, QuoteC>(
    feed_bin: Option<PriceFeedBin>,
) -> Result<PriceFeed<BaseC, QuoteC>, PriceFeedsError>
where
    BaseC: Currency,
    QuoteC: Currency,
{
    feed_bin.map_or_else(
        || Ok(PriceFeed::<BaseC, QuoteC>::default()),
        |bin| postcard::from_bytes(&bin).map_err(Into::into),
    )
}
struct PriceCollect<'a, Iter, BaseC, QuoteC>
where
    Iter: Iterator<Item = &'a SymbolSlice>,
    BaseC: Currency,
    QuoteC: Currency,
{
    currency_path: Iter,
    feeds: &'a PriceFeeds<'a>,
    storage: &'a dyn Storage,
    at: Timestamp,
    total_feeders: usize,
    price: Price<BaseC, QuoteC>,
}
impl<'a, Iter, BaseC, QuoteC> PriceCollect<'a, Iter, BaseC, QuoteC>
where
    Iter: Iterator<Item = &'a SymbolSlice>,
    BaseC: Currency,
    QuoteC: Currency,
{
    fn do_collect(
        mut currency_path: Iter,
        feeds: &'a PriceFeeds<'a>,
        storage: &'a dyn Storage,
        at: Timestamp,
        total_feeders: usize,
        price: Price<BaseC, QuoteC>,
    ) -> Result<SpotPrice, PriceFeedsError> {
        if let Some(next_currency) = currency_path.next() {
            let next_collect = PriceCollect {
                currency_path,
                feeds,
                storage,
                at,
                total_feeders,
                price,
            };
            Tickers.visit_any::<CurrencyGroup, _>(next_currency, next_collect)
        } else {
            Ok(price.into())
        }
    }
}
impl<'a, Iter, QuoteC, QuoteQuoteC> AnyVisitor for PriceCollect<'a, Iter, QuoteC, QuoteQuoteC>
where
    Iter: Iterator<Item = &'a SymbolSlice>,
    QuoteC: Currency,
    QuoteQuoteC: Currency,
{
    type Output = SpotPrice;
    type Error = PriceFeedsError;

    fn on<C>(self) -> AnyVisitorResult<Self>
    where
        C: Currency,
    {
        let next_price =
            self.feeds
                .price_of_feed::<C, _>(self.storage, self.at, self.total_feeders)?;
        let total_price = next_price * self.price;
        PriceCollect::do_collect(
            self.currency_path,
            self.feeds,
            self.storage,
            self.at,
            self.total_feeders,
            total_price,
        )
    }
}

fn add_observation(
    feed_bin: Option<PriceFeedBin>,
    from: &Addr,
    at: Timestamp,
    price: &SpotPrice,
    valid_since: Timestamp,
) -> Result<PriceFeedBin, PriceFeedsError> {
    debug_assert!(valid_since < at);
    struct AddObservation<'a> {
        feed_bin: Option<PriceFeedBin>,
        from: &'a Addr,
        at: Timestamp,
        valid_since: Timestamp,
    }

    impl<'a> WithPrice for AddObservation<'a> {
        type Output = PriceFeedBin;
        type Error = PriceFeedsError;

        fn exec<C, QuoteC>(self, price: Price<C, QuoteC>) -> Result<Self::Output, Self::Error>
        where
            C: Currency,
            QuoteC: Currency,
        {
            load_feed(self.feed_bin).and_then(|feed| {
                let feed =
                    feed.add_observation(self.from.clone(), self.at, price, self.valid_since);
                postcard::to_allocvec(&feed).map_err(Into::into)
            })
        }
    }
    with_price::execute(
        price,
        AddObservation {
            feed_bin,
            from,
            at,
            valid_since,
        },
    )
}

#[cfg(test)]
mod test {
    use currencies::test::{PaymentC1, PaymentC3, PaymentC4, PaymentC5, PaymentC6, PaymentC7};
    use currency::Currency;
    use finance::{
        coin::Coin,
        duration::Duration,
        percent::Percent,
        price::{self, Price},
    };
    use sdk::cosmwasm_std::{Addr, MemoryStorage, Timestamp};

    use crate::{error::PriceFeedsError, market_price::Config, SpotPrice};

    use super::PriceFeeds;

    const FEEDS_NAMESPACE: &str = "feeds";
    const FEEDER: &str = "0xifeege";
    const TOTAL_FEEDERS: usize = 1;
    const FEED_VALIDITY: Duration = Duration::from_secs(30);
    const SAMPLE_PERIOD_SECS: Duration = Duration::from_secs(5);
    const SAMPLES_NUMBER: u16 = 6;
    const DISCOUNTING_FACTOR: Percent = Percent::from_permille(750);

    const NOW: Timestamp = Timestamp::from_seconds(FEED_VALIDITY.secs() * 2);

    #[test]
    fn no_feed() {
        let feeds = PriceFeeds::new(FEEDS_NAMESPACE, config());
        let storage = MemoryStorage::new();

        assert_eq!(
            Ok(Price::<PaymentC3, PaymentC3>::identity().into()),
            feeds.price::<PaymentC3, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC3::TICKER].into_iter()
            )
        );

        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feeds.price::<PaymentC3, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC3::TICKER].into_iter()
            )
        );
    }

    #[test]
    fn feed_pair() {
        let feeds = PriceFeeds::new(FEEDS_NAMESPACE, config());
        let mut storage = MemoryStorage::new();
        let new_price: SpotPrice = price::total_of(Coin::<PaymentC7>::new(1))
            .is(Coin::<PaymentC4>::new(18500))
            .into();

        feeds
            .feed(
                &mut storage,
                NOW,
                &Addr::unchecked(FEEDER),
                &[new_price.clone()],
            )
            .unwrap();

        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feeds.price::<PaymentC3, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC3::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok(new_price),
            feeds.price::<PaymentC4, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC4::TICKER].into_iter()
            )
        );
    }

    #[test]
    fn feed_pairs() {
        let feeds = PriceFeeds::new(FEEDS_NAMESPACE, config());
        let mut storage = MemoryStorage::new();
        let new_price12 = price::total_of(Coin::<PaymentC7>::new(1)).is(Coin::<PaymentC5>::new(2));
        let new_price23 = price::total_of(Coin::<PaymentC5>::new(1)).is(Coin::<PaymentC6>::new(3));
        let new_price24 = price::total_of(Coin::<PaymentC5>::new(1)).is(Coin::<PaymentC4>::new(4));

        feeds
            .feed(
                &mut storage,
                NOW,
                &Addr::unchecked(FEEDER),
                &[new_price24.into(), new_price12.into(), new_price23.into()],
            )
            .unwrap();

        assert_eq!(
            Err(PriceFeedsError::NoPrice()),
            feeds.price::<PaymentC1, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC1::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok(new_price12.into()),
            feeds.price::<PaymentC5, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC5::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok(new_price23.into()),
            feeds.price::<PaymentC6, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC5::TICKER, PaymentC6::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok(new_price24.into()),
            feeds.price::<PaymentC4, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC5::TICKER, PaymentC4::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok((new_price12 * new_price23).into()),
            feeds.price::<PaymentC6, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC5::TICKER, PaymentC6::TICKER].into_iter()
            )
        );
        assert_eq!(
            Ok((new_price12 * new_price24).into()),
            feeds.price::<PaymentC4, _>(
                &storage,
                NOW,
                TOTAL_FEEDERS,
                [PaymentC7::TICKER, PaymentC5::TICKER, PaymentC4::TICKER].into_iter()
            )
        );
    }

    fn config() -> Config {
        Config::new(
            Percent::HUNDRED,
            SAMPLE_PERIOD_SECS,
            SAMPLES_NUMBER,
            DISCOUNTING_FACTOR,
        )
    }
}
