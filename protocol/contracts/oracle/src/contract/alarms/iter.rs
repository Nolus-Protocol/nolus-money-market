use std::{iter, ops::Deref};

use currencies::Lpn;
use serde::{de::DeserializeOwned, Serialize};

use currency::{AnyVisitor, AnyVisitorResult, Currency, Group, GroupVisit, Tickers};
use finance::price::{base::BasePrice, Price};
use marketprice::alarms::{errors::AlarmError, AlarmsIterator, PriceAlarms};
use sdk::cosmwasm_std::{Addr, Storage};

use crate::{contract::alarms::PriceResult, error::ContractError, result::ContractResult};

type AlarmIterMapFn = fn(Result<Addr, AlarmError>) -> ContractResult<Addr>;
type AlarmIter<'alarms, G> = iter::Map<AlarmsIterator<'alarms, G>, AlarmIterMapFn>;

pub struct Iter<'storage, 'alarms, S, I, PriceG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG>>,
    PriceG: Group + Clone,
{
    alarms: &'alarms PriceAlarms<'storage, PriceG, S>,
    price_iter: I,
    alarm_iter: Option<AlarmIter<'alarms, PriceG>>,
}

impl<'storage, 'alarms, S, I, PriceG> Iter<'storage, 'alarms, S, I, PriceG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG>>,
    PriceG: Group + Clone,
{
    pub fn new(
        alarms: &'alarms PriceAlarms<'storage, PriceG, S>,
        price_iter: I,
    ) -> ContractResult<Self> {
        let mut iter = Self {
            alarms,
            price_iter,
            alarm_iter: None,
        };
        iter.alarm_iter = iter.next_alarms()?;
        Ok(iter)
    }

    fn move_to_next_alarms(&mut self) -> ContractResult<()> {
        debug_assert!(self.next_alarm().is_none());

        self.alarm_iter = self.next_alarms()?;
        Ok(())
    }

    fn next_alarms(&mut self) -> ContractResult<Option<AlarmIter<'alarms, PriceG>>> {
        self.price_iter
            .next()
            .map(|price_result: PriceResult<PriceG>| {
                price_result.and_then(|ref price| {
                    Tickers.visit_any::<PriceG, Cmd<'storage, 'alarms, '_, S, PriceG>>(
                        price.base_ticker(),
                        Cmd {
                            alarms: self.alarms,
                            price,
                        },
                    )
                })
            })
            .transpose()
    }

    #[allow(clippy::unwrap_in_result)]
    fn next_alarm(&mut self) -> Option<ContractResult<Addr>> {
        debug_assert!(self.alarm_iter.is_some());
        self.alarm_iter
            .as_mut()
            .expect("calling 'next_alarm' on Some price alarms")
            .next()
    }
}

impl<'storage, 'alarms, S, I, PriceG> Iterator for Iter<'storage, 'alarms, S, I, PriceG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG>>,
    PriceG: Group + Clone,
{
    type Item = ContractResult<Addr>;

    fn next(&mut self) -> Option<Self::Item> {
        self.alarm_iter.as_ref()?;

        let mut result = self.next_alarm();
        while result.is_none() && self.alarm_iter.is_some() {
            result = if let Err(error) = self.move_to_next_alarms() {
                Some(Err(error))
            } else if self.alarm_iter.is_none() {
                None
            } else {
                self.next_alarm()
            }
        }
        result
    }
}

struct Cmd<'storage, 'alarms, 'price, S, PriceG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    PriceG: Group + Clone,
{
    alarms: &'alarms PriceAlarms<'storage, PriceG, S>,
    price: &'price BasePrice<PriceG, Lpn>,
}

impl<'storage, 'alarms, 'price, S, PriceG> AnyVisitor for Cmd<'storage, 'alarms, 'price, S, PriceG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    PriceG: Group + Clone,
{
    type Output = AlarmIter<'alarms, PriceG>;
    type Error = ContractError;

    fn on<C>(self) -> AnyVisitorResult<Self>
    where
        C: Currency + Serialize + DeserializeOwned,
    {
        Price::<C, Lpn>::try_from(self.price)
            .map(|price: Price<C, Lpn>| {
                self.alarms
                    .alarms(price)
                    .map::<ContractResult<Addr>, AlarmIterMapFn>(
                        |result: Result<Addr, AlarmError>| result.map_err(Into::into),
                    )
            })
            .map_err(ContractError::from)
    }
}
