use crate::{
    error::Error, group::MemberOf, matcher, pairs::PairsGroup, Currency, CurrencyDTO, CurrencyDef,
    MaybeAnyVisitResult, Symbol, SymbolSlice,
};

use super::Group;

use self::impl_any_tickers::FirstTickerVisitor;

pub type AnyVisitorResult<VisitedG, Visitor> =
    Result<<Visitor as AnyVisitor<VisitedG>>::Output, <Visitor as AnyVisitor<VisitedG>>::Error>;

pub type AnyVisitorPairResult<V> =
    Result<<V as AnyVisitorPair>::Output, <V as AnyVisitorPair>::Error>;

pub trait AnyVisitor<VisitedG>
where
    VisitedG: Group,
{
    type Output;
    type Error;

    fn on<C>(self, def: &CurrencyDTO<C::Group>) -> AnyVisitorResult<VisitedG, Self>
    where
        // for the sake of generating less monomorphized functions, try to:
        // C: Currency + MemberOf<VisitedG> + MemberOf<VisitedG::TopG> + ...
        C: CurrencyDef + PairsGroup<CommonGroup = VisitedG::TopG>,
        C::Group: MemberOf<VisitedG> + MemberOf<VisitedG::TopG>;
}

pub trait InPoolWith<C> {}

pub trait AnyVisitorPair {
    type VisitedG: Group<TopG = Self::VisitedG>;

    type Output;
    type Error;

    fn on<C1, C2>(
        self,
        dto1: &CurrencyDTO<Self::VisitedG>,
        dto2: &CurrencyDTO<Self::VisitedG>,
    ) -> AnyVisitorPairResult<Self>
    where
        C1: Currency + MemberOf<Self::VisitedG>,
        C2: Currency + MemberOf<Self::VisitedG> + InPoolWith<C1>;
}

pub trait GroupVisit
where
    Self: Symbol,
{
    fn visit_any<V>(
        symbol: &SymbolSlice,
        visitor: V,
    ) -> Result<AnyVisitorResult<Self::Group, V>, Error>
    where
        V: AnyVisitor<Self::Group>,
    {
        Self::maybe_visit_any(symbol, visitor)
            .map_err(|_visitor| Error::not_in_currency_group::<_, Self, Self::Group>(symbol))
    }

    fn maybe_visit_any<V>(symbol: &SymbolSlice, visitor: V) -> MaybeAnyVisitResult<Self::Group, V>
    where
        V: AnyVisitor<Self::Group>,
    {
        let matcher = matcher::symbol_matcher::<Self>(symbol);
        <Self::Group as Group>::maybe_visit(&matcher, visitor)
    }
}
impl<T> GroupVisit for T
where
    T: Symbol,
    T::Group: MemberOf<T::Group>,
{
}

pub fn visit_any_on_currencies<V>(
    currency1: CurrencyDTO<V::VisitedG>,
    currency2: CurrencyDTO<V::VisitedG>,
    visitor: V,
) -> Result<V::Output, V::Error>
where
    V: AnyVisitorPair,
    Error: Into<V::Error>,
{
    currency1.into_currency_type(FirstTickerVisitor::new(currency1, currency2, visitor))
}

mod impl_any_tickers {
    use std::marker::PhantomData;

    use crate::{
        error::Error,
        pairs::{PairsGroup, PairsVisitor, PairsVisitorResult},
        Currency, CurrencyDTO, CurrencyDef, Group, MemberOf,
    };

    use super::{AnyVisitor, AnyVisitorPair, AnyVisitorResult, InPoolWith};

    pub struct FirstTickerVisitor<V>
    where
        V: AnyVisitorPair,
    {
        currency1: CurrencyDTO<V::VisitedG>,
        currency2: CurrencyDTO<V::VisitedG>,
        visitor: V,
    }
    impl<V> FirstTickerVisitor<V>
    where
        V: AnyVisitorPair,
    {
        pub fn new(
            currency1: CurrencyDTO<V::VisitedG>,
            currency2: CurrencyDTO<V::VisitedG>,
            visitor: V,
        ) -> Self {
            Self {
                currency1,
                currency2,
                visitor,
            }
        }
    }
    impl<V> AnyVisitor<V::VisitedG> for FirstTickerVisitor<V>
    where
        V: AnyVisitorPair,
        Error: Into<V::Error>,
    {
        type Output = <V as AnyVisitorPair>::Output;
        type Error = <V as AnyVisitorPair>::Error;

        fn on<C1>(self, def: &CurrencyDTO<C1::Group>) -> AnyVisitorResult<V::VisitedG, Self>
        where
            C1: CurrencyDef + PairsGroup<CommonGroup = <V::VisitedG as Group>::TopG>,
            C1::Group: MemberOf<V::VisitedG> + MemberOf<<V::VisitedG as Group>::TopG>, // TODO since V::VisitedG === Self::VisitorG, do we need them both?
        {
            debug_assert_eq!(def, &self.currency1);
            self.currency2
                .may_into_pair_member_type(SecondTickerVisitor {
                    c: PhantomData::<C1>,
                    currency1: self.currency1,
                    currency2: self.currency2,
                    visitor: self.visitor,
                })
                .unwrap_or_else(|_| {
                    Err(Error::not_in_pool_with(&self.currency1, &self.currency2).into())
                })
        }
    }

