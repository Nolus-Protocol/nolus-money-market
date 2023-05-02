use profit::{
    contract::{execute, instantiate, query, sudo},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg, SudoMsg},
    ContractError,
};
use sdk::{cosmwasm_std::Addr, cw_multi_test::Executor};

use crate::common::{ContractWrapper, MockApp};

use super::ADMIN;

pub struct ProfitWrapper {
    contract_wrapper: Box<ProfitContractWrapper>,
}

impl ProfitWrapper {
    #[track_caller]
    pub fn instantiate(
        self,
        app: &mut MockApp,
        cadence_hours: u16,
        treasury: Addr,
        timealarms: Addr,
    ) -> Addr {
        let code_id = app.store_code(self.contract_wrapper);
        let msg = InstantiateMsg {
            cadence_hours,
            treasury,
            timealarms,
        };

        app.instantiate_contract(code_id, Addr::unchecked(ADMIN), &msg, &[], "profit", None)
            .unwrap()
    }
}

impl Default for ProfitWrapper {
    fn default() -> Self {
        let contract = ContractWrapper::new(execute, instantiate, query).with_sudo(sudo);

        Self {
            contract_wrapper: Box::new(contract),
        }
    }
}

type ProfitContractWrapper = ContractWrapper<
    ExecuteMsg,
    ContractError,
    InstantiateMsg,
    ContractError,
    QueryMsg,
    ContractError,
    SudoMsg,
    ContractError,
>;
