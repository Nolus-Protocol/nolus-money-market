use std::{iter, marker::PhantomData, ops::Deref};

use currency::{Currency, CurrencyDef, Group, MemberOf};
use finance::price::{self, base::with_price::WithPrice, Price};
use marketprice::alarms::{errors::AlarmError, AlarmsIterator, PriceAlarms};
use sdk::cosmwasm_std::{Addr, Storage};

use crate::{contract::alarms::PriceResult, error::Error, result::Result};

type AlarmIterMapFn<ErrorG> = fn(std::result::Result<Addr, AlarmError>) -> Result<Addr, ErrorG>;
type AlarmIter<'alarms, AlarmG, ErrorG> =
    iter::Map<AlarmsIterator<'alarms, AlarmG>, AlarmIterMapFn<ErrorG>>;

pub struct Iter<'storage, 'alarms, S, I, AlarmsG, BaseC, BaseG, ErrorG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<AlarmsG, BaseC, BaseG, ErrorG>>,
    AlarmsG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<AlarmsG::TopG>,
    BaseG: Group,
    ErrorG: Group,
{
    alarms: &'alarms PriceAlarms<'storage, AlarmsG, S>,
    price_iter: I,
    alarm_iter: Option<AlarmIter<'alarms, AlarmsG, ErrorG>>,
}

impl<'storage, 'alarms, S, I, AlarmsG, BaseC, BaseG, ErrorG>
    Iter<'storage, 'alarms, S, I, AlarmsG, BaseC, BaseG, ErrorG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<AlarmsG, BaseC, BaseG, ErrorG>>,
    AlarmsG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<AlarmsG::TopG>,
    BaseG: Group,
    ErrorG: Group,
{
    pub fn new(
        alarms: &'alarms PriceAlarms<'storage, AlarmsG, S>,
        price_iter: I,
    ) -> Result<Self, ErrorG> {
        let mut iter = Self {
            alarms,
            price_iter,
            alarm_iter: None,
        };

        #[expect(if_let_rescope)]
        // TODO remove once stop linting with the 'rust-2024-compatibility' group
        if let Some(alarms_iter) = iter.next_alarms() {
            alarms_iter.map(|alarms_iter| {
                iter.alarm_iter = Some(alarms_iter);

                iter
            })
        } else {
            Ok(iter)
        }
    }

    fn next_alarms(&mut self) -> Option<Result<AlarmIter<'alarms, AlarmsG, ErrorG>, ErrorG>> {
        debug_assert!(self.alarm_iter.is_none());

        self.price_iter.next().map(|price_result| {
            price_result.and_then(|price| {
                price::base::with_price::execute(
                    &price,
                    Cmd {
                        alarms: self.alarms,
                        _base_c: PhantomData,
                        _error_g: PhantomData,
                    },
                )
            })
        })
    }
}

impl<'storage, S, I, AlarmsG, BaseC, BaseG, ErrorG> Iterator
    for Iter<'storage, '_, S, I, AlarmsG, BaseC, BaseG, ErrorG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    I: Iterator<Item = PriceResult<AlarmsG, BaseC, BaseG, ErrorG>>,
    AlarmsG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<AlarmsG::TopG>,
    BaseG: Group,
    ErrorG: Group,
{
    type Item = Result<Addr, ErrorG>;

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
                Some(Ok(iter)) => Some(iter),
                Some(Err(error)) => {
                    result = Some(Err(error));

                    None
                }
                None => None,
            };
        }

        debug_assert!(matches!(
            (&result, &self.alarm_iter),
            (Some(Ok(_)), Some(_)) | (Some(Err(_)), None) | (None, None)
        ));

        result
    }
}

struct Cmd<'storage, 'alarms, S, AlarmsG, BaseC, ErrorG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    AlarmsG: Group + Clone,
    BaseC: Currency,
    ErrorG: Group,
{
    alarms: &'alarms PriceAlarms<'storage, AlarmsG, S>,
    _base_c: PhantomData<BaseC>,
    _error_g: PhantomData<ErrorG>,
}

impl<'storage, 'alarms, S, AlarmsG, BaseC, ErrorG> WithPrice<BaseC>
    for Cmd<'storage, 'alarms, S, AlarmsG, BaseC, ErrorG>
where
    S: Deref<Target = (dyn Storage + 'storage)>,
    AlarmsG: Group,
    BaseC: CurrencyDef,
    ErrorG: Group,
{
    type PriceG = AlarmsG;

    type Output = AlarmIter<'alarms, AlarmsG, ErrorG>;

    type Error = Error<ErrorG>;

    fn exec<C>(self, price: Price<C, BaseC>) -> std::result::Result<Self::Output, Self::Error>
    where
        C: CurrencyDef,
        C::Group: MemberOf<Self::PriceG>,
    {
        Ok(self
            .alarms
            .alarms(price)
            .map(|alarm_result| alarm_result.map_err(Into::into)))
    }
}