    struct SecondTickerVisitor<C1, V>
    where
        C1: Currency,
        V: AnyVisitorPair,
    {
        c: PhantomData<C1>,
        currency1: CurrencyDTO<V::VisitedG>,
        currency2: CurrencyDTO<V::VisitedG>,
        visitor: V,
    }
    impl<C1, V> PairsVisitor for SecondTickerVisitor<C1, V>
    where
        C1: CurrencyDef + PairsGroup,
        //+ PairsGroup<CommonGroup = <V::VisitedG as Group>::TopG>,
        C1::Group: MemberOf<V::VisitedG> + MemberOf<<C1 as PairsGroup>::CommonGroup>,
        V: AnyVisitorPair<VisitedG = C1::CommonGroup>,
    {
        type Pivot = C1;
        // C1::CommonGroup;

        type Output = <V as AnyVisitorPair>::Output;
        type Error = <V as AnyVisitorPair>::Error;

        fn on<C2>(self, currency: &CurrencyDTO<C2::Group>) -> PairsVisitorResult<Self>
        where
            C2: CurrencyDef + InPoolWith<Self::Pivot>,
            C2::Group: MemberOf<<Self::Pivot as PairsGroup>::CommonGroup>,
        {
            debug_assert_eq!(currency, &self.currency2);
            self.visitor.on::<C1, C2>(&self.currency1, &self.currency2)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        error::Error,
        from_symbol_any::GroupVisit,
        test::{
            Expect, ExpectPair, ExpectUnknownCurrency, SubGroup, SubGroupTestC10, SubGroupTestC6,
            SuperGroup, SuperGroupTestC1, SuperGroupTestC2, SuperGroupTestC3, SuperGroupTestC4,
        },
        CurrencyDef, Group, MemberOf, Tickers,
    };

    #[test]
    fn visit_any() {
        let v_usdc = Expect::<SuperGroupTestC1, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(true),
            Tickers::<SuperGroup>::visit_any(SuperGroupTestC1::ticker(), v_usdc.clone()).unwrap()
        );
        assert_eq!(
            Ok(Ok(true)),
            Tickers::<SuperGroup>::maybe_visit_any(SuperGroupTestC1::ticker(), v_usdc)
        );

        let v_nls = Expect::<SuperGroupTestC2, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(true),
            Tickers::<SuperGroup>::visit_any(SuperGroupTestC2::ticker(), v_nls).unwrap()
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
            .unwrap()
        );
    }

    #[test]
    fn visit_any_not_in_group() {
        let v_usdc = Expect::<SuperGroupTestC1, SuperGroup, SuperGroup>::new();
        assert_eq!(
            Ok(false),
            Tickers::<SuperGroup>::visit_any(SubGroupTestC10::ticker(), v_usdc).unwrap()
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
        //Pool pairs: 1:2, 1:4, 2:3, 4:5, 2:6, 2:10, 5:10, 6:10
        visit_any_currencies_ok::<SuperGroup, SuperGroupTestC1, SuperGroupTestC2>();
        visit_any_currencies_ok::<SuperGroup, SuperGroupTestC2, SubGroupTestC6>();

        // visit_any_currencies_ok::<SubGroup, SubGroupTestC10, SubGroupTestC6>();
        visit_any_currencies_ok::<SuperGroup, SubGroupTestC10, SubGroupTestC6>();

        visit_any_currencies_ok::<SuperGroup, SuperGroupTestC2, SubGroupTestC10>();
        visit_any_currencies_ok::<SuperGroup, SubGroupTestC10, SuperGroupTestC2>();

        visit_any_currencies_nok::<SuperGroup, SubGroupTestC10, SubGroupTestC10>();

        visit_any_currencies_nok::<SuperGroup, SuperGroupTestC1, SuperGroupTestC3>();
        visit_any_currencies_nok::<SuperGroup, SuperGroupTestC3, SuperGroupTestC1>();

        visit_any_currencies_nok::<SuperGroup, SuperGroupTestC2, SuperGroupTestC4>();
        visit_any_currencies_nok::<SuperGroup, SuperGroupTestC4, SuperGroupTestC2>();
    }

    fn visit_any_currencies_ok<VisitedG, CDef1, CDef2>()
    where
        VisitedG: Group<TopG = VisitedG>,
        CDef1: CurrencyDef,
        CDef1::Group: MemberOf<CDef1::Group> + MemberOf<VisitedG>,
        CDef2: CurrencyDef,
        CDef2::Group: MemberOf<CDef2::Group> + MemberOf<VisitedG>,
    {
        assert_eq!(
            Ok(true),
            visit_any_currencies_int::<VisitedG, CDef1, CDef2>()
        );
    }

    fn visit_any_currencies_nok<VisitedG, CDef1, CDef2>()
    where
        VisitedG: Group<TopG = VisitedG>,
        CDef1: CurrencyDef,
        CDef1::Group: MemberOf<CDef1::Group> + MemberOf<VisitedG>,
        CDef2: CurrencyDef,
        CDef2::Group: MemberOf<CDef2::Group> + MemberOf<VisitedG>,
    {
        assert_eq!(
            Err(Error::not_in_pool_with(
                &CDef1::definition().dto().into_super_group::<VisitedG>(),
                &CDef2::definition().dto().into_super_group::<VisitedG>()
            )),
            visit_any_currencies_int::<VisitedG, CDef1, CDef2>()
        );
    }

    fn visit_any_currencies_int<VisitedG, CDef1, CDef2>() -> Result<bool, Error>
    where
        VisitedG: Group<TopG = VisitedG>,
        CDef1: CurrencyDef,
        CDef1::Group: MemberOf<CDef1::Group> + MemberOf<VisitedG>,
        CDef2: CurrencyDef,
        CDef2::Group: MemberOf<CDef2::Group> + MemberOf<VisitedG>,
    {
        let dto1 = crate::dto::<CDef1, _>();
        let dto2 = crate::dto::<CDef2, _>();
        let v_c1_c2 = ExpectPair::<VisitedG, _, _>::new(&dto1, &dto2);
        super::visit_any_on_currencies(dto1, dto2, v_c1_c2)
    }
}
