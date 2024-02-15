use std::collections::{hash_set::IntoIter, HashSet};

use sdk::{
    cosmwasm_ext::as_dyn::storage,
    cosmwasm_std::{Addr, StdResult},
    cw_storage_plus::{Bound, Item, Map},
};

use crate::{
    migrate::{Customer, MaybeCustomer},
    result::ContractResult,
};

pub(crate) struct Leases {}

impl Leases {
    const PENDING_CUSTOMER: Item<'static, Addr> = Item::new("pending_customer");
    const CUSTOMER_LEASES: Map<'static, Addr, HashSet<Addr>> = Map::new("loans");

    pub fn cache_open_req<S>(storage: &mut S, customer: &Addr) -> ContractResult<()>
    where
        S: storage::DynMut + ?Sized,
    {
        Self::PENDING_CUSTOMER
            .save(storage.as_dyn_mut(), customer)
            .map_err(Into::into)
    }

    /// Return true if the lease has been stored or false if there has already been the same lease
    pub fn save<S>(storage: &mut S, lease: Addr) -> ContractResult<bool>
    where
        S: storage::DynMut + ?Sized,
    {
        let mut stored = false;

        let update_fn = |may_leases: Option<HashSet<Addr>>| -> StdResult<HashSet<Addr>> {
            let mut leases = may_leases.unwrap_or_default();

            stored = leases.insert(lease);

            Ok(leases)
        };

        let storage = storage.as_dyn_mut();

        Self::PENDING_CUSTOMER
            .load(storage)
            .map(|customer| {
                Self::PENDING_CUSTOMER.remove(storage);
                customer
            })
            .map(|customer| Self::CUSTOMER_LEASES.update(storage, customer, update_fn))
            .map(|_| stored)
            .map_err(Into::into)
    }

    pub fn load_by_customer<S>(storage: &S, customer: Addr) -> ContractResult<HashSet<Addr>>
    where
        S: storage::Dyn + ?Sized,
    {
        Self::CUSTOMER_LEASES
            .may_load(storage.as_dyn(), customer)
            .map(Option::unwrap_or_default)
            .map_err(Into::into)
    }

    /// Return whether the lease was present before the removal
    pub fn remove<S>(storage: &mut S, customer: Addr, lease: &Addr) -> ContractResult<bool>
    where
        S: storage::DynMut + ?Sized,
    {
        let mut removed = false;

        let update_fn = |may_leases: Option<HashSet<Addr>>| -> StdResult<HashSet<Addr>> {
            let mut leases = may_leases.unwrap_or_default();

            removed = leases.remove(lease);

            Ok(leases)
        };

        Self::CUSTOMER_LEASES
            .update(storage.as_dyn_mut(), customer, update_fn)
            .map(|_| removed)
            .map_err(Into::into)
    }

    pub fn iter<S>(
        storage: &S,
        next_customer: Option<Addr>,
    ) -> impl Iterator<Item = MaybeCustomer<IntoIter<Addr>>> + '_
    where
        S: storage::Dyn + ?Sized,
    {
        let start_bound = next_customer.map(Bound::<Addr>::inclusive);
        Self::CUSTOMER_LEASES
            .prefix(())
            .range(
                storage.as_dyn(),
                start_bound,
                None,
                cosmwasm_std::Order::Ascending,
            )
            .map(|record| {
                record
                    .map(|(customer, leases)| Customer::from(customer, leases.into_iter()))
                    .map_err(Into::into)
            })
    }
}

#[cfg(test)]
mod test {
    use sdk::{
        cosmwasm_ext::as_dyn::storage,
        cosmwasm_std::{testing::MockStorage, Addr, Storage},
    };

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
            Leases::remove(&mut storage, test_customer(), &test_lease())
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
    fn assert_lease_exist<S>(storage: &S)
    where
        S: storage::Dyn + ?Sized,
    {
        assert!(lease_exist(storage, &test_lease()));
    }

    #[track_caller]
    fn assert_lease_not_exist<S>(storage: &S)
    where
        S: storage::Dyn + ?Sized,
    {
        assert!(!lease_exist(storage, &test_lease()));
    }

    fn lease_exist<S>(storage: &S, lease: &Addr) -> bool
    where
        S: storage::Dyn + ?Sized,
    {
        Leases::load_by_customer(storage, test_customer())
            .unwrap()
            .contains(lease)
    }
}
