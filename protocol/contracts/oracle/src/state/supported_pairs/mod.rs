use std::{fmt::Debug, marker::PhantomData};

use ::currencies::{LeaseGroup, Lpns, Native, PaymentOnlyGroup};
use serde::{Deserialize, Serialize};

use currency::{Currency, CurrencyDTO, CurrencyDef, Group, MemberOf};
use sdk::{cosmwasm_std::Storage, cw_storage_plus::Item};
use tree::{FindBy as _, NodeRef};

use crate::{
    api::{self, swap::SwapTarget, SwapLeg},
    error::{self, Error},
    result::Result,
};

mod currencies;

type Tree<G> = tree::Tree<SwapTarget<G>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", bound(serialize = "", deserialize = ""))]
pub(crate) struct SupportedPairs<PriceG, BaseC>
where
    PriceG: Group,
{
    tree: Tree<PriceG>,
    #[serde(skip)]
    _type: PhantomData<BaseC>,
}

impl<PriceG, BaseC> SupportedPairs<PriceG, BaseC>
where
    PriceG: Group,
    BaseC: CurrencyDef,
    BaseC::Group: MemberOf<PriceG>,
{
    const DB_ITEM: Item<SupportedPairs<PriceG, BaseC>> = Item::new("supported_pairs");

    pub fn new<StableC>(tree: Tree<PriceG>) -> Result<Self, PriceG>
    where
        StableC: CurrencyDef,
        StableC::Group: MemberOf<PriceG>,
    {
        Self::check_tree::<StableC>(&tree).map(|()| SupportedPairs {
            tree,
            _type: PhantomData,
        })
    }

    pub fn load(storage: &dyn Storage) -> Result<Self, PriceG> {
        Self::DB_ITEM
            .load(storage)
            .map_err(Error::LoadSupportedPairs)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> Result<(), PriceG> {
        Self::DB_ITEM
            .save(storage, self)
            .map_err(Error::StoreSupportedPairs)
    }

    pub fn load_path<'r>(
        &'r self,
        currency: &CurrencyDTO<PriceG>,
    ) -> Result<
        impl DoubleEndedIterator<Item = &'r CurrencyDTO<PriceG>> + 'r + use<'r, PriceG, BaseC>,
        PriceG,
    > {
        self.internal_load_path(currency)
            .map(|iter| iter.map(|node| &node.value().target))
    }

    pub fn load_swap_path(
        &self,
        from: &CurrencyDTO<PriceG>,
        to: &CurrencyDTO<PriceG>,
    ) -> Result<Vec<SwapTarget<PriceG>>, PriceG> {
        let path_from = self.internal_load_path(from)?;

        let mut path_to: Vec<_> = self.internal_load_path(to)?.collect();

        let mut path = vec![];

        path.extend(
            path_from
                .take_while(|node| {
                    if let Some((index, _)) = path_to
                        .iter()
                        .enumerate()
                        .rfind(|&(_, to_node)| node.value() == to_node.value())
                    {
                        path_to.truncate(index);

                        return false;
                    }

                    true
                })
                .filter_map(|node| {
                    Some(SwapTarget {
                        pool_id: node.value().pool_id,
                        target: node.parent()?.value().target,
                    })
                }),
        );

        path_to
            .into_iter()
            .rev()
            .for_each(|node| path.push(node.value().clone()));

        Ok(path)
    }

    pub fn swap_pairs_df(&self) -> impl Iterator<Item = SwapLeg<PriceG>> + '_ {
        self.tree
            .iter()
            .filter_map(|node: NodeRef<'_, SwapTarget<PriceG>>| {
                let parent: NodeRef<'_, SwapTarget<PriceG>> = node.parent()?;

                let SwapTarget {
                    pool_id,
                    target: child,
                } = node.value().clone();

                Some(SwapLeg {
                    from: child,
                    to: SwapTarget {
                        pool_id,
                        target: parent.value().target,
                    },
                })
            })
    }

    pub fn query_swap_tree(self) -> Tree<PriceG> {
        self.tree
    }

    fn internal_load_path<'r>(
        &'r self,
        query: &CurrencyDTO<PriceG>,
    ) -> Result<
        impl DoubleEndedIterator<Item = NodeRef<'r, SwapTarget<PriceG>>> + 'r + use<'r, PriceG, BaseC>,
        PriceG,
    > {
        self.tree
            .find_by(|target| &target.target == query)
            .map(|node| std::iter::once(node).chain(node.parents_iter()))
            .ok_or_else(|| error::unsupported_currency::<PriceG, BaseC>(*query))
        // intentionally copy-ed a CurrencyDTO and not following the best practices since this case should be extremely rare
    }

    fn check_tree<StableC>(tree: &Tree<PriceG>) -> Result<(), PriceG>
    where
        StableC: CurrencyDef,
        StableC::Group: MemberOf<PriceG>,
    {
        let root_currency = tree.root().value().target;
        if root_currency != currency::dto::<BaseC, _>() {
            Err(error::invalid_base_currency::<_, BaseC>(root_currency))
        } else {
            let mut supported_currencies: Vec<&CurrencyDTO<PriceG>> =
                tree.iter().map(|ref node| &node.value().target).collect();

            supported_currencies.sort_unstable();

            if supported_currencies
                .binary_search(&&currency::dto::<StableC, _>())
                .is_err()
            {
                Err(Error::StableCurrencyNotInTree {})
            } else if supported_currencies
                .windows(2)
                .any(|window| window[0] == window[1])
            {
                Err(Error::DuplicatedNodes {})
            } else {
                Ok(())
            }
        }
    }
}

