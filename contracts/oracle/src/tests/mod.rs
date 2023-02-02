use currency::{
    lease::{Atom, Cro, Osmo, Wbtc, Weth},
    lpn::Usdc,
    native::Nls,
};
use finance::{
    coin::Coin,
    currency::{Currency, SymbolOwned},
    duration::Duration,
    percent::Percent,
    price::{self, dto::PriceDTO},
};
use marketprice::config::Config as PriceConfig;
use sdk::cosmwasm_std::{
    coins,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier},
    MemoryStorage, MessageInfo, OwnedDeps,
};
use swap::SwapTarget;
use tree::HumanReadableTree;

use crate::{
    contract::{execute, instantiate},
    msg::{ExecuteMsg, InstantiateMsg},
    state::config::Config,
};

#[cfg(test)]
mod oracle_tests;

pub(crate) const CREATOR: &str = "creator";

pub(crate) fn dummy_instantiate_msg(
    base_asset: SymbolOwned,
    price_feed_period_secs: u32,
    expected_feeders: Percent,
    swap_tree: HumanReadableTree<SwapTarget>,
) -> InstantiateMsg {
    InstantiateMsg {
        config: Config {
            base_asset,
            price_config: PriceConfig::new(
                expected_feeders,
                Duration::from_secs(price_feed_period_secs),
                1,
                Percent::from_percent(88),
            ),
        },
        swap_tree,
    }
}

pub(crate) fn dummy_default_instantiate_msg() -> InstantiateMsg {
    dummy_instantiate_msg(
        Usdc::TICKER.to_string(),
        60,
        Percent::from_percent(50),
        serde_json_wasm::from_str(&format!(
            r#"{{
                "value":[0,"{usdc}"],
                "children":[
                    {{
                        "value":[3,"{weth}"],
                        "children":[
                            {{
                                "value":[2,"{atom}"],
                                "children":[
                                    {{"value":[1,"{osmo}"]}}
                                ]
                            }}
                        ]
                    }},
                    {{
                        "value":[4,"{wbtc}"],
                        "children":[
                            {{"value":[5,"{cro}"]}}
                        ]
                    }}
                ]
            }}"#,
            usdc = Usdc::TICKER,
            weth = Weth::TICKER,
            atom = Atom::TICKER,
            osmo = Osmo::TICKER,
            wbtc = Wbtc::TICKER,
            cro = Cro::TICKER,
        ))
        .unwrap(),
    )
}

pub(crate) fn dummy_feed_prices_msg() -> ExecuteMsg {
    ExecuteMsg::FeedPrices {
        prices: vec![
            PriceDTO::try_from(price::total_of(Coin::<Osmo>::new(10)).is(Coin::<Atom>::new(12)))
                .unwrap(),
            PriceDTO::try_from(price::total_of(Coin::<Atom>::new(10)).is(Coin::<Weth>::new(32)))
                .unwrap(),
            PriceDTO::try_from(price::total_of(Coin::<Weth>::new(10)).is(Coin::<Usdc>::new(12)))
                .unwrap(),
            PriceDTO::try_from(price::total_of(Coin::<Wbtc>::new(10)).is(Coin::<Usdc>::new(120)))
                .unwrap(),
        ],
    }
}

pub(crate) fn setup_test(
    msg: InstantiateMsg,
) -> (OwnedDeps<MemoryStorage, MockApi, MockQuerier>, MessageInfo) {
    let mut deps = mock_dependencies();
    let info = mock_info(CREATOR, &coins(1000, Nls::TICKER));
    let res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    assert_eq!(res.messages.len(), 0);

    // register single feeder address
    let msg = ExecuteMsg::RegisterFeeder {
        feeder_address: CREATOR.to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    (deps, info)
}
