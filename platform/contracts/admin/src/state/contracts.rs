use std::collections::BTreeMap;

use sdk::{
    cosmwasm_std::{Addr, Order, Storage},
    cw_storage_plus::{Item, Map},
};

use crate::{
    contracts::{ContractsGroupedByProtocol, ContractsTemplate, PlatformTemplate, Protocol},
    error::Error,
    result::Result,
};

const PLATFORM: Item<'_, PlatformTemplate<Addr>> = Item::new("platform_contracts");
const PROTOCOL: Map<'_, String, Protocol> = Map::new("protocol_contracts");

pub(crate) fn store(
    storage: &mut dyn Storage,
    contracts: ContractsGroupedByProtocol,
) -> Result<()> {
    PLATFORM
        .save(storage, &contracts.platform)
        .map_err(Into::into)
        .and_then(|()| {
            contracts.protocol.into_iter().try_for_each(
                |(protocol, ref contracts): (String, Protocol)| {
                    PROTOCOL
                        .save(storage, protocol, contracts)
                        .map_err(Into::into)
                },
            )
        })
}

pub(crate) fn add_protocol(
    storage: &mut dyn Storage,
    name: String,
    protocol: &Protocol,
) -> Result<()> {
    if PROTOCOL.has(storage, name.clone()) {
        Err(Error::ProtocolSetAlreadyExists(name))
    } else {
        PROTOCOL.save(storage, name, protocol).map_err(Into::into)
    }
}

pub(crate) fn remove_protocol(storage: &mut dyn Storage, name: String) -> Result<()> {
    if PROTOCOL.has(storage, name.clone()) {
        PROTOCOL.remove(storage, name);

        Ok(())
    } else {
        Err(Error::ProtocolDoesNotExists(name))
    }
}

pub(crate) fn protocols(storage: &dyn Storage) -> Result<Vec<String>> {
    PROTOCOL
        .keys(storage, None, None, Order::Ascending)
        .collect::<std::result::Result<_, _>>()
        .map_err(Into::into)
}

pub(crate) fn load_platform(storage: &dyn Storage) -> Result<PlatformTemplate<Addr>> {
    PLATFORM.load(storage).map_err(Into::into)
}

pub(crate) fn load_protocol(storage: &dyn Storage, name: String) -> Result<Protocol> {
    PROTOCOL.load(storage, name).map_err(Into::into)
}

pub(crate) fn load_all(storage: &dyn Storage) -> Result<ContractsGroupedByProtocol> {
    load_platform(storage).and_then(|platform: PlatformTemplate<Addr>| {
        PROTOCOL
            .range(storage, None, None, Order::Ascending)
            .collect::<::std::result::Result<_, _>>()
            .map(|protocol: BTreeMap<String, Protocol>| ContractsTemplate { platform, protocol })
            .map_err(Into::into)
    })
}
