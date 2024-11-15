use std::ops::{Deref, DerefMut};

use currency::{CurrencyDef, Group, MemberOf};
use finance::price::{
    base::{
        with_price::{self, WithPrice},
        BasePrice,
    },
    Price,
};
use marketprice::alarms::PriceAlarms;
use sdk::cosmwasm_std::{Addr, Storage};

use crate::{api::Alarm as AlarmDTO, error::ContractError, result::ContractResult};

use self::iter::Iter as AlarmsIter;

use super::oracle::PriceResult;

mod iter;

const NAMESPACE_ALARMS_BELOW: &str = "alarms_below";
const NAMESPACE_INDEX_BELOW: &str = "index_below";
const NAMESPACE_ALARMS_ABOVE: &str = "alarms_above";
const NAMESPACE_INDEX_ABOVE: &str = "index_above";
const NAMESPACE_IN_DELIVERY: &str = "in_delivery";

pub(super) struct MarketAlarms<'storage, S, PriceG>
where
    S: Deref<Target = dyn Storage + 'storage>,
    PriceG: Group + Clone,
{
    alarms: PriceAlarms<'storage, PriceG, S>,
}

impl<'storage, S, PriceG> MarketAlarms<'storage, S, PriceG>
where
    S: Deref<Target = dyn Storage + 'storage>,
    PriceG: Group + Clone,
{
    pub fn new(storage: S) -> Self {
        Self {
            alarms: PriceAlarms::new(
                storage,
                NAMESPACE_ALARMS_BELOW,
                NAMESPACE_INDEX_BELOW,
                NAMESPACE_ALARMS_ABOVE,
                NAMESPACE_INDEX_ABOVE,
                NAMESPACE_IN_DELIVERY,
            ),
        }
    }

    pub fn notify_alarms_iter<I, BaseC, BaseG>(
        &self,
        prices: I,
    ) -> ContractResult<AlarmsIter<'storage, '_, S, I, PriceG, BaseC, BaseG>>
    where
        I: Iterator<Item = PriceResult<PriceG, BaseC, BaseG>>,
        BaseC: CurrencyDef,
        BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
        BaseG: Group,
    {
        AlarmsIter::new(&self.alarms, prices)
    }

    pub fn try_query_alarms<I, BaseC, BaseG>(&self, prices: I) -> Result<bool, ContractError>
    where
        I: Iterator<Item = PriceResult<PriceG, BaseC, BaseG>>,
        BaseC: CurrencyDef,
        BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
        BaseG: Group,
    {
        Ok(AlarmsIter::new(&self.alarms, prices)?
            .next()
            .transpose()?
            .is_some())
    }

    pub fn ensure_no_in_delivery(&self) -> ContractResult<&Self> {
        self.alarms
            .ensure_no_in_delivery()
            .map(|()| self)
            .map_err(Into::into)
    }
}

impl<'storage, S, PriceG> MarketAlarms<'storage, S, PriceG>
where
    S: Deref<Target = dyn Storage + 'storage> + DerefMut,
    PriceG: Group,
{
    pub fn try_add_price_alarm<BaseC, BaseG>(
        &mut self,
        receiver: Addr,
        alarm: AlarmDTO<PriceG, BaseC, BaseG>,
    ) -> Result<(), ContractError>
    where
        BaseC: CurrencyDef,
        BaseC::Group: MemberOf<BaseG> + MemberOf<PriceG::TopG>,
        BaseG: Group,
    {
        let (below, above_or_equal) = alarm.into();

        with_price::execute::<_, BaseC, BaseG, _>(
            &below,
            AddAlarmsCmd {
                receiver,
                above_or_equal,
                price_alarms: &mut self.alarms,
            },
        )
    }

    pub fn out_for_delivery(&mut self, subscriber: Addr) -> ContractResult<()> {
        self.alarms.out_for_delivery(subscriber).map_err(Into::into)
    }

    pub fn last_delivered(&mut self) -> ContractResult<()> {
        self.alarms.last_delivered().map_err(Into::into)
    }

    pub fn last_failed(&mut self) -> ContractResult<()> {
        self.alarms.last_failed().map_err(Into::into)
    }

    #[cfg(test)]
    fn remove(&mut self, receiver: Addr) -> Result<(), ContractError> {
        self.alarms.remove_all(receiver).map_err(Into::into)
    }
}

