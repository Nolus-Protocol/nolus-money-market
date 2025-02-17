use currency::{AnyVisitor, Group, Matcher, MaybeAnyVisitResult, MemberOf};

use crate::payment::Group as PaymentGroup;

use super::Group as LeaseGroup;

use self::definitions::{LeaseC1, LeaseC2, LeaseC3, LeaseC4, LeaseC5, LeaseC6, LeaseC7};

pub(super) fn maybe_visit<M, V, VisitedG>(
    matcher: &M,
    visitor: V,
) -> MaybeAnyVisitResult<VisitedG, V>
where
    LeaseGroup: MemberOf<VisitedG>,
    M: Matcher,
    V: AnyVisitor<VisitedG>,
    VisitedG: Group<TopG = PaymentGroup>,
{
    use currency::maybe_visit_member as visit;

    visit::<_, LeaseC1, VisitedG, _>(matcher, visitor)
        .or_else(|visitor| visit::<_, LeaseC2, VisitedG, _>(matcher, visitor))
        .or_else(|visitor| visit::<_, LeaseC3, VisitedG, _>(matcher, visitor))
        .or_else(|visitor| visit::<_, LeaseC4, VisitedG, _>(matcher, visitor))
        .or_else(|visitor| visit::<_, LeaseC5, VisitedG, _>(matcher, visitor))
        .or_else(|visitor| visit::<_, LeaseC6, VisitedG, _>(matcher, visitor))
        .or_else(|visitor| visit::<_, LeaseC7, VisitedG, _>(matcher, visitor))
}

pub(super) mod definitions {
    use serde::{Deserialize, Serialize};

    use currency::{
        CurrencyDTO, CurrencyDef, Definition, InPoolWith, Matcher, MaybePairsVisitorResult,
        PairsGroup, PairsVisitor,
    };

    use crate::{lpn::Lpn, native::Nls, payment::Group as PaymentGroup};

    use super::super::Group as LeaseGroup;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC1(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC1 {
        type Group = LeaseGroup;

        #[inline]
        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC1", "ibc/bank_LC1", "ibc/dex_LC1", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC1 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<LeaseC2, _, _>(matcher, visitor)
                .or_else(|visitor| visit::<LeaseC3, _, _>(matcher, visitor))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC2(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC2 {
        type Group = LeaseGroup;

        #[inline]
        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC2", "ibc/bank_LC2", "ibc/dex_LC2", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC2 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<Lpn, _, _>(matcher, visitor)
        }
    }

    impl InPoolWith<LeaseC1> for LeaseC2 {}

    impl InPoolWith<LeaseC3> for LeaseC2 {}

    impl InPoolWith<LeaseC4> for LeaseC2 {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC3(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC3 {
        type Group = LeaseGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC3", "ibc/bank_LC3", "ibc/dex_LC3", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC3 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<LeaseC2, _, _>(matcher, visitor)
        }
    }

    impl InPoolWith<LeaseC1> for LeaseC3 {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC4(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC4 {
        type Group = LeaseGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC4", "ibc/bank_LC4", "ibc/dex_LC4", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC4 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<LeaseC2, _, _>(matcher, visitor)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC5(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC5 {
        type Group = LeaseGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC5", "ibc/bank_LC5", "ibc/dex_LC5", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC5 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<Nls, _, _>(matcher, visitor)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC6(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC6 {
        type Group = LeaseGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC6", "ibc/bank_LC6", "ibc/dex_LC6", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC6 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(_: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            // let's stay detached from the swap tree for some corner cases.
            currency::visit_noone(visitor)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct LeaseC7(CurrencyDTO<LeaseGroup>);

    impl CurrencyDef for LeaseC7 {
        type Group = LeaseGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const {
                CurrencyDTO::new(
                    const { &Definition::new("LC7", "ibc/bank_LC7", "ibc/dex_LC7", 6) },
                )
            }
        }
    }

    impl PairsGroup for LeaseC7 {
        type CommonGroup = PaymentGroup;

        #[inline]
        fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
        where
            M: Matcher,
            V: PairsVisitor<Pivot = Self>,
        {
            use currency::maybe_visit_buddy as visit;

            visit::<Lpn, _, _>(matcher, visitor)
        }
    }
}

#[cfg(test)]
mod test {
    use currency::CurrencyDef as _;

    use crate::{
        lpn::{Group as Lpns, Lpn},
        native::Nls,
        test_impl::{
            maybe_visit_on_bank_symbol_err, maybe_visit_on_bank_symbol_impl,
            maybe_visit_on_ticker_err, maybe_visit_on_ticker_impl,
        },
    };

    use super::{LeaseC1, LeaseC2, LeaseC3, LeaseC4, LeaseC5, LeaseC6, LeaseC7, LeaseGroup};

    #[test]
    fn maybe_visit_on_ticker() {
        maybe_visit_on_ticker_impl::<LeaseC1, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC2, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC3, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC4, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC5, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC6, LeaseGroup>();
        maybe_visit_on_ticker_impl::<LeaseC7, LeaseGroup>();
        maybe_visit_on_ticker_err::<Lpn, Lpns>(Lpn::bank());
        maybe_visit_on_ticker_err::<LeaseC2, LeaseGroup>(LeaseC2::bank());
        maybe_visit_on_ticker_err::<LeaseC3, LeaseGroup>(LeaseC3::dex());
    }

    #[test]
    fn maybe_visit_on_bank_symbol() {
        maybe_visit_on_bank_symbol_impl::<LeaseC1, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC2, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC3, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC4, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC5, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC6, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<LeaseC7, LeaseGroup>();
        maybe_visit_on_bank_symbol_err::<Lpn, Lpns>(Lpn::ticker());
        maybe_visit_on_bank_symbol_err::<LeaseC1, LeaseGroup>(LeaseC1::ticker());
        maybe_visit_on_bank_symbol_err::<LeaseC1, LeaseGroup>(LeaseC1::dex());
        maybe_visit_on_bank_symbol_err::<LeaseC1, LeaseGroup>(Lpn::ticker());
        maybe_visit_on_bank_symbol_err::<LeaseC1, LeaseGroup>(Nls::bank());
        maybe_visit_on_bank_symbol_err::<LeaseC1, LeaseGroup>(Nls::ticker());
        maybe_visit_on_bank_symbol_err::<LeaseC5, LeaseGroup>(LeaseC5::ticker());
    }
}
