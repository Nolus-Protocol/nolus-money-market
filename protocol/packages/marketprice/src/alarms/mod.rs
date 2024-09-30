use std::{
    iter,
    ops::{Deref, DerefMut},
};

use prefix::Prefix;
use serde::{Deserialize, Serialize};

use currency::{CurrencyDef, Group, MemberOf, SymbolOwned};
use finance::{
    coin::{Amount, CoinDTO},
    error::Error as FinanceError,
    price::{self, Price},
};
use sdk::{
    cosmwasm_std::{Addr, Order, StdError as CwError, Storage},
    cw_storage_plus::{
        Bound, Deque, Index, IndexList, IndexedMap as CwIndexedMap, IntKey, Key, MultiIndex,
        Prefixer, PrimaryKey,
    },
};

use self::errors::AlarmError;

pub mod errors;
pub mod prefix;

pub type AlarmsCount = u32;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(bound(serialize = "", deserialize = ""))]
struct NormalizedPrice<G>(CoinDTO<G>)
where
    G: Group + Clone;

type BoxedIter<'storage, G> =
    Box<dyn Iterator<Item = Result<(Addr, NormalizedPrice<G>), CwError>> + 'storage>;

pub struct AlarmsIterator<'alarms, G>(iter::Chain<BoxedIter<'alarms, G>, BoxedIter<'alarms, G>>)
where
    G: Group + Clone;

impl<'alarms, G> Iterator for AlarmsIterator<'alarms, G>
where
    G: Group + Clone,
{
    type Item = Result<Addr, AlarmError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|res| {
            res.map(|pair| pair.0)
                .map_err(AlarmError::IteratorLoadFailed)
        })
    }
}

impl<G> NormalizedPrice<G>
where
    G: Group + Clone,
{
    fn new<C, BaseC>(price: &Price<C, BaseC>) -> Option<Self>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        BaseC: CurrencyDef,
    {
        const NORM_SCALE: Amount = 10u128.pow(18);
        price::total(NORM_SCALE.into(), price.inv()).map(|price| NormalizedPrice::<G>(price.into()))
    }
}

impl<'a, G> PrimaryKey<'a> for NormalizedPrice<G>
where
    G: Group + Clone,
{
    type Prefix = SymbolOwned;
    type Suffix = Amount;
    type SubPrefix = ();
    type SuperSuffix = (SymbolOwned, Amount);

    fn key(&self) -> Vec<Key<'_>> {
        vec![
            Key::Ref(self.first_key().as_bytes()),
            Key::Val128(self.0.amount().to_cw_bytes()),
        ]
    }
}

impl<'a, G> Prefixer<'a> for NormalizedPrice<G>
where
    G: Group + Clone,
{
    fn prefix(&self) -> Vec<Key<'_>> {
        self.key()
    }
}

struct AlarmsIndexes<G>(MultiIndex<'static, NormalizedPrice<G>, NormalizedPrice<G>, Addr>)
where
    G: Group + Clone;

impl<G> IndexList<NormalizedPrice<G>> for AlarmsIndexes<G>
where
    G: Group + Clone,
{
    fn get_indexes(&self) -> Box<dyn Iterator<Item = &dyn Index<NormalizedPrice<G>>> + '_> {
        Box::new(vec![&self.0 as &_].into_iter())
    }
}

type IndexedMap<G> = CwIndexedMap<'static, Addr, NormalizedPrice<G>, AlarmsIndexes<G>>;

fn alarms_index<G>(alarms_namespace: &'static str, index_namespace: &'static str) -> IndexedMap<G>
where
    G: Group + Clone,
{
    let indexes = AlarmsIndexes(MultiIndex::new(
        |_, price| price.to_owned(),
        alarms_namespace,
        index_namespace,
    ));

    IndexedMap::new(alarms_namespace, indexes)
}

pub struct PriceAlarms<'storage, G, S>
where
    G: Group + Clone,
    S: Deref<Target = dyn Storage + 'storage>,
{
    storage: S,
    alarms_below: IndexedMap<G>,
    alarms_above_or_equal: IndexedMap<G>,
    in_delivery: Deque<'static, AlarmWithSubscriber<G>>,
}

