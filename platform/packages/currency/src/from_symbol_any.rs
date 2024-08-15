use crate::{
    error::Error, group::MemberOf, matcher, CurrencyDTO, CurrencyDef, MaybeAnyVisitResult, Symbol,
    SymbolSlice,
};

use super::Group;

use self::impl_any_tickers::FirstTickerVisitor;

pub type AnyVisitorResult<VisitedG, Visitor> =
    Result<<Visitor as AnyVisitor<VisitedG>>::Output, <Visitor as AnyVisitor<VisitedG>>::Error>;
pub type AnyVisitorPairResult<V> =
    Result<<V as AnyVisitorPair>::Output, <V as AnyVisitorPair>::Error>;

pub trait AnyVisitor<VisitedG>
where
    VisitedG: Group + MemberOf<Self::VisitorG>,
{
    type VisitorG: Group;

    type Output;
    type Error;

    fn on<C>(self, def: &C) -> AnyVisitorResult<VisitedG, Self>
    where
        C: CurrencyDef,
        C::Group: MemberOf<VisitedG> + MemberOf<Self::VisitorG>;
}
pub trait AnyVisitorPair {
    type VisitedG1: Group;
    type VisitedG2: Group;

    type Output;
    type Error;

    fn on<C1, C2>(self, def1: &C1, def2: &C2) -> AnyVisitorPairResult<Self>
    where
        C1: CurrencyDef + MemberOf<Self::VisitedG1>,
        C1::Group: MemberOf<Self::VisitedG1>,
        C2: CurrencyDef + MemberOf<Self::VisitedG2>,
        C2::Group: MemberOf<Self::VisitedG2>;
}

pub trait GroupVisit: Symbol {
    fn visit_any<V>(symbol: &SymbolSlice, visitor: V) -> Result<V::Output, V::Error>
    where
        V: AnyVisitor<Self::Group, VisitorG = Self::Group>,
        Error: Into<V::Error>,
    {
        Self::maybe_visit_any(symbol, visitor).unwrap_or_else(|_| {
            Err(Error::not_in_currency_group::<_, Self, Self::Group>(symbol).into())
        })
    }

    fn maybe_visit_any<V>(symbol: &SymbolSlice, visitor: V) -> MaybeAnyVisitResult<Self::Group, V>
    where
        V: AnyVisitor<Self::Group, VisitorG = Self::Group>,
    {
        let matcher = matcher::symbol_matcher::<Self>(symbol);
        Self::Group::maybe_visit(&matcher, visitor)
    }

    fn visit_member_any<V>(symbol: &SymbolSlice, visitor: V) -> Result<V::Output, V::Error>
    where
        V: AnyVisitor<Self::Group>,
        Self::Group: MemberOf<V::VisitorG>,
        Error: Into<V::Error>,
    {
        Self::maybe_visit_member_any(symbol, visitor).unwrap_or_else(|_| {
            Err(Error::not_in_currency_group::<_, Self, Self::Group>(symbol).into())
        })
    }

    fn maybe_visit_member_any<V>(
        symbol: &SymbolSlice,
        visitor: V,
    ) -> MaybeAnyVisitResult<Self::Group, V>
    where
        V: AnyVisitor<Self::Group>,
        Self::Group: MemberOf<V::VisitorG>,
    {
        let matcher = matcher::symbol_matcher::<Self>(symbol);
        Self::Group::maybe_visit_super_visitor(&matcher, visitor)
    }
}
impl<T> GroupVisit for T where T: Symbol {}

pub fn visit_any_on_currencies<G1, G2, V>(
    currency1: CurrencyDTO<G1>,
    currency2: CurrencyDTO<G2>,
    visitor: V,
) -> Result<V::Output, V::Error>
where
    G1: Group,
    G2: Group,
    V: AnyVisitorPair<VisitedG1 = G1, VisitedG2 = G2>,
{
    currency1.into_currency_type(FirstTickerVisitor::<G1, G2, _>::new(currency2, visitor))
}

mod impl_any_tickers {
    use std::marker::PhantomData;

    use crate::{CurrencyDTO, CurrencyDef, Group, MemberOf};

    use super::{AnyVisitor, AnyVisitorPair, AnyVisitorResult};

    pub struct FirstTickerVisitor<G1, G2, V>
    where
        G2: Group,
        V: AnyVisitorPair,
    {
        group1: PhantomData<G1>,
        currency2: CurrencyDTO<G2>,
        visitor: V,
    }
    impl<G1, G2, V> FirstTickerVisitor<G1, G2, V>
    where
        G2: Group,
        V: AnyVisitorPair,
    {
        pub fn new(ticker2: CurrencyDTO<G2>, visitor: V) -> Self {
            Self {
                group1: PhantomData::<G1>,
                currency2: ticker2,
                visitor,
            }
        }
    }
    impl<G1, G2, V> AnyVisitor<G1> for FirstTickerVisitor<G1, G2, V>
    where
        G1: Group,
        G2: Group,
        V: AnyVisitorPair<VisitedG1 = G1, VisitedG2 = G2>,
    {
        type VisitorG = G1;

        type Output = <V as AnyVisitorPair>::Output;
        type Error = <V as AnyVisitorPair>::Error;

        fn on<C1>(self, def1: &C1) -> AnyVisitorResult<G1, Self>
        where
            C1: CurrencyDef,
            C1::Group: MemberOf<G1>,
        {
            self.currency2.into_currency_type(SecondTickerVisitor {
                def1,
                group2: PhantomData::<G2>,
                visitor: self.visitor,
            })
        }
    }

