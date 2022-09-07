use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
};

use cosmwasm_std::{Addr, DepsMut, StdError, StdResult, Storage, Timestamp};
use marketprice::{
    feeders::{PriceFeeders, PriceFeedersError},
    market_price::{PriceFeeds, PriceFeedsError, PriceQuery},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use std::convert::TryFrom;

use finance::{
    currency::{Currency, SymbolOwned},
    duration::Duration,
    price::{Price as FinPrice, PriceDTO},
};

use crate::{state::config::Config, ContractError};
const PRECISION_FACTOR: u128 = 1_000_000_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MarketOracle {
    config: Config,
}

impl MarketOracle {
    const FEEDERS: PriceFeeders<'static> = PriceFeeders::new("feeders");
    const MARKET_PRICE: PriceFeeds<'static> = PriceFeeds::new("market_price");

    pub fn new(config: Config) -> Self {
        Self { config }
    }
    pub fn get_feeders(storage: &dyn Storage) -> StdResult<HashSet<Addr>> {
        Self::FEEDERS.get(storage)
    }

    pub fn is_feeder(storage: &dyn Storage, address: &Addr) -> StdResult<bool> {
        Self::FEEDERS.is_registered(storage, address)
    }

    pub fn register_feeder(deps: DepsMut, address: Addr) -> Result<(), PriceFeedersError> {
        Self::FEEDERS.register(deps, address)
    }

    fn init_price_query(
        storage: &dyn Storage,
        base: SymbolOwned,
        config: &Config,
    ) -> StdResult<PriceQuery> {
        Self::assert_supported_denom(&config.supported_denom_pairs, base.clone())?;

        let registered_feeders = Self::FEEDERS.get(storage)?;
        let all_feeders_cnt = registered_feeders.len();
        let feeders_needed =
            Self::feeders_needed(all_feeders_cnt, config.feeders_percentage_needed);

        Ok(PriceQuery::new(
            (base, config.base_asset.clone()),
            Duration::from_secs(config.price_feed_period_secs),
            feeders_needed,
        ))
    }

    fn assert_supported_denom(
        supported_denom_pairs: &[(SymbolOwned, SymbolOwned)],
        currency: SymbolOwned,
    ) -> StdResult<()> {
        let mut all_supported_denoms = HashSet::<SymbolOwned>::new();
        for pair in supported_denom_pairs {
            all_supported_denoms.insert(pair.0.clone());
            all_supported_denoms.insert(pair.1.clone());
        }
        if !all_supported_denoms.contains(&currency) {
            return Err(StdError::generic_err("Unsupported denom"));
        }
        Ok(())
    }

    pub fn get_prices(
        &self,
        storage: &dyn Storage,
        block_time: Timestamp,
        denoms: HashSet<SymbolOwned>,
    ) -> Result<HashMap<SymbolOwned, PriceDTO>, PriceFeedsError> {
        let mut prices: HashMap<SymbolOwned, PriceDTO> = HashMap::new();
        for denom in denoms {
            let price_query = Self::init_price_query(storage, denom.clone(), &self.config)?;
            let feed =
                Self::MARKET_PRICE.get_converted_dto_price(storage, block_time, price_query)?;
            prices.insert(denom, feed);
        }
        Ok(prices)
    }

    pub fn get_single_price<C, QuoteC>(
        storage: &dyn Storage,
        block_time: Timestamp,
        currency: SymbolOwned,
    ) -> Result<FinPrice<C, QuoteC>, PriceFeedsError>
    where
        C: Currency,
        QuoteC: Currency,
    {
        let config = Config::load(storage)?;

        let price_query = Self::init_price_query(storage, currency, &config)?;
        let price = Self::MARKET_PRICE.get_converted_price(storage, block_time, price_query)?;

        Ok(price)
    }

    pub fn feed_prices(
        &self,
        storage: &mut dyn Storage,
        block_time: Timestamp,
        sender_raw: &Addr,
        prices: Vec<PriceDTO>,
    ) -> Result<(), ContractError> {
        // FIXME: add the check again once the supported pairs implementation is done
        // let filtered_prices = self.remove_invalid_prices(prices);
        // if filtered_prices.is_empty() {
        //     return Err(ContractError::UnsupportedDenomPairs {});
        // }

        Self::MARKET_PRICE.feed(
            storage,
            block_time,
            sender_raw,
            prices,
            Duration::from_secs(self.config.price_feed_period_secs),
        )?;

        Ok(())
    }

    // this is a helper function so Decimal works with u64 rather than Uint128
    // also, we must *round up* here, as we need 8, not 7 feeders to reach 50% of 15 total
    fn feeders_needed(weight: usize, percentage: u8) -> usize {
        let weight128 = u128::try_from(weight).expect("usize to u128 overflow");
        let res = (PRECISION_FACTOR * weight128) * u128::from(percentage) / 100;
        ((res + PRECISION_FACTOR - 1) / PRECISION_FACTOR)
            .try_into()
            .expect("usize overflow")
    }

    // fn remove_invalid_prices(&self, prices: Vec<PriceDTO>) -> Vec<Price> {
    //     prices
    //         .iter()
    //         .filter(|price| {
    //             self.config
    //                 .supported_denom_pairs
    //                 .contains(&price.denom_pair())
    //                 && !price
    //                     .base()
    //                     .symbol
    //                     .eq_ignore_ascii_case(&price.quote().symbol)
    //         })
    //         .map(|p| p.to_owned())
    //         .collect()
    // }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Addr;

    use crate::{oracle::MarketOracle, state::config::Config};

    #[test]
    // we ensure this rounds up (as it calculates needed votes)
    fn feeders_needed_rounds_properly() {
        // round up right below 1
        assert_eq!(8, MarketOracle::feeders_needed(3, 255));
        // round up right over 1
        assert_eq!(8, MarketOracle::feeders_needed(3, 254));
        assert_eq!(77, MarketOracle::feeders_needed(30, 254));

        // exact matches don't round
        assert_eq!(17, MarketOracle::feeders_needed(34, 50));
        assert_eq!(12, MarketOracle::feeders_needed(48, 25));
        assert_eq!(2, MarketOracle::feeders_needed(132, 1));
        assert_eq!(2, MarketOracle::feeders_needed(189, 1));
    }

    // #[test]
    // fn test_remove_invalid_prices() {
    //     let supported_pairs = vec![
    //         ("A".to_string(), "B".to_string()),
    //         ("A".to_string(), "C".to_string()),
    //         ("B".to_string(), "A".to_string()),
    //         ("C".to_string(), "D".to_string()),
    //     ];

    //     let prices = vec![
    //         Price::new("B", 10, "A", 12),
    //         Price::new("B", 10, "D", 32),
    //         Price::new("B", 10, "B", 12),
    //     ];

    //     let filtered = MarketOracle::new(Config::new(
    //         "denom".to_string(),
    //         Addr::unchecked("owner"),
    //         20,
    //         5,
    //         supported_pairs,
    //         Addr::unchecked("timealarms_contract"),
    //     ))
    //     .remove_invalid_prices(prices);

    //     assert_eq!(vec![Price::new("B", 10, "A", 12),], filtered);
    // }
}
