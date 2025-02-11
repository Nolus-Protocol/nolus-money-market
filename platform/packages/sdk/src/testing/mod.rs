use cosmwasm_std::{
    testing::{mock_dependencies, MockApi, MockQuerier, MockStorage},
    Addr, Checksum, CodeInfoResponse, ContractInfoResponse, ContractResult, Empty, GovMsg, IbcMsg,
    IbcQuery, OwnedDeps, SystemError, SystemResult, WasmQuery,
};
use cw_multi_test::{
    AddressGenerator, BankKeeper, BasicAppBuilder as BasicCwAppBuilder, DistributionKeeper,
    FailingModule, StakeKeeper, StargateFailing, WasmKeeper,
};
pub use cw_multi_test::{ContractWrapper as CwContractWrapper, Executor as CwExecutor};

use crate::cosmwasm_ext::InterChainMsg;

use self::custom_msg::Module as CustomMsgModule;

pub mod manage_state;

pub type CwApp<Exec = InterChainMsg, Query = Empty> =
    cw_multi_test::App<BankKeeper, MockApi, MockStorage, CustomMsgModule, WasmKeeper<Exec, Query>>;

pub type CwAppBuilder<Exec = InterChainMsg, Query = Empty> = cw_multi_test::AppBuilder<
    BankKeeper,
    MockApi,
    MockStorage,
    CustomMsgModule,
    WasmKeeper<Exec, Query>,
    StakeKeeper,
    DistributionKeeper,
    FailingModule<IbcMsg, IbcQuery, Empty>,
    FailingModule<GovMsg, Empty, Empty>,
    StargateFailing,
>;

pub type CwContract = dyn cw_multi_test::Contract<InterChainMsg>;

pub type InterChainMsgSender = std::sync::mpsc::Sender<InterChainMsg>;
pub type InterChainMsgReceiver = std::sync::mpsc::Receiver<InterChainMsg>;

pub fn new_inter_chain_msg_queue() -> (InterChainMsgSender, InterChainMsgReceiver) {
    let (sender, receiver): (InterChainMsgSender, InterChainMsgReceiver) =
        std::sync::mpsc::channel();

    (sender, receiver)
}

pub fn mock_deps_with_contracts<const N: usize>(
    contracts: [Addr; N],
) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    customized_mock_deps_with_contracts(mock_dependencies(), contracts)
}

pub fn customized_mock_deps_with_contracts<const N: usize>(
    mut deps: OwnedDeps<MockStorage, MockApi, MockQuerier>,
    contracts: [Addr; N],
) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    deps.querier.update_wasm(move |query| match query {
        WasmQuery::ContractInfo { contract_addr }
            if contracts.contains(&Addr::unchecked(contract_addr)) =>
        {
            SystemResult::Ok(ContractResult::Ok(
                cosmwasm_std::to_json_binary(&ContractInfoResponse::new(
                    2,
                    user("input"),
                    None,
                    false,
                    None,
                ))
                .expect("serialization succeed"),
            ))
        }
        WasmQuery::Smart { contract_addr, .. }
        | WasmQuery::Raw { contract_addr, .. }
        | WasmQuery::ContractInfo { contract_addr, .. } => {
            SystemResult::Err(SystemError::NoSuchContract {
                addr: contract_addr.clone(),
            })
        }
        WasmQuery::CodeInfo { code_id } => SystemResult::Ok(ContractResult::Ok(
            cosmwasm_std::to_json_binary(&CodeInfoResponse::new(
                *code_id,
                user(""),
                Checksum::generate(&[0x1f, 0x4e, 0x20, 0x9a]),
            ))
            .expect("serialization succeed"),
        )),
        _ => unimplemented!(),
    });

    deps
}

pub fn new_app(message_sender: InterChainMsgSender) -> CwAppBuilder {
    BasicCwAppBuilder::<InterChainMsg, Empty>::new_custom()
        .with_custom(CustomMsgModule::new(message_sender))
        .with_wasm(WasmKeeper::new().with_address_generator(TestAddressGenerator))
}

pub fn user(addr: &str) -> Addr {
    MockApi::default().addr_make(addr)
}

pub fn contract(code_id: u64, instance_id: u64) -> Addr {
    user(&format!("contract_{}_{}", code_id, instance_id))
}

struct TestAddressGenerator;
impl AddressGenerator for TestAddressGenerator {
    fn contract_address(
        &self,
        _api: &dyn cosmwasm_std::Api,
        _storage: &mut dyn cosmwasm_std::Storage,
        code_id: u64,
        instance_id: u64,
    ) -> anyhow::Result<Addr> {
        Ok(contract(code_id, instance_id))
    }
}

mod custom_msg {
    use anyhow::{bail, Result as AnyResult};
    use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CustomQuery, Empty, Querier, Storage};
    use cw_multi_test::{AppResponse, CosmosRouter, Module as ModuleTrait};
    use serde::de::DeserializeOwned;

    use crate::cosmwasm_ext::InterChainMsg;

    use super::InterChainMsgSender;

    pub struct Module {
        message_sender: InterChainMsgSender,
    }

    impl Module {
        pub fn new(message_sender: InterChainMsgSender) -> Self {
            Self { message_sender }
        }
    }

    impl ModuleTrait for Module {
        type ExecT = InterChainMsg;

        type QueryT = Empty;

        type SudoT = Empty;

        fn execute<ExecC, QueryC>(
            &self,
            _api: &dyn Api,
            _storage: &mut dyn Storage,
            _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
            _block: &BlockInfo,
            _sender: Addr,
            msg: Self::ExecT,
        ) -> AnyResult<AppResponse>
        where
            ExecC: std::fmt::Debug + Clone + PartialEq + DeserializeOwned + 'static,
            QueryC: CustomQuery + DeserializeOwned + 'static,
        {
            self.message_sender
                .send(msg)
                .map(|()| AppResponse::default())
                .map_err(Into::into)
        }

        fn sudo<ExecC, QueryC>(
            &self,
            _api: &dyn Api,
            _storage: &mut dyn Storage,
            _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
            _block: &BlockInfo,
            msg: Self::SudoT,
        ) -> AnyResult<AppResponse>
        where
            ExecC: std::fmt::Debug + Clone + PartialEq + DeserializeOwned + 'static,
            QueryC: CustomQuery + DeserializeOwned + 'static,
        {
            bail!("Unexpected sudo msg {:?}", msg)
        }

        fn query(
            &self,
            _api: &dyn Api,
            _storage: &dyn Storage,
            _querier: &dyn Querier,
            _block: &BlockInfo,
            request: Self::QueryT,
        ) -> AnyResult<Binary> {
            bail!("Unexpected custom query {:?}", request)
        }
    }
}
