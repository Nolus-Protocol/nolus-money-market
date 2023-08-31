use serde::{Deserialize, Serialize};

use crate::{
    visitor::GeneralizedVisitorExt, AnyVisitor, Currency, Group, MaybeAnyVisitResult, SymbolStatic,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct Usdc;

impl Currency for Usdc {
    const TICKER: SymbolStatic = "uusdc";
    const BANK_SYMBOL: SymbolStatic = "ibc/uusdc";
    const DEX_SYMBOL: SymbolStatic = "ibc/dex_uusdc";
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct Nls;

impl Currency for Nls {
    const TICKER: SymbolStatic = "unls";
    const BANK_SYMBOL: SymbolStatic = "ibc/unls";
    const DEX_SYMBOL: SymbolStatic = "ibc/dex_unls";
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub struct Dai;

impl Currency for Dai {
    const TICKER: SymbolStatic = "udai";
    const BANK_SYMBOL: SymbolStatic = "ibc/udai";
    const DEX_SYMBOL: SymbolStatic = "ibc/dex_udai";
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct TestCurrencies {}

impl Group for TestCurrencies {
    const DESCR: SymbolStatic = "test";

    fn maybe_visit_on_by_ref<GV, V>(generalized_visitor: &GV, visitor: V) -> MaybeAnyVisitResult<V>
    where
        GV: GeneralizedVisitorExt,
        V: AnyVisitor,
    {
        generalized_visitor
            .maybe_visit::<Usdc, V>(visitor)
            .or_else(|visitor: V| generalized_visitor.maybe_visit::<Nls, V>(visitor))
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct TestExtraCurrencies {}

impl Group for TestExtraCurrencies {
    const DESCR: SymbolStatic = "test_extra";

    fn maybe_visit_on_by_ref<GV, V>(generalized_visitor: &GV, visitor: V) -> MaybeAnyVisitResult<V>
    where
        GV: GeneralizedVisitorExt,
        V: AnyVisitor,
    {
        generalized_visitor
            .maybe_visit::<Usdc, V>(visitor)
            .or_else(|visitor: V| generalized_visitor.maybe_visit::<Nls, V>(visitor))
            .or_else(|visitor: V| generalized_visitor.maybe_visit::<Dai, V>(visitor))
    }
}

pub mod visitor {
    use std::marker::PhantomData;

    use crate::{error::Error, AnyVisitor, AnyVisitorPair, AnyVisitorResult, Currency};

    #[derive(Debug, PartialEq, Eq, Clone)]
    pub struct Expect<C>(PhantomData<C>);

    impl<C> Default for Expect<C> {
        fn default() -> Self {
            Self(Default::default())
        }
    }

    impl<C> AnyVisitor for Expect<C>
    where
        C: 'static,
    {
        type Output = bool;
        type Error = Error;

        fn on<Cin>(self) -> AnyVisitorResult<Self>
        where
            Cin: 'static,
        {
            Ok(crate::equal::<C, Cin>())
        }
    }

    pub struct ExpectUnknownCurrency;

    impl AnyVisitor for ExpectUnknownCurrency {
        type Output = bool;
        type Error = Error;

        fn on<C>(self) -> AnyVisitorResult<Self>
        where
            C: Currency,
        {
            unreachable!();
        }
    }
    pub struct ExpectPair<C1, C2>(PhantomData<C1>, PhantomData<C2>);
    impl<C1, C2> Default for ExpectPair<C1, C2> {
        fn default() -> Self {
            Self(Default::default(), Default::default())
        }
    }
    impl<C1, C2> AnyVisitorPair for ExpectPair<C1, C2>
    where
        C1: 'static,
        C2: 'static,
    {
        type Output = bool;
        type Error = Error;

        fn on<C1in, C2in>(self) -> Result<Self::Output, Self::Error>
        where
            C1in: Currency,
            C2in: Currency,
        {
            Ok(crate::equal::<C1, C1in>() && crate::equal::<C2, C2in>())
        }
    }
}

pub mod group {
    use crate::{test::visitor::Expect, Currency, GroupExt, Symbol};

    #[track_caller]
    pub fn maybe_visit_on_ticker_impl<C, G>()
    where
        C: Currency,
        G: GroupExt,
    {
        let v = Expect::<C>::default();
        assert_eq!(G::maybe_visit_on_ticker(C::TICKER, v), Ok(Ok(true)));
    }

    #[track_caller]
    pub fn maybe_visit_on_ticker_err<C, G>(unknown_ticker: Symbol<'_>)
    where
        C: Currency,
        G: GroupExt,
    {
        let v = Expect::<C>::default();
        assert_eq!(G::maybe_visit_on_ticker(unknown_ticker, v.clone()), Err(v));
    }

    #[track_caller]
    pub fn maybe_visit_on_bank_symbol_impl<C, G>()
    where
        C: Currency,
        G: GroupExt,
    {
        let v = Expect::<C>::default();
        assert_eq!(
            G::maybe_visit_on_bank_symbol(C::BANK_SYMBOL, v),
            Ok(Ok(true))
        );
    }

    #[track_caller]
    pub fn maybe_visit_on_bank_symbol_err<C, G>(unknown_ticker: Symbol<'_>)
    where
        C: Currency,
        G: GroupExt,
    {
        let v = Expect::<C>::default();
        assert_eq!(
            G::maybe_visit_on_bank_symbol(unknown_ticker, v.clone()),
            Err(v)
        );
    }
}
