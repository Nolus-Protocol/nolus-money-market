use std::{iter, marker::PhantomData, ops::Deref};

use currency::{Currency, CurrencyDef, Group, MemberOf};
use finance::price::{self, base::with_price::WithPrice, Price};
use marketprice::alarms::{errors::AlarmError, AlarmsIterator, PriceAlarms};
use sdk::cosmwasm_std::{Addr, Storage};

use crate::{contract::alarms::PriceResult, error::ContractError, result::ContractResult};

type AlarmIterMapFn = fn(Result<Addr, AlarmError>) -> ContractResult<Addr>;
type AlarmIter<'alarms, G> = iter::Map<AlarmsIterator<'alarms, G>, AlarmIterMapFn>;

pub struct Iter<'storage, 'alarms, S, I, PriceG, BaseC, BaseG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG, BaseC, BaseG>>,
    PriceG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
    BaseG: Group,
{
    alarms: &'alarms PriceAlarms<'storage, PriceG, S>,
    price_iter: I,
    alarm_iter: Option<AlarmIter<'alarms, PriceG>>,
}

impl<'storage, 'alarms, S, I, PriceG, BaseC, BaseG>
    Iter<'storage, 'alarms, S, I, PriceG, BaseC, BaseG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG, BaseC, BaseG>>,
    PriceG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
    BaseG: Group,
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
            .map(|price_result| {
                price_result.and_then(|ref price| {
                    price::base::with_price::execute(
                        price,
                        Cmd {
                            alarms: self.alarms,
                            _base_c: PhantomData,
                        },
                    )
                })
            })
            .transpose()
    }

    fn next_alarm(&mut self) -> Option<ContractResult<Addr>> {
        match self.alarm_iter.as_mut() {
            None => unimplemented!("calling 'next_alarm' on Some price alarms"),
            Some(iter) => iter.next(),
        }
    }
}

impl<'storage, S, I, PriceG, BaseC, BaseG> Iterator
    for Iter<'storage, '_, S, I, PriceG, BaseC, BaseG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<PriceG, BaseC, BaseG>>,
    PriceG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
    BaseG: Group,
{
    type Item = ContractResult<Addr>;

    #[expect(tail_expr_drop_order)] // TODO remove once stop linting with the 'rust-2024-compatibility' group
    fn next(&mut self) -> Option<Self::Item> {
        let mut result = None;

        while let Some(ref mut alarms_iter) = self.alarm_iter {
            result = alarms_iter.next();

            if result.is_some() {
                break;
            }

            #[cfg(debug_assertions)]
            {
                self.alarm_iter = None;
            }

            self.alarm_iter = match self.next_alarms() {
                Ok(iter) => iter,
                Err(error) => {
                    result = Some(Err(error));

                    None
                }
            };
        }

        result
    }
}

struct Cmd<'storage, 'alarms, S, PriceG, BaseC>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    PriceG: Group + Clone,
    BaseC: Currency,
{
    alarms: &'alarms PriceAlarms<'storage, PriceG, S>,
    _base_c: PhantomData<BaseC>,
}

impl<'storage, 'alarms, S, PriceG, BaseC> WithPrice<BaseC>
    for Cmd<'storage, 'alarms, S, PriceG, BaseC>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    PriceG: Group,
    BaseC: CurrencyDef,
{
    type PriceG = PriceG;

    type Output = AlarmIter<'alarms, PriceG>;
    type Error = ContractError;

    fn exec<C>(self, price: Price<C, BaseC>) -> Result<Self::Output, Self::Error>
    where
        C: CurrencyDef,
        C::Group: MemberOf<Self::PriceG>,
    {
        Ok(self
            .alarms
            .alarms(price)
            .map(|result| result.map_err(Into::into)))
    }
}
