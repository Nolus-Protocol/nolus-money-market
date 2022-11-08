use lease::api::dex::{ConnectionParams, Ics20Channel};
use serde::{Deserialize, Serialize};

use finance::{
    currency::Currency, duration::Duration, liability::Liability, percent::Percent,
    test::currency::Usdc,
};
use sdk::{
    cosmwasm_std::{
        coins, from_binary,
        testing::{mock_dependencies, mock_env, mock_info},
        to_binary, Addr, CosmosMsg, DepsMut, MessageInfo, SubMsg, Uint64, WasmMsg,
    },
    schemars::{self, JsonSchema},
};

use crate::{
    cmd::Borrow,
    contract::{execute, instantiate, query},
    msg::{ConfigResponse, ExecuteMsg, QueryMsg, Repayment},
    ContractError,
};

const CREATOR: &str = "creator";
const LPP_ADDR: &str = "test";
type TheCurrency = Usdc;
const DENOM: &str = TheCurrency::TICKER;
const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(30);

fn leaser_instantiate_msg(lease_code_id: u64, lpp_addr: Addr) -> crate::msg::InstantiateMsg {
    crate::msg::InstantiateMsg {
        lease_code_id: Uint64::new(lease_code_id),
        lpp_ust_addr: lpp_addr,
        lease_interest_rate_margin: MARGIN_INTEREST_RATE,
        liability: Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(5),
            Percent::from_percent(10),
            Percent::from_percent(2),
            Percent::from_percent(3),
            Percent::from_percent(2),
            1,
        ),
        repayment: Repayment::new(Duration::from_days(90), Duration::from_days(10)),
        time_alarms: Addr::unchecked("timealarms"),
        market_price_oracle: Addr::unchecked("oracle"),
        profit: Addr::unchecked("profit"),
        dex: ConnectionParams {
            connection_id: "connection-0".into(),
            transfer_channel: Ics20Channel {
                local_endpoint: "channel-0".into(),
                remote_endpoint: "channel-2048".into(),
            },
        },
    }
}

fn setup_test_case(deps: DepsMut) -> MessageInfo {
    let lpp_addr = Addr::unchecked(LPP_ADDR);
    let msg = leaser_instantiate_msg(1, lpp_addr);

    let info = mock_info(CREATOR, &coins(2, DENOM));
    let _res = instantiate(deps, mock_env(), info.clone(), msg).unwrap();
    info
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies();

    let lpp_addr = Addr::unchecked(LPP_ADDR);
    let msg = leaser_instantiate_msg(1, lpp_addr.clone());
    let info = mock_info(CREATOR, &coins(1000, DENOM));

    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_response: ConfigResponse = from_binary(&res).unwrap();
    let config = config_response.config;
    assert_eq!(CREATOR, config.owner);
    assert_eq!(1, config.lease_code_id);
    assert_eq!(lpp_addr, config.lpp_addr);
}

#[test]
fn test_update_config() {
    let mut deps = mock_dependencies();
    let expected_liability = Liability::new(
        Percent::from_percent(55),
        Percent::from_percent(5),
        Percent::from_percent(5),
        Percent::from_percent(1),
        Percent::from_percent(2),
        Percent::from_percent(1),
        12,
    );
    let expected_repaiment = Repayment::new(Duration::from_secs(100), Duration::from_secs(10));
    let info = setup_test_case(deps.as_mut());
    let msg = ExecuteMsg::Config {
        lease_interest_rate_margin: Percent::from_percent(5),
        liability: expected_liability,
        repayment: expected_repaiment.clone(),
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_response: ConfigResponse = from_binary(&res).unwrap();

    assert_eq!(expected_liability, config_response.config.liability);
    assert_eq!(expected_repaiment, config_response.config.repayment);
}

#[test]
#[should_panic(expected = "Period length should be greater than grace period")]
fn test_update_config_invalid_repay_period() {
    let mut deps = mock_dependencies();
    let expected_liability = Liability::new(
        Percent::from_percent(55),
        Percent::from_percent(5),
        Percent::from_percent(5),
        Percent::from_percent(1),
        Percent::from_percent(2),
        Percent::from_percent(1),
        12,
    );
    let expected_repaiment = Repayment::new(Duration::from_secs(18000), Duration::from_secs(23000));
    let info = setup_test_case(deps.as_mut());
    let msg = ExecuteMsg::Config {
        lease_interest_rate_margin: Percent::from_percent(5),
        liability: expected_liability,
        repayment: expected_repaiment,
    };
    execute(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
#[should_panic(expected = "BrokenInvariant")]
fn test_update_config_invalid_liability() {
    let mut deps = mock_dependencies();

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub struct Liability {
        initial: Percent,
        healthy: Percent,
        max: Percent,
        first_liq_warn: Percent,
        second_liq_warn: Percent,
        third_liq_warn: Percent,
        recalc_time: Duration,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum MockExecuteMsg {
        Config {
            lease_interest_rate_margin: Percent,
            liability: Liability,
            repayment: Repayment,
        },
        OpenLease {
            currency: String,
        },
    }

    let liability = Liability {
        initial: Percent::from_percent(55),
        healthy: Percent::from_percent(55),
        max: Percent::from_percent(55),
        first_liq_warn: Percent::from_percent(55),
        second_liq_warn: Percent::from_percent(55),
        third_liq_warn: Percent::from_percent(55),
        recalc_time: Duration::from_secs(100),
    };
    let mock_msg = MockExecuteMsg::Config {
        lease_interest_rate_margin: Percent::from_percent(5),
        liability,
        repayment: Repayment::new(Duration::from_secs(10), Duration::from_secs(10)),
    };

    let msg: ExecuteMsg = from_binary(&to_binary(&mock_msg).unwrap()).unwrap();

    let info = setup_test_case(deps.as_mut());

    execute(deps.as_mut(), mock_env(), info, msg).unwrap();
}

#[test]
fn test_update_config_unauthorized() {
    let mut deps = mock_dependencies();
    let expected_liability = Liability::new(
        Percent::from_percent(55),
        Percent::from_percent(5),
        Percent::from_percent(5),
        Percent::from_percent(1),
        Percent::from_percent(2),
        Percent::from_percent(1),
        12,
    );
    let expected_repaiment = Repayment::new(Duration::from_secs(10), Duration::from_secs(10));
    setup_test_case(deps.as_mut());
    let msg = ExecuteMsg::Config {
        lease_interest_rate_margin: Percent::from_percent(5),
        liability: expected_liability,
        repayment: expected_repaiment,
    };

    let info = mock_info("addr0000", coins(40, DENOM).as_ref());
    let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(ContractError::Unauthorized {}, err);
}

#[test]
fn test_open_lease() {
    let mut deps = mock_dependencies();
    setup_test_case(deps.as_mut());

    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config_response: ConfigResponse = from_binary(&res).unwrap();
    let config = config_response.config;

    // try open lease with enought UST
    let msg = ExecuteMsg::OpenLease {
        currency: DENOM.to_string(),
    };
    let info = mock_info("addr0000", coins(40, DENOM).as_ref());
    let res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    let msg = Borrow::open_lease_msg(info.sender, config, DENOM.to_string());
    assert_eq!(
        res.messages,
        vec![SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                funds: coins(40, DENOM),
                msg: to_binary(&msg).unwrap(),
                admin: None,
                code_id: 1,
                label: "lease".to_string()
            }),
            1
        )]
    );
}
