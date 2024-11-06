use std::collections::HashSet;

use admin_contract::msg::{ExecuteMsg, MigrationSpec, ProtocolContracts};
use currencies::LeaseGroup;
use currency::CurrencyDTO;
use finance::{duration::Duration, percent::Percent};
use lease::api::{open::PositionSpecDTO, DownpaymentCoin, MigrateMsg};
use lpp::{msg::ExecuteMsg as LppExecuteMsg, stub::LppRef};
use platform::{
    batch::{Batch, Emit, Emitter},
    contract::Code,
    message::Response as MessageResponse,
};
use reserve::api::ExecuteMsg as ReserveExecuteMsg;
use sdk::cosmwasm_std::{Addr, Deps, Storage};

use crate::{
    cmd::Quote,
    finance::LpnCurrencies,
    migrate,
    msg::{ConfigResponse, MaxLeases, QuoteResponse},
    result::ContractResult,
    state::{config::Config, leases::Leases},
};
use crate::{
    finance::{LpnCurrency, OracleRef},
    msg::ForceClose,
    ContractError,
};

pub struct Leaser<'a> {
    deps: Deps<'a>,
}

impl<'a> Leaser<'a> {
    pub fn new(deps: Deps<'a>) -> Self {
        Self { deps }
    }
    pub fn config(&self) -> ContractResult<ConfigResponse> {
        Config::load(self.deps.storage).map(|config| ConfigResponse { config })
    }

    pub fn customer_leases(&self, customer: Addr) -> ContractResult<HashSet<Addr>> {
        Leases::load_by_customer(self.deps.storage, customer)
    }

    pub fn quote(
        &self,
        downpayment: DownpaymentCoin,
        lease_asset: CurrencyDTO<LeaseGroup>,
        max_ltd: Option<Percent>,
    ) -> ContractResult<QuoteResponse> {
        let config = Config::load(self.deps.storage)?;

        let lpp = LppRef::<LpnCurrency, LpnCurrencies>::try_new(config.lpp, self.deps.querier)?;

        let oracle = OracleRef::try_from_base(config.market_price_oracle, self.deps.querier)?;

        lpp.execute_lender(
            Quote::new(
                self.deps.querier,
                downpayment,
                lease_asset,
                oracle,
                config.lease_position_spec.liability,
                config.lease_interest_rate_margin,
                max_ltd,
            ),
            self.deps.querier,
        )
    }
}

pub(super) fn try_configure(
    storage: &mut dyn Storage,
    lease_interest_rate_margin: Percent,
    lease_position_spec: PositionSpecDTO,
    lease_due_period: Duration,
) -> ContractResult<MessageResponse> {
    Config::update(
        storage,
        lease_interest_rate_margin,
        lease_position_spec,
        lease_due_period,
    )
    .map(|()| MessageResponse::default())
}

pub(super) fn try_migrate_leases<MsgFactory>(
    storage: &mut dyn Storage,
    new_lease: Code,
    max_leases: MaxLeases,
    migrate_msg: MsgFactory,
) -> ContractResult<MessageResponse>
where
    MsgFactory: Fn() -> MigrateMsg,
{
    Config::update_lease_code(storage, new_lease)?;

    let cusomers = Leases::iter(storage, None);
    migrate::migrate_leases(cusomers, new_lease, max_leases, migrate_msg)
        .and_then(|result| result.try_add_msgs(|msgs| update_remote_refs(storage, new_lease, msgs)))
        .map(|result| {
            MessageResponse::messages_with_events(result.msgs, emit_status(result.next_customer))
        })
}

pub(super) fn try_migrate_leases_cont<MsgFactory>(
    storage: &mut dyn Storage,
    next_customer: Addr,
    max_leases: MaxLeases,
    migrate_msg: MsgFactory,
) -> ContractResult<MessageResponse>
where
    MsgFactory: Fn() -> MigrateMsg,
{
    let lease_code = Config::load(storage)?.lease_code;

    let customers = Leases::iter(storage, Some(next_customer));
    migrate::migrate_leases(customers, lease_code, max_leases, migrate_msg).map(|result| {
        MessageResponse::messages_with_events(result.msgs, emit_status(result.next_customer))
    })
}

pub(super) fn try_close_protocol<ProtocolsRegistryLoader, MsgFactory>(
    storage: &mut dyn Storage,
    new_lease_code: Code,
    max_leases: MaxLeases,
    migrate_msg: MsgFactory,
    protocols_registry: ProtocolsRegistryLoader,
    migration_spec: ProtocolContracts<MigrationSpec>,
    force: ForceClose,
) -> ContractResult<MessageResponse>
where
    MsgFactory: Fn() -> MigrateMsg,
    ProtocolsRegistryLoader: FnOnce(&dyn Storage) -> ContractResult<Addr>,
{
    match force {
        ForceClose::KillProtocol => {
            try_migrate_leases(storage, new_lease_code, max_leases, migrate_msg)
        }
        ForceClose::No if has_lease(storage) => Err(ContractError::ProtocolStillInUse()),
        ForceClose::No => Ok(MessageResponse::default()),
    }
    .and_then(|leases_resp| {
        protocols_registry(storage).and_then(|protocols_registry| {
            Batch::default()
                .schedule_execute_wasm_no_reply_no_funds(
                    protocols_registry,
                    &ExecuteMsg::DeregisterProtocol(migration_spec),
                )
                .map_err(ContractError::ProtocolDeregistration)
                .map(|msgs| leases_resp.merge_with(msgs))
        })
    })
}

fn has_lease(storage: &dyn Storage) -> bool {
    Leases::iter(storage, None).next().is_some()
}

