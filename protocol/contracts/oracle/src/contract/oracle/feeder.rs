use std::collections::HashSet;

use currency::Group;
use serde::{Deserialize, Serialize};

use marketprice::feeders::PriceFeeders;
use sdk::cosmwasm_std::{Addr, DepsMut, Storage};

use crate::{api::Config, error::Error, result::Result};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Feeders {
    config: Config,
}

impl Feeders {
    const FEEDERS: PriceFeeders = PriceFeeders::new("feeders");

    pub(crate) fn get<PriceG>(storage: &dyn Storage) -> Result<HashSet<Addr>, PriceG>
    where
        PriceG: Group,
    {
        Self::FEEDERS
            .get(storage)
            .map_err(Error::<PriceG>::LoadFeeders)
    }

    pub(crate) fn is_feeder<PriceG>(storage: &dyn Storage, address: &Addr) -> Result<bool, PriceG>
    where
        PriceG: Group,
    {
        Self::FEEDERS
            .is_registered(storage, address)
            .map_err(Error::<PriceG>::LoadFeeders)
    }

    pub(crate) fn try_register<PriceG>(deps: DepsMut<'_>, feeder_txt: String) -> Result<(), PriceG>
    where
        PriceG: Group,
    {
        deps.api
            .addr_validate(&feeder_txt)
            .map_err(Error::<PriceG>::RegisterFeederAddressValidation)
            .and_then(|feeder| Self::FEEDERS.register(deps, feeder).map_err(Into::into))
    }

    pub(crate) fn try_remove<PriceG>(deps: DepsMut<'_>, address: String) -> Result<(), PriceG>
    where
        PriceG: Group,
    {
        deps.api
            .addr_validate(&address)
            .map_err(Error::<PriceG>::UnregisterFeederAddressValidation)
            .and_then(|f_address| {
                Self::is_feeder(deps.storage, &f_address).and_then(|is_feeder| {
                    if is_feeder {
                        Self::FEEDERS.remove(deps, &f_address).map_err(Into::into)
                    } else {
                        Err(Error::<PriceG>::UnknownFeeder {})
                    }
                })
            })
    }

    pub(crate) fn total_registered<PriceG>(storage: &dyn Storage) -> Result<usize, PriceG>
    where
        PriceG: Group,
    {
        Self::get(storage).map(|ref c| c.len())
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod tests {
    use std::collections::HashSet;

    use currencies::PaymentGroup as PriceCurrencies;
    use sdk::{
        cosmwasm_ext::Response as CwResponse,
        cosmwasm_std::{from_json, testing::mock_env, Addr, DepsMut},
        testing,
    };

    use crate::{
        api::{QueryMsg, SudoMsg},
        contract::{query, sudo},
        result::Result,
        tests::{dummy_default_instantiate_msg, setup_test},
    };

    #[test]
    fn register_feeder() {
        let (mut deps, _info) = setup_test(dummy_default_instantiate_msg()).unwrap();

        let feeder0 = testing::user("addr0000");
        let feeder1 = testing::user("addr0001");

        // register new feeder address
        register(deps.as_mut(), &feeder0).unwrap();

        // check if the new address is added to FEEDERS Item
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Feeders {}).unwrap();
        let resp: HashSet<Addr> = from_json(res).unwrap();
        assert_eq!(2, resp.len());
        assert!(resp.contains(&feeder0));

        // should not add the same address twice
        assert!(register(deps.as_mut(), &feeder0).is_err());

        // register new feeder address
        register(deps.as_mut(), &feeder1).unwrap();
        // check if the new address is added to FEEDERS Item
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Feeders {}).unwrap();
        let resp: HashSet<Addr> = from_json(res).unwrap();
        assert_eq!(3, resp.len());
        assert!(resp.contains(&feeder0));
        assert!(resp.contains(&feeder1));
    }

    #[test]
    fn remove_feeder() {
        let (mut deps, _info) = setup_test(dummy_default_instantiate_msg()).unwrap();

        let feeder0 = testing::user("addr0000");
        let feeder1 = testing::user("addr0001");
        let feeder2 = testing::user("addr0002");
        let feeder3 = testing::user("addr0003");

        register(deps.as_mut(), &feeder0).unwrap();
        register(deps.as_mut(), &feeder1).unwrap();
        register(deps.as_mut(), &feeder2).unwrap();
        register(deps.as_mut(), &feeder3).unwrap();

        // check if the new address is added to FEEDERS Item
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Feeders {}).unwrap();
        let resp: HashSet<Addr> = from_json(res).unwrap();
        assert_eq!(5, resp.len());
        assert!(resp.contains(&feeder0));
        assert!(resp.contains(&feeder1));

        remove(deps.as_mut(), &feeder0);
        remove(deps.as_mut(), &feeder1);
        let res = query(deps.as_ref(), mock_env(), QueryMsg::Feeders {}).unwrap();
        let resp: HashSet<Addr> = from_json(res).unwrap();
        assert_eq!(3, resp.len());
        assert!(!resp.contains(&feeder0));
        assert!(!resp.contains(&feeder1));
    }

    fn register(deps: DepsMut<'_>, feeder: &Addr) -> Result<CwResponse, PriceCurrencies> {
        sudo(
            deps,
            mock_env(),
            SudoMsg::RegisterFeeder {
                feeder_address: feeder.to_string(),
            },
        )
    }

    fn remove(deps: DepsMut<'_>, feeder: &Addr) {
        let CwResponse {
            messages,
            attributes,
            events,
            data,
            ..
        }: CwResponse = sudo(
            deps,
            mock_env(),
            SudoMsg::RemoveFeeder {
                feeder_address: feeder.to_string(),
            },
        )
        .expect("Feeder should be removed");

        assert_eq!(messages.len(), 0);
        assert_eq!(attributes.len(), 0);
        assert_eq!(events.len(), 0);
        assert_eq!(data, None);
    }
}