impl<PriceG, BaseC> SupportedPairs<PriceG, BaseC>
where
    PriceG: Group,
    BaseC: Currency + MemberOf<PriceG>,
    LeaseGroup: MemberOf<PriceG>,
    Lpns: MemberOf<PriceG>,
    Native: MemberOf<PriceG>,
    PaymentOnlyGroup: MemberOf<PriceG>,
{
    pub fn currencies(&self) -> impl Iterator<Item = api::Currency> + '_ {
        currencies::currencies(self.tree.iter())
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod tests {
    use std::cmp::Ordering;

    use ::currencies::{
        testing::{LeaseC1, LeaseC2, LeaseC3, LeaseC4, LeaseC5, PaymentC4},
        Lpn, Nls, PaymentGroup as PriceCurrencies,
    };
    use currency::{CurrencyDTO, CurrencyDef, MemberOf};
    use sdk::cosmwasm_std::{self, testing};
    use tree::HumanReadableTree;

    use crate::{
        api::{self, swap::SwapTarget, SwapLeg},
        error::{self, Error},
    };

    type SupportedPairs = super::SupportedPairs<PriceCurrencies, TheCurrency>;

    type TheCurrency = Lpn;

    fn test_case() -> HumanReadableTree<SwapTarget<PriceCurrencies>> {
        cosmwasm_std::from_json(format!(
            r#"{{
                "value":[0,"{base}"],
                "children":[
                    {{
                        "value":[4,"{lease4}"],
                        "children":[
                            {{"value":[3,"{lease3}"]}}
                        ]
                    }},
                    {{
                        "value":[2,"{lease2}"],
                        "children":[
                            {{
                                "value":[1,"{lease1}"],
                                "children":[
                                    {{"value":[5,"{lease5}"]}},
                                    {{"value":[6,"{native}"]}}
                                ]
                            }}
                        ]
                    }}
                ]
            }}"#,
            base = TheCurrency::ticker(),
            lease1 = LeaseC1::ticker(),
            lease2 = LeaseC2::ticker(),
            lease3 = LeaseC3::ticker(),
            lease4 = LeaseC4::ticker(),
            lease5 = LeaseC5::ticker(),
            native = Nls::ticker(),
        ))
        .expect("123")
    }

    #[test]
    fn test_storage() {
        let tree = test_case();
        let sp = SupportedPairs::new::<Lpn>(tree.into_tree()).unwrap();
        let mut deps = testing::mock_dependencies();

        sp.save(deps.as_mut().storage).unwrap();
        let restored = SupportedPairs::load(deps.as_ref().storage).unwrap();

        assert_eq!(restored, sp);
    }

    #[test]
    fn test_invalid_base() {
        let tree: HumanReadableTree<_> = cosmwasm_std::from_json(format!(
            r#"{{
                "value": [0, "{lease1}"],
                "children": [
                    {{
                        "value": [1, "{stable}"]
                    }}
                ]
            }}"#,
            lease1 = LeaseC1::dto(),
            stable = Lpn::dto(),
        ))
        .unwrap();

        assert_eq!(
            SupportedPairs::new::<Lpn>(tree.into_tree()),
            Err(error::invalid_base_currency::<PriceCurrencies, Lpn>(
                LeaseC1::dto().into_super_group()
            ))
        );
    }

    #[test]
    fn test_duplicated_nodes() {
        let tree: HumanReadableTree<_> = cosmwasm_std::from_json(format!(
            r#"{{
                "value": [0, "{base}"],
                "children":[
                    {{
                        "value": [1, "{lease1}"]
                    }},
                    {{
                        "value": [2, "{lease2}"],
                        "children": [
                            {{
                                "value": [1, "{lease1}"]
                            }}
                        ]
                    }}
                ]
            }}"#,
            base = TheCurrency::dto(),
            lease1 = LeaseC1::dto(),
            lease2 = LeaseC2::dto(),
        ))
        .unwrap();

        assert_eq!(
            SupportedPairs::new::<TheCurrency>(tree.into_tree()),
            Err(Error::DuplicatedNodes {})
        );
    }

    #[test]
    fn test_unknown_stable_currency() {
        let tree: HumanReadableTree<_> = cosmwasm_std::from_json(format!(
            r#"{{
                "value": [0, "{base}"],
                "children":[
                    {{
                        "value": [1, "{lease1}"]
                    }}
                ]
            }}"#,
            base = TheCurrency::dto(),
            lease1 = LeaseC1::dto(),
        ))
        .unwrap();

        assert_eq!(
            SupportedPairs::new::<PaymentC4>(tree.into_tree()),
            Err(Error::StableCurrencyNotInTree {})
        );
    }

    #[test]
    fn test_load_path() {
        let tree = SupportedPairs::new::<TheCurrency>(test_case().into_tree()).unwrap();

        let resp: Vec<_> = tree
            .load_path(&currency_dto::<LeaseC5>())
            .unwrap()
            .collect();

        assert_eq!(
            resp.as_slice(),
            [
                &currency_dto::<LeaseC5>(),
                &currency_dto::<LeaseC1>(),
                &currency_dto::<LeaseC2>(),
                &currency_dto::<TheCurrency>(),
            ]
        );
    }

    #[test]
    fn test_load_swap_path() {
        let tree = SupportedPairs::new::<TheCurrency>(test_case().into_tree()).unwrap();

        assert!(tree
            .load_swap_path(&currency_dto::<LeaseC5>(), &currency_dto::<LeaseC5>())
            .unwrap()
            .is_empty());

        let resp = tree
            .load_swap_path(&currency_dto::<LeaseC5>(), &currency_dto::<TheCurrency>())
            .unwrap();
        let expect = vec![
            SwapTarget {
                pool_id: 5,
                target: currency_dto::<LeaseC1>(),
            },
            SwapTarget {
                pool_id: 1,
                target: currency_dto::<LeaseC2>(),
            },
            SwapTarget {
                pool_id: 2,
                target: currency_dto::<TheCurrency>(),
            },
        ];

        assert_eq!(resp, expect);

        let resp = tree
            .load_swap_path(&currency_dto::<Nls>(), &currency_dto::<LeaseC5>())
            .unwrap();
        let expect = vec![
            SwapTarget {
                pool_id: 6,
                target: currency_dto::<LeaseC1>(),
            },
            SwapTarget {
                pool_id: 5,
                target: currency_dto::<LeaseC5>(),
            },
        ];
        assert_eq!(resp, expect);

        let resp = tree
            .load_swap_path(&currency_dto::<LeaseC2>(), &currency_dto::<LeaseC4>())
            .unwrap();
        let expect = vec![
            SwapTarget {
                pool_id: 2,
                target: currency_dto::<TheCurrency>(),
            },
            SwapTarget {
                pool_id: 4,
                target: currency_dto::<LeaseC4>(),
            },
        ];
        assert_eq!(resp, expect);
    }

    #[test]
    fn test_query_supported_pairs() {
        let paths = test_case();
        let tree = SupportedPairs::new::<TheCurrency>(paths.into_tree()).unwrap();

        fn leg_cmp(a: &SwapLeg<PriceCurrencies>, b: &SwapLeg<PriceCurrencies>) -> Ordering {
            a.from.cmp(&b.from)
        }

        let mut response: Vec<_> = tree.swap_pairs_df().collect();
        response.sort_by(leg_cmp);

        let mut expected = vec![
            SwapLeg {
                from: currency_dto::<LeaseC2>(),
                to: SwapTarget {
                    pool_id: 2,
                    target: currency_dto::<TheCurrency>(),
                },
            },
            SwapLeg {
                from: currency_dto::<LeaseC4>(),
                to: SwapTarget {
                    pool_id: 4,
                    target: currency_dto::<TheCurrency>(),
                },
            },
            SwapLeg {
                from: currency_dto::<LeaseC1>(),
                to: SwapTarget {
                    pool_id: 1,
                    target: currency_dto::<LeaseC2>(),
                },
            },
            SwapLeg {
                from: currency_dto::<Nls>(),
                to: SwapTarget {
                    pool_id: 6,
                    target: currency_dto::<LeaseC1>(),
                },
            },
            SwapLeg {
                from: currency_dto::<LeaseC5>(),
                to: SwapTarget {
                    pool_id: 5,
                    target: currency_dto::<LeaseC1>(),
                },
            },
            SwapLeg {
                from: currency_dto::<LeaseC3>(),
                to: SwapTarget {
                    pool_id: 3,
                    target: currency_dto::<LeaseC4>(),
                },
            },
        ];
        expected.sort_by(leg_cmp);

        assert_eq!(response, expected);
    }

    #[test]
    fn currencies() {
        let listed_currencies: Vec<_> = SupportedPairs::new::<TheCurrency>(
            cosmwasm_std::from_json::<HumanReadableTree<_>>(format!(
                r#"{{
                    "value":[0,{0:?}],
                    "children":[
                        {{
                            "value":[1,{1:?}],
                            "children":[
                                {{"value":[2,{2:?}]}}
                            ]
                        }},
                        {{"value":[3,{3:?}]}}
                    ]
                }}"#,
                TheCurrency::ticker(),
                LeaseC1::ticker(),
                Nls::ticker(),
                LeaseC2::ticker(),
            ))
            .unwrap()
            .into_tree(),
        )
        .unwrap()
        .currencies()
        .collect();

        assert_eq!(
            listed_currencies.as_slice(),
            [
                api::Currency::new(currency_dto::<Lpn>().definition(), api::CurrencyGroup::Lpn),
                api::Currency::new(
                    currency_dto::<LeaseC1>().definition(),
                    api::CurrencyGroup::Lease
                ),
                api::Currency::new(
                    currency_dto::<Nls>().definition(),
                    api::CurrencyGroup::Native
                ),
                api::Currency::new(
                    currency_dto::<LeaseC2>().definition(),
                    api::CurrencyGroup::Lease
                ),
            ]
        );
    }

    fn currency_dto<C>() -> CurrencyDTO<PriceCurrencies>
    where
        C: CurrencyDef,
        C::Group: MemberOf<PriceCurrencies>,
    {
        currency::dto::<C, _>()
    }
}
