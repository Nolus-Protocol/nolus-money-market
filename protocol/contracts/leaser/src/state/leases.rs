use std::collections::{hash_set::IntoIter, HashSet};

use sdk::{
    cosmwasm_std::{Addr, Order, StdResult, Storage},
    cw_storage_plus::{Bound, Item, Map},
};

use crate::{
    migrate::{Customer, MaybeCustomer},
    result::ContractResult,
};

pub(crate) struct Leases {}

impl Leases {
    const PENDING_CUSTOMER: Item<Addr> = Item::new("pending_customer");
    const CUSTOMER_LEASES: Map<Addr, HashSet<Addr>> = Map::new("loans");

    pub fn cache_open_req(storage: &mut dyn Storage, customer: &Addr) -> ContractResult<()> {
        Self::PENDING_CUSTOMER
            .save(storage, customer)
            .map_err(Into::into)
    }

    /// Return true if the lease has been stored or false if there has already been the same lease
    pub fn save(storage: &mut dyn Storage, lease: Addr) -> ContractResult<bool> {
        let mut stored = false;
        let update_fn = |may_leases: Option<HashSet<Addr>>| -> StdResult<HashSet<Addr>> {
            let mut leases = may_leases.unwrap_or_default();

            stored = leases.insert(lease);

            Ok(leases)
        };

        Self::PENDING_CUSTOMER
            .load(storage)
            .inspect(|_| Self::PENDING_CUSTOMER.remove(storage))
            .and_then(|customer| Self::CUSTOMER_LEASES.update(storage, customer, update_fn))
            .map(|_| stored)
            .map_err(Into::into)
    }

    pub fn load_by_customer(
        storage: &dyn Storage,
        customer: Addr,
    ) -> ContractResult<HashSet<Addr>> {
        Self::CUSTOMER_LEASES
            .may_load(storage, customer)
            .map(Option::unwrap_or_default)
            .map_err(Into::into)
    }

    /// Return whether the lease was present before the removal
    pub fn remove(storage: &mut dyn Storage, customer: Addr, lease: &Addr) -> ContractResult<bool> {
        let mut removed = false;
        let update_fn = |may_leases: Option<HashSet<Addr>>| -> StdResult<HashSet<Addr>> {
            let mut leases = may_leases.unwrap_or_default();
            removed = leases.remove(lease);
            Ok(leases)
        };

        Self::CUSTOMER_LEASES
            .update(storage, customer, update_fn)
            .map(|_| removed)
            .map_err(Into::into)
    }

    pub fn iter(
        storage: &dyn Storage,
        next_customer: Option<Addr>,
    ) -> impl Iterator<Item = MaybeCustomer<IntoIter<Addr>>> + '_ {
        let start_bound = next_customer.map(Bound::<Addr>::inclusive);
        Self::CUSTOMER_LEASES
            .prefix(())
            .range(storage, start_bound, None, Order::Ascending)
            .map(|record| {
                record
                    .map(|(customer, leases)| Customer::from(customer, leases.into_iter()))
                    .map_err(Into::into)
            })
    }
}

#[cfg(all(feature = "internal.test.testing", test))]
mod test {
    use sdk::cosmwasm_std::{testing::MockStorage, Addr, Storage};

    use crate::{state::leases::Leases, ContractError};

    #[test]
    fn test_save_customer_not_cached() {
        let mut storage = MockStorage::default();
        assert!(matches!(
            Leases::save(&mut storage, test_lease(),),
            Err(ContractError::Std { .. })
        ));
        assert_lease_not_exist(&storage);
    }

    #[test]
    fn test_save_first_lease() {
        let mut storage = MockStorage::default();
        assert_lease_not_exist(&storage);
        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();

        assert_eq!(Ok(true), Leases::save(&mut storage, test_lease()));
        assert_lease_exist(&storage);
    }

    #[test]
    fn test_save_same_lease() {
        let mut storage = MockStorage::default();
        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();
        assert_eq!(Ok(true), Leases::save(&mut storage, test_lease()));
        assert_lease_exist(&storage);

        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();
        assert_eq!(Ok(false), Leases::save(&mut storage, test_lease()));
        assert_lease_exist(&storage);
    }

    #[test]
    fn test_save_another_lease() {
        let mut storage = MockStorage::default();
        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();
        assert_eq!(Ok(true), Leases::save(&mut storage, test_lease()));
        assert_lease_exist(&storage);

        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();
        assert_eq!(Ok(true), Leases::save(&mut storage, test_another_lease()));
        assert_lease_exist(&storage);
        assert!(lease_exist(&storage, &test_another_lease()));
    }

    #[test]
    fn test_remove_not_exist() {
        let mut storage = MockStorage::default();
        assert_lease_not_exist(&storage);
        assert_eq!(
            Ok(false),
            Leases::remove(
                &mut storage,
                Addr::unchecked("customer"),
                &Addr::unchecked("lease1"),
            )
        );
    }

    #[test]
    fn test_remove_exist() {
        let mut storage = MockStorage::default();

        Leases::cache_open_req(&mut storage, &test_customer()).unwrap();
        Leases::save(&mut storage, test_lease()).unwrap();
        assert_lease_exist(&storage);

        assert_eq!(
            Ok(true),
            Leases::remove(&mut storage, test_customer(), &test_lease(),)
        );
        assert_lease_not_exist(&storage);
    }

    fn test_customer() -> Addr {
        const CUSTOMER: &str = "customerX";
        Addr::unchecked(CUSTOMER)
    }

    fn test_lease() -> Addr {
        const LEASE: &str = "lease1";
        Addr::unchecked(LEASE)
    }

    fn test_another_lease() -> Addr {
        const LEASE: &str = "lease2";
        Addr::unchecked(LEASE)
    }

    #[track_caller]
    fn assert_lease_exist(storage: &dyn Storage) {
        assert!(lease_exist(storage, &test_lease()));
    }

    #[track_caller]
    fn assert_lease_not_exist(storage: &dyn Storage) {
        assert!(!lease_exist(storage, &test_lease()));
    }

    fn lease_exist(storage: &dyn Storage, lease: &Addr) -> bool {
        Leases::load_by_customer(storage, test_customer())
            .expect("Customer leases map should exist")
            .contains(lease)
    }
}
