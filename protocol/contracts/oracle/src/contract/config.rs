use currency::Group;
use sdk::cosmwasm_std::Storage;

use crate::{api::Config, result::Result};

pub(super) fn query_config<PriceG>(storage: &dyn Storage) -> Result<Config, PriceG>
where
    PriceG: Group,
{
    Config::load(storage)
}

#[cfg(all(feature = "internal.test.contract", test))]
mod tests {
    use currencies::{testing::PaymentC9, Lpn, PaymentGroup as PriceCurrencies};
    use finance::{duration::Duration, percent::Percent};
    use sdk::{
        cosmwasm_ext::Response,
        cosmwasm_std::{from_json, testing::mock_env},
    };

    use crate::{
        api::{swap::SwapTarget, Config, QueryMsg, SudoMsg, SwapLeg},
        contract::{query, sudo},
        error::Error,
        test_tree, tests,
    };

    #[test]
    fn configure() {
        use marketprice::config::Config as PriceConfig;
        let msg = tests::dummy_instantiate_msg(
            60,
            Percent::from_percent(50),
            test_tree::dummy_swap_tree(),
        );
        let (mut deps, _info) = tests::setup_test(msg).unwrap();

        let msg = SudoMsg::UpdateConfig(PriceConfig::new(
            Percent::from_percent(44),
            Duration::from_secs(5),
            7,
            Percent::from_percent(88),
        ));

        let Response {
            messages,
            attributes,
            events,
            data,
            ..
        }: Response = sudo(deps.as_mut(), mock_env(), msg).unwrap();

        assert!(messages.is_empty());
        assert!(attributes.is_empty());
        assert!(events.is_empty());
        assert!(data.is_none());

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let value: Config = from_json(res).unwrap();
        assert_eq!(
            value,
            Config {
                price_config: PriceConfig::new(
                    Percent::from_percent(44),
                    Duration::from_secs(5),
                    7,
                    Percent::from_percent(88),
                )
            }
        );
    }

    #[test]
    fn config_supported_pairs() {
        let (mut deps, _info) = tests::setup_test(tests::dummy_default_instantiate_msg()).unwrap();

        let test_tree = test_tree::minimal_swap_tree();

        let res = sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::SwapTree { tree: test_tree },
        );
        assert!(res.is_ok());

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::SupportedCurrencyPairs {},
        )
        .unwrap();
        let mut value: Vec<SwapLeg<PriceCurrencies>> = from_json(res).unwrap();
        value.sort_by(|a, b| a.from.cmp(&b.from));

        let mut expected = vec![SwapLeg::<PriceCurrencies> {
            from: currency::dto::<PaymentC9, PriceCurrencies>().into_super_group(),
            to: SwapTarget {
                pool_id: 1,
                target: currency::dto::<Lpn, PriceCurrencies>().into_super_group(),
            },
        }];
        expected.sort_by(|a, b| a.from.cmp(&b.from));

        assert_eq!(value, expected);
    }

    #[test]
    fn invalid_supported_pairs() {
        let (mut deps, _info) = tests::setup_test(tests::dummy_default_instantiate_msg()).unwrap();

        let test_tree = test_tree::invalid_pair_swap_tree();

        let err = sudo(
            deps.as_mut(),
            mock_env(),
            SudoMsg::SwapTree { tree: test_tree },
        )
        .unwrap_err();

        assert!(matches!(err, Error::BrokenSwapTree(_)));
    }
}
