use access_control::SingleUserAccess;
use marketprice::config::Config as PriceConfig;
use sdk::{
    cosmwasm_ext::Response,
    cosmwasm_std::{MessageInfo, Storage},
};

use crate::{msg::ConfigResponse, state::config::Config, ContractError};

pub fn query_config(storage: &dyn Storage) -> Result<ConfigResponse, ContractError> {
    let owner = SingleUserAccess::load_contract_owner(storage)?.into();
    let config = Config::load(storage)?;

    Ok(ConfigResponse { owner, config })
}

pub fn try_configure(
    storage: &mut dyn Storage,
    info: MessageInfo,
    price_config: PriceConfig,
) -> Result<Response, ContractError> {
    SingleUserAccess::check_owner_access::<ContractError>(storage, &info.sender)?;

    Config::update(storage, price_config)?;

    Ok(Response::new())
}

#[cfg(test)]
mod tests {
    use currency::{
        lease::{Cro, Osmo},
        lpn::Usdc,
        native::Nls,
    };
    use finance::{currency::Currency, duration::Duration, percent::Percent};
    use marketprice::config::Config as PriceConfig;
    use sdk::cosmwasm_std::{
        coins, from_binary,
        testing::{mock_env, mock_info},
    };
    use swap::SwapTarget;

    use crate::{
        contract::{execute, query},
        msg::{ConfigResponse, ExecuteMsg, QueryMsg},
        state::{config::Config, supported_pairs::SwapLeg},
        tests::{dummy_default_instantiate_msg, dummy_instantiate_msg, setup_test},
    };

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn configure_unauthorized() {
        let msg = dummy_instantiate_msg(
            Usdc::TICKER.to_string(),
            60,
            Percent::from_percent(50),
            // TreeStore(tr((0, Usdc::TICKER.to_string())) / tr((1, Cro::TICKER.to_string()))),
            serde_json_wasm::from_str(&format!(
                r#"{{
                    "value":[0,"{usdc}"],
                    "children":[
                        {{"value":[1,"{cro}"]}}
                    ]
                }}"#,
                usdc = Usdc::TICKER,
                cro = Cro::TICKER,
            ))
            .unwrap(),
        );
        let (mut deps, _) = setup_test(msg);

        let unauth_info = mock_info("anyone", &coins(2, Nls::TICKER));
        let msg = ExecuteMsg::UpdateConfig(PriceConfig::new(
            Percent::from_percent(12),
            Duration::from_secs(5),
            3,
            Percent::from_percent(88),
        ));
        let _res = execute(deps.as_mut(), mock_env(), unauth_info, msg).unwrap();
    }

    #[test]
    fn configure() {
        use marketprice::config::Config as PriceConfig;
        let msg = dummy_instantiate_msg(
            Usdc::TICKER.to_string(),
            60,
            Percent::from_percent(50),
            serde_json_wasm::from_str(&dbg!(format!(
                r#"{{
                    "value":[0,"{usdc}"],
                    "children":[
                        {{"value":[1,"{cro}"]}}
                    ]
                }}"#,
                usdc = Usdc::TICKER,
                cro = Cro::TICKER,
            )))
            .unwrap(),
        );
        let (mut deps, info) = setup_test(msg);

        let msg = ExecuteMsg::UpdateConfig(PriceConfig::new(
            Percent::from_percent(44),
            Duration::from_secs(5),
            7,
            Percent::from_percent(88),
        ));
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(
            ConfigResponse {
                owner: info.sender,
                config: Config {
                    base_asset: Usdc::TICKER.into(),
                    price_config: PriceConfig::new(
                        Percent::from_percent(44),
                        Duration::from_secs(5),
                        7,
                        Percent::from_percent(88),
                    )
                }
            },
            value
        );
    }

    #[test]
    fn config_supported_pairs() {
        let (mut deps, info) = setup_test(dummy_default_instantiate_msg());

        let test_tree = serde_json_wasm::from_str(&format!(
            r#"{{
                "value":[0,"{usdc}"],
                "children":[
                    {{"value":[1,"{cro}"]}},
                    {{"value":[2,"{osmo}"]}}
                ]
            }}"#,
            usdc = Usdc::TICKER,
            cro = Cro::TICKER,
            osmo = Osmo::TICKER,
        ))
        .unwrap();

        let msg = ExecuteMsg::SwapTree { tree: test_tree };
        let res = execute(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_ok());

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SupportedCurrencyPairs {},
        )
        .unwrap();
        let mut value: Vec<SwapLeg> = from_binary(&res).unwrap();
        value.sort_by(|a, b| a.from.cmp(&b.from));

        let mut expected = vec![
            SwapLeg {
                from: Cro::TICKER.into(),
                to: SwapTarget {
                    pool_id: 1,
                    target: Usdc::TICKER.into(),
                },
            },
            SwapLeg {
                from: Osmo::TICKER.into(),
                to: SwapTarget {
                    pool_id: 2,
                    target: Usdc::TICKER.into(),
                },
            },
        ];
        expected.sort_by(|a, b| a.from.cmp(&b.from));

        assert_eq!(expected, value);
    }

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn config_supported_pairs_unauthorized() {
        let (mut deps, _) = setup_test(dummy_default_instantiate_msg());
        let info = mock_info("user", &coins(1000, Nls::TICKER));

        let msg = ExecuteMsg::SwapTree {
            tree: serde_json_wasm::from_str(&format!(
                r#"{{
                    "value":[0,"{usdc}"],
                    "children":[
                        {{"value":[1,"{cro}"]}}
                    ]
                }}"#,
                usdc = Usdc::TICKER,
                cro = Cro::TICKER,
            ))
            .unwrap(),
        };

        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }

    #[test]
    #[should_panic]
    fn invalid_supported_pairs() {
        let (mut deps, info) = setup_test(dummy_default_instantiate_msg());

        let test_tree = serde_json_wasm::from_str(&format!(
            r#"{{
                "value":[0,"{usdc}"],
                "children":[
                    {{"value":[1,"{cro}"]}},
                    {{"value":[2,"{cro}"]}}
                ]
            }}"#,
            usdc = Usdc::TICKER,
            cro = Cro::TICKER,
        ))
        .unwrap();

        let msg = ExecuteMsg::SwapTree { tree: test_tree };

        execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    }
}