impl<'storage, G, S> PriceAlarms<'storage, G, S>
where
    G: Group + Clone,
    S: Deref<Target = dyn Storage + 'storage>,
{
    pub fn new(
        storage: S,
        alarms_below_namespace: &'static str,
        index_below_namespace: &'static str,
        alarms_above_namespace: &'static str,
        index_above_namespace: &'static str,
        in_delivery_namespace: &'static str,
    ) -> Self {
        Self {
            storage,
            alarms_below: alarms_index(alarms_below_namespace, index_below_namespace),
            alarms_above_or_equal: alarms_index(alarms_above_namespace, index_above_namespace),
            in_delivery: Deque::new(in_delivery_namespace),
        }
    }

    pub fn alarms<C, BaseC>(&self, price: Price<C, BaseC>) -> Option<AlarmsIterator<'_, G>>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        BaseC: CurrencyDef,
    {
        NormalizedPrice::new(&price).map(|norm_price| {
            AlarmsIterator(
                self.iter_below(&norm_price)
                    .chain(self.iter_above_or_equal(&norm_price)),
            )
        })
    }

    pub fn ensure_no_in_delivery(&self) -> Result<(), AlarmError> {
        match self.in_delivery.is_empty(self.storage.deref()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(AlarmError::NonEmptyAlarmsInDeliveryQueue(String::from(
                "Assertion requested",
            ))),
            Err(error) => Err(AlarmError::InDeliveryIsEmptyFailed(error)),
        }
    }

    fn iter_below(&self, price: &NormalizedPrice<G>) -> BoxedIter<'_, G> {
        self.alarms_below
            .idx
            .0
            .sub_prefix(price.first_key().into())
            .range(
                self.storage.deref(),
                None,
                Some(Bound::exclusive((price.0.amount(), Addr::unchecked("")))),
                Order::Ascending,
            )
    }

    fn iter_above_or_equal(&self, price: &NormalizedPrice<G>) -> BoxedIter<'_, G> {
        self.alarms_above_or_equal
            .idx
            .0
            .sub_prefix(price.first_key().into())
            .range(
                self.storage.deref(),
                Some(Bound::exclusive((price.0.amount(), Addr::unchecked("")))),
                None,
                Order::Ascending,
            )
    }
}

