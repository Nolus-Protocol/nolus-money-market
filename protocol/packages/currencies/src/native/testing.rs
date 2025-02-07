pub(super) mod definitions {
    use serde::{Deserialize, Serialize};

    use currency::{
        CurrencyDTO, CurrencyDef, Definition, InPoolWith, Matcher, MaybePairsVisitorResult,
        PairsGroup, PairsVisitor,
    };

    use crate::{lease::LeaseC5, lpn::Lpn, payment::Group as PaymentGroup};

    use super::super::Group as NativeGroup;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct Nls(CurrencyDTO<NativeGroup>);

    impl CurrencyDef for Nls {
        type Group = NativeGroup;

        fn dto() -> &'static CurrencyDTO<Self::Group> {
            &const { CurrencyDTO::new(const { &Definition::new("NLS", "unls", "ibc/dex_NLS", 6) }) }
        }
    }

    impl PairsGroup for Nls {
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

    impl InPoolWith<LeaseC5> for Nls {}
}