    struct SecondTickerVisitor<'def1, C1, G2, V>
    where
        C1: CurrencyDef,
        V: AnyVisitorPair,
    {
        def1: &'def1 C1,
        group2: PhantomData<G2>,
        visitor: V,
    }
    impl<'def1, C1, G2, V> AnyVisitor<G2> for SecondTickerVisitor<'def1, C1, G2, V>
    where
        C1: CurrencyDef,
        C1::Group: MemberOf<V::VisitedG1>,
        G2: Group,
        V: AnyVisitorPair<VisitedG2 = G2>,
    {
        type VisitorG = G2;

        type Output = <V as AnyVisitorPair>::Output;
        type Error = <V as AnyVisitorPair>::Error;

        fn on<C2>(self, def2: &C2) -> AnyVisitorResult<G2, Self>
        where
            C2: CurrencyDef,
            C2::Group: MemberOf<Self::VisitorG>,
        {
            self.visitor.on::<C1, C2>(self.def1, def2)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        error::Error,
        from_symbol_any::GroupVisit,
        test::{
            Expect, ExpectPair, ExpectUnknownCurrency, SubGroup, SubGroupTestC10, SuperGroup,
            SuperGroupTestC1, SuperGroupTestC2,
        },
        CurrencyDef, Group, MemberOf, Tickers,
    };

    #[test]
    fn visit_any() {
        let v_usdc = Expect::<SuperGroupTestC1, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(true),
            Tickers::<SuperGroup>::visit_any(SuperGroupTestC1::ticker(), v_usdc.clone())
        );
        assert_eq!(
            Ok(Ok(true)),
            Tickers::<SuperGroup>::maybe_visit_any(SuperGroupTestC1::ticker(), v_usdc)
        );

        let v_nls = Expect::<SuperGroupTestC2, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(true),
            Tickers::<SuperGroup>::visit_any(SuperGroupTestC2::ticker(), v_nls)
        );

        assert_eq!(
            Err(Error::not_in_currency_group::<
                _,
                Tickers::<SubGroup>,
                SuperGroup,
            >(SubGroupTestC10::bank())),
            Tickers::<SuperGroup>::visit_any(
                SubGroupTestC10::bank(),
                ExpectUnknownCurrency::<SuperGroup>::new()
            )
        );
        let v = ExpectUnknownCurrency::<SuperGroup>::new();
        assert_eq!(
            Err(v.clone()),
            Tickers::<SuperGroup>::maybe_visit_any(SubGroupTestC10::bank(), v)
        );
    }

    #[test]
    fn visit_super_group() {
        assert_eq!(
            Ok(true),
            Tickers::<SuperGroup>::visit_any(
                SubGroupTestC10::ticker(),
                Expect::<SubGroupTestC10, SuperGroup, SuperGroup>::new()
            )
        );

        assert_eq!(
            Ok(true),
            Tickers::<SubGroup>::visit_member_any(
                SubGroupTestC10::ticker(),
                Expect::<SubGroupTestC10, SubGroup, SuperGroup>::new()
            )
        );
    }

    #[test]
    fn visit_any_not_in_group() {
        let v_usdc = Expect::<SuperGroupTestC1, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(false),
            Tickers::<SuperGroup>::visit_any(SubGroupTestC10::ticker(), v_usdc)
        );

        let v_usdc = ExpectUnknownCurrency::<SubGroup>::new();
        assert_eq!(
            Err(Error::not_in_currency_group::<
                _,
                Tickers::<SuperGroup>,
                SubGroup,
            >(SuperGroupTestC1::ticker())),
            Tickers::<SubGroup>::visit_any(SuperGroupTestC1::ticker(), v_usdc)
        );
    }

    #[test]
    fn visit_any_unexpected() {
        const DENOM: &str = "my_fancy_coin";

        assert_eq!(
            Tickers::<SuperGroup>::visit_any(DENOM, ExpectUnknownCurrency::<SuperGroup>::new()),
            Err(Error::not_in_currency_group::<
                _,
                Tickers::<SubGroup>,
                SuperGroup,
            >(DENOM)),
        );
    }

    #[test]
    fn visit_any_currencies() {
        visit_any_currencies_ok::<SuperGroup, SuperGroup, SuperGroupTestC1, SuperGroupTestC2>();
        visit_any_currencies_ok::<SuperGroup, SuperGroup, SuperGroupTestC2, SuperGroupTestC2>();
        visit_any_currencies_ok::<SubGroup, SuperGroup, SubGroupTestC10, SuperGroupTestC1>();
        visit_any_currencies_ok::<SuperGroup, SubGroup, SuperGroupTestC2, SubGroupTestC10>();

        visit_any_currencies_ok::<SuperGroup, SuperGroup, SubGroupTestC10, SuperGroupTestC2>();
        visit_any_currencies_ok::<SuperGroup, SuperGroup, SubGroupTestC10, SubGroupTestC10>();
    }

    fn visit_any_currencies_ok<VisitedG1, VisitedG2, CDef1, CDef2>()
    where
        VisitedG1: Group,
        VisitedG2: Group,
        CDef1: CurrencyDef,
        CDef1::Group: MemberOf<VisitedG1>,
        CDef2: CurrencyDef,
        CDef2::Group: MemberOf<VisitedG2>,
    {
        let v_c1_c2 = ExpectPair::<VisitedG1, VisitedG2, _, _>::new(
            CDef1::definition().dto(),
            CDef2::definition().dto(),
        );
        assert_eq!(
            Ok(true),
            super::visit_any_on_currencies::<VisitedG1, VisitedG2, _>(
                CDef1::definition().dto().into_super_group::<VisitedG1>(),
                CDef2::definition().dto().into_super_group::<VisitedG2>(),
                v_c1_c2
            )
        );
    }
}