impl<'storage, G, S> PriceAlarms<'storage, G, S>
where
    G: Group + Clone,
    S: Deref<Target = dyn Storage + 'storage> + DerefMut,
{
    pub fn add_alarm<C, BaseC>(
        &mut self,
        subscriber: Addr,
        below: Price<C, BaseC>,
        above_or_equal: Option<Price<C, BaseC>>,
    ) -> Result<(), AlarmError>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        BaseC: CurrencyDef,
    {
        NormalizedPrice::new(&below)
            .ok_or_else(|| {
                AlarmError::CreatingNormalizedPrice(
                    FinanceError::Overflow(
                        format!("Overflow occurred while normalizing the below price: {:?}", below)
                    )
                )
            })
            .and_then(|normalized_below| {
                self.add_alarm_below_internal(subscriber.clone(), &normalized_below)?;

                match above_or_equal {
                    None => self.remove_above_or_equal(subscriber),
                    Some(above_or_equal) => {
                        NormalizedPrice::new(&above_or_equal)
                            .ok_or_else(|| {
                                AlarmError::CreatingNormalizedPrice(
                                    FinanceError::Overflow(
                                        format!("Overflow occurred while normalizing the above_or_equal price: {:?}", above_or_equal)
                                    )
                                )
                            })
                            .and_then(|normalized_above_or_equal| {
                                self.add_alarm_above_or_equal_internal(subscriber, &normalized_above_or_equal)
                            })
                    }
                }
            })
    }

    pub fn remove_above_or_equal(&mut self, subscriber: Addr) -> Result<(), AlarmError> {
        self.alarms_above_or_equal
            .remove(self.storage.deref_mut(), subscriber)
            .map_err(AlarmError::RemoveAboveOrEqual)
    }

    pub fn remove_all(&mut self, subscriber: Addr) -> Result<(), AlarmError> {
        self.alarms_below
            .remove(self.storage.deref_mut(), subscriber.clone())
            .map_err(AlarmError::RemoveBelow)
            .and_then(|()| self.remove_above_or_equal(subscriber))
    }

    pub fn out_for_delivery(&mut self, subscriber: Addr) -> Result<(), AlarmError> {
        let below: NormalizedPrice<G> = self
            .alarms_below
            .load(self.storage.deref(), subscriber.clone())
            .map_err(AlarmError::InDeliveryLoadBelow)?;

        self.alarms_below
            .replace(
                self.storage.deref_mut(),
                subscriber.clone(),
                None,
                Some(&below),
            )
            .map_err(AlarmError::InDeliveryRemoveBelow)?;

        let above: Option<NormalizedPrice<G>> = self
            .alarms_above_or_equal
            .may_load(self.storage.deref(), subscriber.clone())
            .map_err(AlarmError::InDeliveryLoadAboveOrEqual)?;

        if let Some(above) = &above {
            self.alarms_above_or_equal
                .replace(
                    self.storage.deref_mut(),
                    subscriber.clone(),
                    None,
                    Some(above),
                )
                .map_err(AlarmError::InDeliveryRemoveAboveOrEqual)?;
        }

        self.in_delivery
            .push_back(
                self.storage.deref_mut(),
                &AlarmWithSubscriber {
                    subscriber,
                    below,
                    above,
                },
            )
            .map_err(AlarmError::InDeliveryAppend)
    }

    pub fn last_delivered(&mut self) -> Result<(), AlarmError> {
        self.pop_front_in_delivery(
            AlarmError::LastDeliveredRemove,
            "Received success reply status",
        )
        .map(|_: AlarmWithSubscriber<G>| ())
    }

    pub fn last_failed(&mut self) -> Result<(), AlarmError> {
        self.pop_front_in_delivery(
            AlarmError::LastFailedRemove,
            "Received failure reply status",
        )
        .and_then(|alarm: AlarmWithSubscriber<G>| {
            self.add_alarm_below_internal(alarm.subscriber.clone(), &alarm.below)
                .and_then(|()| {
                    if let Some(above) = alarm.above {
                        self.add_alarm_above_or_equal_internal(alarm.subscriber.clone(), &above)
                    } else {
                        Ok(())
                    }
                })
        })
    }

    fn pop_front_in_delivery<PopErrFn>(
        &mut self,
        error_on_pop: PopErrFn,
        error_on_empty: &str,
    ) -> Result<AlarmWithSubscriber<G>, AlarmError>
    where
        PopErrFn: FnOnce(CwError) -> AlarmError,
    {
        self.in_delivery
            .pop_front(self.storage.deref_mut())
            .map_err(error_on_pop)
            .and_then(|maybe_alarm: Option<AlarmWithSubscriber<G>>| {
                maybe_alarm.ok_or_else(|| {
                    AlarmError::EmptyAlarmsInDeliveryQueue(String::from(error_on_empty))
                })
            })
    }

    fn add_alarm_below_internal(
        &mut self,
        subscriber: Addr,
        alarm: &NormalizedPrice<G>,
    ) -> Result<(), AlarmError> {
        Self::add_alarm_internal(
            self.storage.deref_mut(),
            &self.alarms_below,
            subscriber,
            alarm,
            AlarmError::AddAlarmStoreBelow,
        )
    }

    fn add_alarm_above_or_equal_internal(
        &mut self,
        subscriber: Addr,
        alarm: &NormalizedPrice<G>,
    ) -> Result<(), AlarmError> {
        Self::add_alarm_internal(
            self.storage.deref_mut(),
            &self.alarms_above_or_equal,
            subscriber,
            alarm,
            AlarmError::AddAlarmStoreAboveOrEqual,
        )
    }

    fn add_alarm_internal<ErrFn>(
        storage: &mut dyn Storage,
        alarms: &IndexedMap<G>,
        subscriber: Addr,
        alarm: &NormalizedPrice<G>,
        error_map: ErrFn,
    ) -> Result<(), AlarmError>
    where
        ErrFn: FnOnce(CwError) -> AlarmError,
    {
        alarms.save(storage, subscriber, alarm).map_err(error_map)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(bound(serialize = "", deserialize = ""))]
struct AlarmWithSubscriber<G>
where
    G: Group + Clone,
{
    subscriber: Addr,
    below: NormalizedPrice<G>,
    above: Option<NormalizedPrice<G>>,
}

#[cfg(test)]
pub mod tests {
    use currency::test::{SuperGroup, SuperGroupTestC2, SuperGroupTestC3, SuperGroupTestC4};
    use finance::{coin::Coin, price};
    use sdk::cosmwasm_std::{testing::MockStorage, Addr};

    use super::*;

    type BaseCurrency = SuperGroupTestC3;

    #[test]
    fn test_below_exclusive() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let addr1 = Addr::unchecked("addr1");

        let price =
            price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20));
        alarms.add_alarm(addr1, price, None).unwrap();

        assert_eq!(None, alarms.alarms(price).unwrap().next());
    }

    #[test]
    fn test_above_inclusive() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let addr1 = Addr::unchecked("addr1");

        let price =
            price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20));
        alarms
            .add_alarm(
                addr1.clone(),
                price::total_of(Coin::new(1)).is(Coin::new(10)),
                Some(price),
            )
            .unwrap();

        let mut triggered_alarms = alarms.alarms(price).unwrap();
        assert_eq!(Some(Ok(addr1)), triggered_alarms.next());
        assert_eq!(None, triggered_alarms.next());
    }

    #[test]
    fn test_equal_alarms() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let addr1 = Addr::unchecked("addr1");

        let price =
            price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20));
        alarms.add_alarm(addr1.clone(), price, None).unwrap();
        alarms
            .add_alarm(
                addr1.clone(),
                price::total_of(Coin::new(1)).is(Coin::new(10)),
                Some(price),
            )
            .unwrap();

        let mut triggered_alarms = alarms.alarms(price).unwrap();
        assert_eq!(Some(Ok(addr1)), triggered_alarms.next());
        assert_eq!(None, triggered_alarms.next());
    }

    #[test]
    fn test_out_for_delivery_removes_above() {
        type QuoteCurrency = SuperGroupTestC4;
        type PriceBaseQuote = Price<BaseCurrency, QuoteCurrency>;

        const PRICE_BASE: Coin<BaseCurrency> = Coin::new(1);
        const PRICE_QUOTE: Coin<QuoteCurrency> = Coin::new(2);
        const PRICE: fn() -> PriceBaseQuote = || price::total_of(PRICE_BASE).is(PRICE_QUOTE);
        const LOWER_PRICE: fn() -> PriceBaseQuote =
            || price::total_of(PRICE_BASE).is(PRICE_QUOTE - Coin::new(1));

        fn expect_no_alarms<'storage>(
            alarms: &PriceAlarms<'storage, SuperGroup, &mut (dyn Storage + 'storage)>,
        ) {
            // Catch below
            assert_eq!(alarms.alarms(LOWER_PRICE()).unwrap().count(), 0);

            // Catch above or equal
            assert_eq!(alarms.alarms(PRICE()).unwrap().count(), 0);
        }

        /* TEST START */

        let mut storage: MockStorage = MockStorage::new();
        let mut alarms: PriceAlarms<'_, SuperGroup, &mut dyn Storage> = alarms(&mut storage);

        let subscriber: Addr = Addr::unchecked("addr1");

        // Add alarms
        alarms
            .add_alarm(subscriber.clone(), PRICE(), Some(PRICE()))
            .unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        // Queue for delivery
        alarms.out_for_delivery(subscriber).unwrap();

        expect_no_alarms(&alarms);

        assert!(matches!(
            alarms.ensure_no_in_delivery().unwrap_err(),
            AlarmError::NonEmptyAlarmsInDeliveryQueue(_)
        ));

        // Mark as delivered
        alarms.last_delivered().unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        expect_no_alarms(&alarms);
    }

    #[test]
    fn test_add_remove() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let addr1 = Addr::unchecked("addr1");
        let addr2 = Addr::unchecked("addr2");
        let addr3 = Addr::unchecked("addr3");

        alarms
            .add_alarm(
                addr1.clone(),
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20)),
                None,
            )
            .unwrap();

        alarms
            .add_alarm(
                addr2.clone(),
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(5)),
                Some(
                    price::total_of(Coin::<SuperGroupTestC4>::new(1))
                        .is(Coin::<BaseCurrency>::new(10)),
                ),
            )
            .unwrap();
        alarms
            .add_alarm(
                addr3.clone(),
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20)),
                None,
            )
            .unwrap();

        alarms.remove_all(addr1).unwrap();
        alarms.remove_all(addr2).unwrap();

        let resp: Vec<_> = alarms
            .alarms(
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(15)),
            )
            .unwrap()
            .collect();

        assert_eq!(resp, vec![Ok(addr3)]);
    }

    #[test]
    fn test_alarms_selection() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let addr1 = Addr::unchecked("addr1");
        let addr2 = Addr::unchecked("addr2");
        let addr3 = Addr::unchecked("addr3");
        let addr4 = Addr::unchecked("addr4");
        let addr5 = Addr::unchecked("addr5");

        alarms
            .add_alarm(
                addr1,
                price::total_of(Coin::<SuperGroupTestC2>::new(1)).is(Coin::<BaseCurrency>::new(10)),
                None,
            )
            .unwrap();
        alarms
            .add_alarm(
                addr2.clone(),
                price::total_of(Coin::<SuperGroupTestC2>::new(1)).is(Coin::<BaseCurrency>::new(20)),
                None,
            )
            .unwrap();
        alarms
            .add_alarm(
                addr3.clone(),
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(30)),
                None,
            )
            .unwrap();
        alarms
            .add_alarm(
                addr4.clone(),
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20)),
                Some(
                    price::total_of(Coin::<SuperGroupTestC4>::new(1))
                        .is(Coin::<BaseCurrency>::new(25)),
                ),
            )
            .unwrap();
        alarms
            .add_alarm(
                addr5,
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(20)),
                Some(
                    price::total_of(Coin::<SuperGroupTestC4>::new(1))
                        .is(Coin::<BaseCurrency>::new(35)),
                ),
            )
            .unwrap();

        let resp: Vec<_> = alarms
            .alarms(
                price::total_of(Coin::<SuperGroupTestC2>::new(1)).is(Coin::<BaseCurrency>::new(15)),
            )
            .unwrap()
            .collect();

        assert_eq!(resp, vec![Ok(addr2)]);

        let resp: Vec<_> = alarms
            .alarms(
                price::total_of(Coin::<SuperGroupTestC4>::new(1)).is(Coin::<BaseCurrency>::new(26)),
            )
            .unwrap()
            .collect();

        assert_eq!(resp, vec![Ok(addr3), Ok(addr4)]);
    }

    #[test]
    fn test_delivered() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let subscriber1 = Addr::unchecked("subscriber1");
        let subscriber2 = Addr::unchecked("subscriber2");

        alarms.ensure_no_in_delivery().unwrap();

        alarms
            .add_alarm(
                subscriber1.clone(),
                Price::<SuperGroupTestC4, BaseCurrency>::identity(),
                None,
            )
            .unwrap();
        alarms
            .add_alarm(
                subscriber1.clone(),
                price::total_of::<SuperGroupTestC4>(1.into()).is::<BaseCurrency>(2.into()),
                None,
            )
            .unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        alarms
            .add_alarm(
                subscriber2.clone(),
                Price::<SuperGroupTestC4, BaseCurrency>::identity(),
                None,
            )
            .unwrap();
        alarms
            .add_alarm(
                subscriber2.clone(),
                price::total_of::<SuperGroupTestC4>(1.into()).is::<BaseCurrency>(2.into()),
                None,
            )
            .unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        alarms.out_for_delivery(subscriber1).unwrap();
        alarms.out_for_delivery(subscriber2).unwrap();

        assert!(matches!(
            alarms.ensure_no_in_delivery().unwrap_err(),
            AlarmError::NonEmptyAlarmsInDeliveryQueue(_)
        ));

        alarms.last_delivered().unwrap();

        assert!(matches!(
            alarms.ensure_no_in_delivery().unwrap_err(),
            AlarmError::NonEmptyAlarmsInDeliveryQueue(_)
        ));

        alarms.last_delivered().unwrap();

        alarms.ensure_no_in_delivery().unwrap();
    }

    #[test]
    fn test_failed() {
        let mut storage = MockStorage::new();
        let mut alarms = alarms(&mut storage);

        let subscriber1 = Addr::unchecked("subscriber1");
        let subscriber2 = Addr::unchecked("subscriber2");

        let subscriber2_below_price = Price::<SuperGroupTestC4, BaseCurrency>::identity();
        let subscriber2_above_or_equal_price = Price::<SuperGroupTestC4, BaseCurrency>::identity();

        alarms.ensure_no_in_delivery().unwrap();

        alarms
            .add_alarm(
                subscriber1.clone(),
                Price::<SuperGroupTestC4, BaseCurrency>::identity(),
                Some(price::total_of::<SuperGroupTestC4>(1.into()).is::<BaseCurrency>(2.into())),
            )
            .unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        alarms
            .add_alarm(
                subscriber2.clone(),
                subscriber2_below_price,
                Some(subscriber2_above_or_equal_price),
            )
            .unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        alarms.out_for_delivery(subscriber1).unwrap();
        alarms.out_for_delivery(subscriber2.clone()).unwrap();

        assert!(matches!(
            alarms.ensure_no_in_delivery().unwrap_err(),
            AlarmError::NonEmptyAlarmsInDeliveryQueue(_)
        ));

        alarms.last_delivered().unwrap();

        assert!(matches!(
            alarms.ensure_no_in_delivery().unwrap_err(),
            AlarmError::NonEmptyAlarmsInDeliveryQueue(_)
        ));

        alarms.last_failed().unwrap();

        alarms.ensure_no_in_delivery().unwrap();

        assert_eq!(
            alarms
                .alarms(subscriber2_below_price)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![subscriber2.clone()]
        );
        assert_eq!(
            alarms
                .alarms(subscriber2_above_or_equal_price)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![subscriber2]
        );
    }

    fn alarms<'storage, 'storage_ref>(
        storage: &'storage_ref mut (dyn Storage + 'storage),
    ) -> PriceAlarms<'storage, SuperGroup, &'storage_ref mut (dyn Storage + 'storage)> {
        PriceAlarms::new(
            storage,
            "alarms_below",
            "index_below",
            "alarms_above",
            "index_above",
            "in_delivery",
        )
    }
}