struct AddAlarmsCmd<'storage, 'alarms, S, G, BaseC, BaseG>
where
    S: Deref<Target = dyn Storage + 'storage> + DerefMut,
    G: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<G::TopG>,
    BaseG: Group,
{
    receiver: Addr,
    above_or_equal: Option<BasePrice<G, BaseC, BaseG>>,
    price_alarms: &'alarms mut PriceAlarms<'storage, G, S>,
}

impl<'storage, 'alarms, S, G, BaseC, BaseG> WithPrice<BaseC>
    for AddAlarmsCmd<'storage, 'alarms, S, G, BaseC, BaseG>
where
    S: Deref<Target = dyn Storage + 'storage> + DerefMut,
    G: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<BaseG> + MemberOf<G::TopG>,
    BaseG: Group,
{
    type PriceG = G;
    type Output = ();
    type Error = ContractError;

    fn exec<C>(self, below: Price<C, BaseC>) -> Result<Self::Output, Self::Error>
    where
        C: CurrencyDef,
        C::Group: MemberOf<Self::PriceG>,
    {
        self.above_or_equal
            .map(|base_price| Price::try_from(&base_price))
            .transpose()
            .map_err(Into::into)
            .and_then(|above_or_equal: Option<Price<C, BaseC>>| {
                self.price_alarms
                    .add_alarm(self.receiver, below, above_or_equal)
                    .map_err(Into::into)
            })
    }
}

#[cfg(test)]
mod test {
    use currencies::{
        testing::{PaymentC5, PaymentC6, PaymentC7},
        Lpn as BaseCurrency, Lpns as BaseCurrencies, PaymentGroup as PriceCurrencies,
    };
    use sdk::cosmwasm_std::testing::MockStorage;

    use crate::tests;

    use super::*;

    fn alarm_dto<C>(
        below: (u128, u128),
        above: Option<(u128, u128)>,
    ) -> AlarmDTO<PriceCurrencies, BaseCurrency, BaseCurrencies>
    where
        C: CurrencyDef,
        C::Group: MemberOf<PriceCurrencies>,
    {
        AlarmDTO::new(
            tests::base_price::<C>(below.0, below.1),
            above.map(|above| tests::base_price::<C>(above.0, above.1)),
        )
    }

