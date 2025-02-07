use currencies::{testing::LeaseC1, LeaseGroup, Lpn};
use currency::{CurrencyDTO, CurrencyDef as _};
use finance::{
    coin::{Amount, Coin},
    duration::Duration,
    liability::Liability,
    percent::Percent,
};
use lease::api::{
    open::{ConnectionParams, Ics20Channel, PositionSpecDTO},
    LpnCoinDTO,
};
use platform::contract::{Code, CodeId};
use sdk::{
    cosmwasm_std::{
        coins, from_json, testing,
        testing::{MockApi, MockQuerier, MockStorage},
        to_json_binary, Addr, CosmosMsg, Deps, DepsMut, MessageInfo, OwnedDeps, SubMsg, WasmMsg,
    },
    testing as sdk_testing,
};

use crate::{
    cmd::Borrow,
    contract::{execute, instantiate, query, sudo},
    msg::{ConfigResponse, ExecuteMsg, QueryMsg, SudoMsg},
    state::config::Config,
};

const CREATOR: &str = "creator";
const LPP_ADDR: &str = "test";
const TIMEALARMS_ADDR: &str = "timealarms";
const ORACLE_ADDR: &str = "oracle";
const PROFIT_ADDR: &str = "profit";
const RESERVE_ADDR: &str = "reserve";
const PROTOCOLS_REGISTRY_ADDR: &str = "protocols";

type TheCurrency = Lpn;

fn lease_currency() -> CurrencyDTO<LeaseGroup> {
    currency::dto::<LeaseC1, _>()
}

const MARGIN_INTEREST_RATE: Percent = Percent::from_permille(30);

fn leaser_instantiate_msg(lease_code: Code, lpp: Addr) -> crate::msg::InstantiateMsg {
    crate::msg::InstantiateMsg {
        lease_code: CodeId::from(lease_code).into(),
        lpp,
        profit: sdk_testing::user(PROFIT_ADDR),
        reserve: sdk_testing::user(RESERVE_ADDR),
        time_alarms: sdk_testing::user(TIMEALARMS_ADDR),
        market_price_oracle: sdk_testing::user(ORACLE_ADDR),
        protocols_registry: sdk_testing::user(PROTOCOLS_REGISTRY_ADDR),
        lease_position_spec: PositionSpecDTO::new(
            Liability::new(
                Percent::from_percent(65),
                Percent::from_percent(70),
                Percent::from_percent(73),
                Percent::from_percent(75),
                Percent::from_percent(78),
                Percent::from_percent(80),
                Duration::from_hours(1),
            ),
            lpn_coin(1000),
            lpn_coin(10),
        ),
        lease_interest_rate_margin: MARGIN_INTEREST_RATE,
        lease_due_period: Duration::from_days(90),
        dex: dex_params(),
    }
}

fn owner() -> MessageInfo {
    MessageInfo {
        sender: sdk_testing::user(CREATOR),
        funds: vec![],
    }
}

fn customer() -> MessageInfo {
    MessageInfo {
        sender: sdk_testing::user("addr0000"),
        funds: coins(2, TheCurrency::dex()),
    }
}

fn setup_test_case(deps: DepsMut<'_>) {
    let lpp_addr = sdk_testing::user(LPP_ADDR);
    let msg = leaser_instantiate_msg(Code::unchecked(1), lpp_addr);

    let resp = instantiate(deps, testing::mock_env(), owner(), msg).unwrap();
    assert_eq!(0, resp.messages.len());
}

fn query_config(deps: Deps<'_>) -> Config {
    let res = query(deps, testing::mock_env(), QueryMsg::Config {}).unwrap();
    let config_response: ConfigResponse = from_json(res).unwrap();
    config_response.config
}

fn dex_params() -> ConnectionParams {
    ConnectionParams {
        connection_id: "connection-0".into(),
        transfer_channel: Ics20Channel {
            local_endpoint: "channel-0".into(),
            remote_endpoint: "channel-2048".into(),
        },
    }
}

#[test]
fn proper_initialization() {
    let mut deps = deps();

    let lpp_addr = sdk_testing::user(LPP_ADDR);
    let lease_code = Code::unchecked(1);
    let msg = leaser_instantiate_msg(lease_code, lpp_addr.clone());

    let res = instantiate(deps.as_mut(), testing::mock_env(), owner(), msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(deps.as_ref(), testing::mock_env(), QueryMsg::Config {}).unwrap();
    let config_response: ConfigResponse = from_json(res).unwrap();
    let config = config_response.config;
    assert_eq!(lease_code, config.lease_code);
    assert_eq!(lpp_addr, config.lpp);
}

#[test]
fn test_update_config() {
    let mut deps = deps();

    let expected_liability = Liability::new(
        Percent::from_percent(55),
        Percent::from_percent(60),
        Percent::from_percent(61),
        Percent::from_percent(62),
        Percent::from_percent(64),
        Percent::from_percent(65),
        Duration::from_hours(12),
    );
    let expected_position_spec = PositionSpecDTO::new(
        expected_liability,
        lpn_coin(4_211_442_000),
        lpn_coin(100_000),
    );
    let expected_due_period = Duration::from_secs(100);

    setup_test_case(deps.as_mut());

    let msg = SudoMsg::Config {
        lease_interest_rate_margin: Percent::from_percent(5),
        lease_position_spec: expected_position_spec,
        lease_due_period: expected_due_period,
    };

    sudo(deps.as_mut(), testing::mock_env(), msg).unwrap();

    let config = query_config(deps.as_ref());
    assert_eq!(expected_position_spec, config.lease_position_spec);
    assert_eq!(expected_due_period, config.lease_due_period);
}

fn open_lease_with(max_ltd: Option<Percent>) {
    let mut deps = deps();

    setup_test_case(deps.as_mut());

    let config = query_config(deps.as_ref());

    let msg = ExecuteMsg::OpenLease {
        currency: lease_currency(),
        max_ltd,
    };
    let info = customer();
    let env = testing::mock_env();
    let admin = env.contract.address.clone();
    let finalizer = admin.clone();
    let res = execute(deps.as_mut(), env, info.clone(), msg).unwrap();

    let msg = Borrow::open_lease_msg(info.sender, config, lease_currency(), max_ltd, finalizer);
    assert_eq!(
        res.messages,
        vec![SubMsg::reply_on_success(
            CosmosMsg::Wasm(WasmMsg::Instantiate {
                funds: info.funds,
                msg: to_json_binary(&msg).unwrap(),
                admin: Some(admin.into()),
                code_id: 1,
                label: "lease".to_string(),
            }),
            0,
        )]
    );
}

#[test]
fn test_open_lease() {
    open_lease_with(None);
}

#[test]
fn test_open_lease_with_max_ltd() {
    open_lease_with(None);
    open_lease_with(Some(Percent::from_percent(5)));
}

fn deps() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    sdk_testing::mock_deps_with_contracts([
        sdk_testing::user(LPP_ADDR),
        sdk_testing::user(TIMEALARMS_ADDR),
        sdk_testing::user(PROFIT_ADDR),
        sdk_testing::user(ORACLE_ADDR),
    ])
}

fn lpn_coin(amount: Amount) -> LpnCoinDTO {
    Coin::<TheCurrency>::from(amount).into()
}
