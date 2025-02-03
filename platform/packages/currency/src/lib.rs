use std::{any::TypeId, fmt::Debug};

pub use crate::{
    definition::Definition,
    dto::{dto, to_string, CurrencyDTO},
    from_symbol::{CurrencyVisit, SingleVisitor},
    from_symbol_any::{
        visit_any_on_currencies, AnyVisitor, AnyVisitorPair, AnyVisitorPairResult,
        AnyVisitorResult, GroupVisit, InPoolWith,
    },
    group::{Group, MaybeAnyVisitResult, MemberOf},
    matcher::{Matcher, TypeMatcher},
    pairs::{MaybePairsVisitorResult, PairsGroup, PairsVisitor, PairsVisitorResult},
    symbol::{BankSymbols, DexSymbols, Symbol, Tickers},
};

mod definition;
mod dto;
pub mod error;
mod from_symbol;
mod from_symbol_any;
mod group;
mod matcher;
pub mod never;
mod pairs;
pub mod platform;
mod symbol;
#[cfg(any(test, feature = "testing"))]
pub mod test;

// TODO get rid of these definitions. Move some to much smaller scope, for example move SymbolOwned close to CurrencyDTO
// and SymbolStatic close to Symbols
pub type SymbolSlice = str;
pub type SymbolStatic = &'static SymbolSlice;
pub type SymbolOwned = String;

/// Currency market trait
///
/// Designed to enforce static checks at compile-time guard protecting against mismatches in financial, at al, operations.
pub trait Currency: Copy + Ord + Debug + 'static {}

/// Represent a Currency definition, including the currency group, the ticker, the symbols on Nolus and the DEX network, and the number of decimals.
///
pub trait CurrencyDef: Currency {
    /// The group this currency belongs to
    type Group: Group;

    fn definition() -> &'static Self;

    fn dto(&self) -> &CurrencyDTO<Self::Group>;

    #[cfg(any(test, feature = "testing"))]
    fn ticker() -> SymbolStatic {
        Self::definition().dto().definition().ticker
    }

    #[cfg(any(test, feature = "testing"))]
    fn bank() -> SymbolStatic {
        Self::definition().dto().definition().bank_symbol
    }

    #[cfg(any(test, feature = "testing"))]
    fn dex() -> SymbolStatic {
        Self::definition().dto().definition().dex_symbol
    }
}

impl<T> Currency for T where T: CurrencyDef {}

pub fn equal<C1, C2>() -> bool
where
    C1: 'static,
    C2: 'static,
{
    TypeId::of::<C1>() == TypeId::of::<C2>()
}

pub fn maybe_visit_any<M, C, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<C::Group, V>
where
    M: Matcher,
    C: CurrencyDef + PairsGroup<CommonGroup = <C::Group as Group>::TopG>,
    C::Group: MemberOf<C::Group> + MemberOf<<C::Group as Group>::TopG>,
    V: AnyVisitor<C::Group>,
{
    maybe_visit_member::<_, C, C::Group, _>(matcher, visitor)
}

pub fn maybe_visit_member<M, C, VisitedG, V>(
    matcher: &M,
    visitor: V,
) -> MaybeAnyVisitResult<VisitedG, V>
where
    M: Matcher,
    C: CurrencyDef + PairsGroup<CommonGroup = VisitedG::TopG>,
    C::Group: MemberOf<VisitedG> + MemberOf<VisitedG::TopG>,
    V: AnyVisitor<VisitedG>,
    VisitedG: Group,
{
    let member = C::definition().dto();
    if matcher.r#match(member.definition()) {
        Ok(visitor.on::<C>(member))
    } else {
        Err(visitor)
    }
}

pub fn maybe_visit_buddy<C, M, V>(matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
where
    M: Matcher,
    C: CurrencyDef
        + InPoolWith<V::Pivot>
        + PairsGroup<CommonGroup = <V::Pivot as PairsGroup>::CommonGroup>,
    C::Group: MemberOf<<V::Pivot as PairsGroup>::CommonGroup>,
    V: PairsVisitor,
{
    let buddy = C::definition().dto();
    if matcher.r#match(buddy.definition()) {
        Ok(visitor.on::<C>(buddy))
    } else {
        Err(visitor)
    }
}

pub fn visit_noone<R, V>(visitor: V) -> Result<R, V> {
    Err(visitor)
}