    fn add_alarms<'a>(
        mut storage: &mut dyn Storage,
        mut alarms: impl Iterator<
            Item = (
                &'a str,
                AlarmDTO<PriceCurrencies, BaseCurrency, BaseCurrencies>,
            ),
        >,
    ) -> Result<(), ContractError> {
        alarms.try_for_each(|(receiver, alarm)| -> Result<(), ContractError> {
            MarketAlarms::new(storage.deref_mut())
                .try_add_price_alarm::<BaseCurrency, BaseCurrencies>(
                    Addr::unchecked(receiver),
                    alarm,
                )
        })
    }

    pub fn test_case(storage: &mut dyn Storage) {
        add_alarms(
            storage,
            [
                ("recv2", alarm_dto::<PaymentC6>((1, 20), Some((1, 50)))),
                ("recv1", alarm_dto::<PaymentC6>((1, 10), Some((1, 60)))),
                ("recv3", alarm_dto::<PaymentC5>((1, 20), Some((1, 60)))),
                ("recv4", alarm_dto::<PaymentC5>((1, 30), Some((1, 70)))),
                ("recv5", alarm_dto::<PaymentC7>((1, 30), Some((1, 70)))),
            ]
            .into_iter(),
        )
        .unwrap();
    }

    #[test]
    fn add_remove() {
        let mut storage = MockStorage::new();
        let mut alarms = MarketAlarms::new(&mut storage as &mut dyn Storage);

        let receiver1 = Addr::unchecked("receiver1");
        let receiver2 = Addr::unchecked("receiver2");

        alarms
            .try_add_price_alarm::<BaseCurrency, BaseCurrencies>(
                receiver1,
                alarm_dto::<PaymentC5>((1, 20), None),
            )
            .unwrap();

        alarms
            .try_add_price_alarm(
                receiver2.clone(),
                alarm_dto::<PaymentC6>((1, 20), Some((1, 30))),
            )
            .unwrap();

        assert!(!alarms
            .try_query_alarms(
                [
                    tests::base_price::<PaymentC5>(1, 20),
                    tests::base_price::<PaymentC6>(1, 20)
                ]
                .into_iter()
                .map(Ok),
            )
            .unwrap());

        assert!(alarms
            .try_query_alarms([tests::base_price::<PaymentC6>(1, 35)].into_iter().map(Ok),)
            .unwrap());

        alarms.remove(receiver2).unwrap();

        assert!(!alarms
            .try_query_alarms([tests::base_price::<PaymentC6>(1, 10)].into_iter().map(Ok))
            .unwrap());
    }

    #[test]
    fn alarms_no_pices() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::<_, PriceCurrencies>::new(&storage as &dyn Storage);

        let mut sent = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>([].into_iter().map(Ok))
            .unwrap();

        assert!(sent.next().is_none());
    }

    #[test]
    fn alarms_below_none() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let mut sent = alarms
            .notify_alarms_iter([tests::base_price::<PaymentC6>(1, 25)].into_iter().map(Ok))
            .unwrap();

        assert!(sent.next().is_none());
    }

    #[test]
    fn alarms_below_mid() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let sent: Vec<_> = MarketAlarms::new(&storage as &dyn Storage)
            .notify_alarms_iter([tests::base_price::<PaymentC6>(1, 15)].into_iter().map(Ok))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(sent, [Addr::unchecked("recv2")]);
    }

    #[test]
    fn alarms_below_all() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let sent: Vec<_> = MarketAlarms::new(&storage as &dyn Storage)
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [tests::base_price::<PaymentC6>(1, 5)].into_iter().map(Ok),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(sent, [Addr::unchecked("recv2"), Addr::unchecked("recv1")]);
    }

    #[test]
    fn alarms_above_none() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let mut sent = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [tests::base_price::<PaymentC6>(1, 25)].into_iter().map(Ok),
            )
            .unwrap();

        assert!(sent.next().is_none());
    }

    #[test]
    fn alarms_above_mid() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let sent: Vec<_> = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [tests::base_price::<PaymentC6>(1, 55)].into_iter().map(Ok),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(sent, [Addr::unchecked("recv2")]);
    }

    #[test]
    fn alarms_above_all() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let sent: Vec<_> = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [tests::base_price::<PaymentC6>(1, 65)].into_iter().map(Ok),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(sent, [Addr::unchecked("recv1"), Addr::unchecked("recv2")]);
    }

    #[test]
    fn alarms_mixed() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let sent: Vec<_> = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [
                    tests::base_price::<PaymentC6>(1, 65),
                    tests::base_price::<PaymentC5>(1, 25),
                ]
                .into_iter()
                .map(Ok),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            sent,
            [
                Addr::unchecked("recv1"),
                Addr::unchecked("recv2"),
                Addr::unchecked("recv4")
            ]
        );
    }

    #[test]
    fn alarms_middle_none() {
        let mut storage = MockStorage::new();

        test_case(&mut storage);

        let alarms = MarketAlarms::new(&storage as &dyn Storage);

        let sent: Vec<_> = alarms
            .notify_alarms_iter::<_, BaseCurrency, BaseCurrencies>(
                [
                    tests::base_price::<PaymentC6>(1, 55),
                    tests::base_price::<PaymentC6>(1, 35),
                    tests::base_price::<PaymentC5>(1, 32),
                    tests::base_price::<PaymentC7>(1, 29),
                ]
                .into_iter()
                .map(Ok),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(sent, [Addr::unchecked("recv2"), Addr::unchecked("recv5")]);
    }
}
