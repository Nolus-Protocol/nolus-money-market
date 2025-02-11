use std::{
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};

use crate::{
    definition::DefinitionRef,
    error::{Error, Result},
    group::MemberOf,
    pairs::{MaybePairsVisitorResult, PairsGroup, PairsVisitor, PairsVisitorResult},
    CurrencyDef, Group, MaybeAnyVisitResult, Symbol, SymbolStatic, Tickers, TypeMatcher,
};

use super::{AnyVisitor, AnyVisitorResult};

mod unchecked;

/// Data-Transferable currency belonging to a group
///
/// This is a value type designed for efficient representation, data transfer and storage.
/// `GroupMember` specifies which currencies are valid instances of this type.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(
    try_from = "unchecked::TickerDTO",
    into = "unchecked::TickerDTO",
    bound(deserialize = "G: Group")
)]
pub struct CurrencyDTO<G>
where
    G: Group,
{
    def: DefinitionRef,
    _host_group: PhantomData<G>,
}

impl<G> CurrencyDTO<G>
where
    G: Group,
{
    pub const fn new(def: DefinitionRef) -> Self {
        Self {
            def,
            _host_group: PhantomData,
        }
    }

    pub fn may_into_currency_type<SubG, V>(self, visitor: V) -> MaybeAnyVisitResult<SubG, V>
    where
        SubG: Group,
        V: AnyVisitor<SubG>,
    {
        SubG::maybe_visit(&TypeMatcher::new(self.def), visitor)
    }

    pub fn into_currency_type<V>(self, visitor: V) -> AnyVisitorResult<G, V>
    where
        V: AnyVisitor<G>,
    {
        G::maybe_visit(&TypeMatcher::new(self.def), visitor).unwrap_or_else(|_| self.unexpected())
    }

    pub fn may_into_pair_member_type<V>(self, visitor: V) -> MaybePairsVisitorResult<V>
    where
        V: PairsVisitor,
    {
        V::Pivot::maybe_visit(&TypeMatcher::new(self.def), visitor)
    }

    pub fn into_pair_member_type<V>(self, visitor: V) -> PairsVisitorResult<V>
    where
        V: PairsVisitor,
    {
        self.may_into_pair_member_type(visitor)
            .unwrap_or_else(|_| self.unknown_buddy::<V::Pivot, _>())
    }

    pub fn into_super_group<SuperG>(self) -> CurrencyDTO<SuperG>
    where
        SuperG: Group,
        G: MemberOf<SuperG>,
    {
        CurrencyDTO::<SuperG> {
            def: self.def,
            _host_group: PhantomData,
        }
    }

    pub fn definition(&self) -> DefinitionRef {
        self.def
    }

    pub fn into_symbol<S>(self) -> SymbolStatic
    where
        S: Symbol,
    {
        S::symbol(self.def)
    }

    pub fn of_currency<SubG>(&self, def: &CurrencyDTO<SubG>) -> Result<()>
    where
        SubG: Group + MemberOf<G>,
    {
        if self == def {
            Ok(())
        } else {
            Err(Error::currency_mismatch(def, self))
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn from_symbol_testing<S>(symbol: &str) -> Result<CurrencyDTO<S::Group>>
    where
        S: Symbol<Group = G>,
    {
        Self::from_symbol::<S>(symbol)
    }

    fn from_symbol<S>(symbol: &str) -> Result<CurrencyDTO<S::Group>>
    where
        S: Symbol<Group = G>,
    {
        use crate::GroupVisit;

        struct TypeToCurrency<G>(PhantomData<G>);

        impl<G> AnyVisitor<G> for TypeToCurrency<G>
        where
            G: Group,
        {
            type Output = CurrencyDTO<G>;

            type Error = Error;

            fn on<C>(self, def: &CurrencyDTO<C::Group>) -> AnyVisitorResult<G, Self>
            where
                C: CurrencyDef,
                C::Group: MemberOf<G>,
            {
                Ok(def.into_super_group())
            }
        }

        // V: AnyVisitor<<Self::Group as Group>::TopG>,
        S::visit_any(symbol, TypeToCurrency(PhantomData::<S::Group>))
    }

    fn unexpected<R>(self) -> R
    where
        G: Group,
    {
        panic!(
            r#"Found an invalid currency instance! "{:?}" did not match "{}" !"#,
            self,
            G::DESCR,
        )
    }

    fn unknown_buddy<P, R>(self) -> R
    where
        P: PairsGroup,
    {
        panic!(
            r#"Found an unknown currency "{:?}" in the pairs group of "{}" !"#,
            self,
            std::any::type_name::<P>(),
        )
    }
}

impl<G, RhsG> PartialEq<CurrencyDTO<RhsG>> for CurrencyDTO<G>
where
    G: Group,
    RhsG: Group,
{
    fn eq(&self, other: &CurrencyDTO<RhsG>) -> bool {
        self.def.eq(other.def)
    }
}

/// Prepare a human-friendly representation of a currency
pub fn to_string<G>(dto: &CurrencyDTO<G>) -> SymbolStatic
where
    G: Group,
{
    let res = dto.into_symbol::<Tickers<G>>();
    debug_assert_eq!(dto.to_string(), res);
    res
}

pub fn dto<C, G>() -> CurrencyDTO<G>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
{
    C::dto().into_super_group::<G>()
}

impl<G> Display for CurrencyDTO<G>
where
    G: Group,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        unchecked::TickerDTO::from(*self).fmt(f)
    }
}