fn update_remote_refs(
    storage: &dyn Storage,
    new_lease: Code,
    batch: Batch,
) -> ContractResult<Batch> {
    let cfg = Config::load(storage)?;
    {
        let update_msg = LppExecuteMsg::<LpnCurrencies>::NewLeaseCode {
            lease_code: new_lease,
        };
        batch
            .schedule_execute_wasm_no_reply_no_funds(cfg.lpp, &update_msg)
            .map_err(Into::into)
    }
    .and_then(|batch| {
        let update_msg = ReserveExecuteMsg::NewLeaseCode(new_lease);
        batch
            .schedule_execute_wasm_no_reply_no_funds(cfg.reserve, &update_msg)
            .map_err(Into::into)
    })
}

fn emit_status(next_customer: Option<Addr>) -> Emitter {
    let emitter = Emitter::of_type("migrate-leases");
    if let Some(next) = next_customer {
        emitter.emit("contunuation-key", next)
    } else {
        emitter.emit("status", "done")
    }
}

#[cfg(test)]
mod test {
    use admin_contract::msg::{MigrationSpec, ProtocolContracts};
    use cosmwasm_std::Addr;
    use currencies::Lpn;
    use finance::{coin::Coin, duration::Duration, liability::Liability, percent::Percent};
    use lease::api::{
        open::{ConnectionParams, Ics20Channel, PositionSpecDTO},
        MigrateMsg,
    };
    use platform::{contract::Code, response};
    use sdk::cosmwasm_std::testing::MockStorage;

    use crate::{
        msg::{Config, ForceClose, InstantiateMsg, MaxLeases},
        state::leases::Leases,
        ContractError,
    };

    const MAX_LEASES: MaxLeases = 100_000;
    const LEASE_VOID_CODE: Code = Code::unchecked(12);

    #[test]
    fn close_empty_protocol() {
        let mut store = MockStorage::default();
        Config::new(Code::unchecked(10), dummy_instantiate_msg())
            .store(&mut store)
            .unwrap();
        let resp = super::try_close_protocol(
            &mut store,
            LEASE_VOID_CODE,
            MAX_LEASES,
            migrate_msg,
            |_| Ok(Addr::unchecked("Registry")),
            dummy_spec(),
            ForceClose::No,
        )
        .unwrap();
        let cw_resp = response::response_only_messages(resp);
        let delete_protocol = 1;
        assert_eq!(delete_protocol, cw_resp.messages.len());
    }

    #[test]
    fn close_non_empty_protocol() {
        let mut store = MockStorage::default();
        Config::new(Code::unchecked(10), dummy_instantiate_msg())
            .store(&mut store)
            .unwrap();
        let customer = Addr::unchecked("CustomerA");
        let lease = Addr::unchecked("Lease1");
        Leases::cache_open_req(&mut store, &customer).expect("cache the customer should succeed");
        Leases::save(&mut store, lease).expect("save a new lease should succeed");
        assert_eq!(
            Err(ContractError::ProtocolStillInUse()),
            super::try_close_protocol(
                &mut store,
                LEASE_VOID_CODE,
                MAX_LEASES,
                migrate_msg,
                |_| Ok(Addr::unchecked("Registry")),
                dummy_spec(),
                ForceClose::No
            )
        );

        let resp = super::try_close_protocol(
            &mut store,
            LEASE_VOID_CODE,
            MAX_LEASES,
            migrate_msg,
            |_| Ok(Addr::unchecked("Registry")),
            dummy_spec(),
            ForceClose::KillProtocol,
        )
        .unwrap();
        let cw_resp = response::response_only_messages(resp);
        let update_lpp_update_reserve_migrate_lease_delete_protocol_legacy_leases = 1 + 1 + 1 + 1;
        assert_eq!(
            update_lpp_update_reserve_migrate_lease_delete_protocol_legacy_leases,
            cw_resp.messages.len()
        );
    }

    fn dummy_instantiate_msg() -> InstantiateMsg {
        InstantiateMsg {
            lease_code: 10u16.into(),
            lpp: Addr::unchecked("LPP"),
            profit: Addr::unchecked("Profit"),
            reserve: Addr::unchecked("reserve"),
            time_alarms: Addr::unchecked("time alarms"),
            market_price_oracle: Addr::unchecked("oracle"),
            protocols_registry: Addr::unchecked("protocols"),
            lease_position_spec: PositionSpecDTO {
                liability: Liability::new(
                    Percent::from_percent(10),
                    Percent::from_percent(65),
                    Percent::from_percent(72),
                    Percent::from_percent(74),
                    Percent::from_percent(76),
                    Percent::from_percent(80),
                    Duration::from_hours(12),
                ),
                min_asset: Coin::<Lpn>::from(120_000).into(),
                min_transaction: Coin::<Lpn>::from(12_000).into(),
            },
            lease_interest_rate_margin: Percent::from_percent(3),
            lease_due_period: Duration::from_days(14),
            dex: ConnectionParams {
                connection_id: "conn-12".into(),
                transfer_channel: Ics20Channel {
                    local_endpoint: "chan-1".into(),
                    remote_endpoint: "chan-13".into(),
                },
            },
        }
    }

    fn dummy_spec() -> ProtocolContracts<MigrationSpec> {
        let migration_spec = MigrationSpec {
            code_id: 23u64.into(),
            migrate_msg: "{}".into(),
            post_migrate_execute_msg: None,
        };
        ProtocolContracts {
            leaser: migration_spec.clone(),
            lpp: migration_spec.clone(),
            oracle: migration_spec.clone(),
            profit: migration_spec.clone(),
            reserve: migration_spec,
        }
    }

    fn migrate_msg() -> MigrateMsg {
        MigrateMsg {}
    }
}