impl<G> Hash for CurrencyDTO<G>
where
    G: Group,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.def.hash(state);
    }
}

#[cfg(test)]
mod test {
    use crate::{
        test::{self, SubGroup, SubGroupTestC10, SuperGroup, SuperGroupTestC1, SuperGroupTestC2},
        BankSymbols, CurrencyDTO, CurrencyDef, DexSymbols, Group, MemberOf, Tickers,
    };

    #[test]
    fn eq_same_type() {
        assert_eq!(
            dto::<SuperGroup, SuperGroupTestC1>(),
            dto::<SuperGroup, SuperGroupTestC1>(),
        );

        assert_ne!(
            dto::<SuperGroup, SuperGroupTestC1>(),
            dto::<SuperGroup, SuperGroupTestC2>(),
        );
    }

    #[test]
    fn into_currency_type() {
        let c1 = dto::<SuperGroup, SuperGroupTestC1>();

        assert_eq!(
            c1.into_currency_type(test::Expect::<SuperGroupTestC1, SuperGroup, SuperGroup>::new()),
            Ok(true),
        );

        assert_eq!(
            c1.into_currency_type(test::Expect::<SuperGroupTestC2, SuperGroup, SuperGroup>::new()),
            Ok(false),
        );
    }

    #[test]
    fn into_super_group() {
        let sub_currency = dto::<SubGroup, SubGroupTestC10>();

        assert_eq!(
            dto::<SubGroup, SubGroupTestC10>(),
            sub_currency.into_super_group::<SuperGroup>()
        )
    }

    #[test]
    fn from_super_group() {
        assert_eq!(
            dto::<SubGroup, SubGroupTestC10>(),
            dto::<SuperGroup, SubGroupTestC10>(),
        );

        assert_eq!(
            dto::<<SubGroupTestC10 as CurrencyDef>::Group, SubGroupTestC10>(),
            dto::<SubGroup, SubGroupTestC10>()
        );
    }

    #[test]
    fn eq_other_type() {
        assert_ne!(
            dto::<SuperGroup, SuperGroupTestC1>(),
            dto::<SubGroup, SubGroupTestC10>(),
        );
    }

    #[test]
    fn to_string() {
        assert_eq!(
            dto::<<SubGroupTestC10 as CurrencyDef>::Group, SubGroupTestC10>().to_string(),
            super::to_string(SubGroupTestC10::dto())
        );
    }

    #[test]
    fn into_symbol() {
        type TheC = SuperGroupTestC1;
        type TheG = <TheC as CurrencyDef>::Group;

        assert_eq!(
            SuperGroupTestC1::bank(),
            dto::<SuperGroup, SuperGroupTestC1>().into_symbol::<BankSymbols::<TheG>>()
        );

        assert_eq!(
            SuperGroupTestC1::dex(),
            dto::<SuperGroup, SuperGroupTestC1>().into_symbol::<DexSymbols::<TheG>>()
        );

        assert_eq!(
            SuperGroupTestC1::ticker(),
            dto::<SuperGroup, SuperGroupTestC1>().into_symbol::<Tickers::<TheG>>()
        );

        let c = dto::<SuperGroup, SuperGroupTestC1>();

        assert_eq!(c.to_string(), c.into_symbol::<Tickers::<TheG>>());
    }

    fn dto<G, C>() -> CurrencyDTO<G>
    where
        G: Group,
        C: CurrencyDef,
        C::Group: MemberOf<G>,
    {
        super::dto::<C, G>()
    }
}
