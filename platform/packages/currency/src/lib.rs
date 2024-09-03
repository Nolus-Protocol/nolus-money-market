use std::{any::TypeId, fmt::Debug};

use from_symbol_any::{MaybePivotVisitResult, PivotVisitor};

pub use crate::{
    definition::Definition,
    dto::{dto, to_string, CurrencyDTO},
    from_symbol::{CurrencyVisit, SingleVisitor},
    from_symbol_any::{
        visit_any_on_currencies, AnyVisitor, AnyVisitorPair, AnyVisitorPairResult,
        AnyVisitorResult, GroupVisit,
    },
    group::{Group, MaybeAnyVisitResult, MemberOf},
    matcher::{Matcher, TypeMatcher},
    nls::{Native as NativePlatform, NlsPlatform},
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
mod nls;
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

pub fn into_super_group<C, SuperG>(c_def: C) -> CurrencyDTO<SuperG>
where
    C: CurrencyDef<Group = SuperG>,
    SuperG: Group,
{
    *c_def.dto()
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
    C: CurrencyDef,
    C::Group: MemberOf<C::Group> + MemberOf<V::VisitorG>,
    V: AnyVisitor<C::Group>,
{
    maybe_visit_member::<_, C, C::Group, _>(matcher, visitor)
}

pub fn maybe_visit_member<M, C, TopG, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<TopG, V>
where
    M: Matcher,
    C: CurrencyDef,
    C::Group: MemberOf<TopG> + MemberOf<V::VisitorG>,
    V: AnyVisitor<TopG>,
    TopG: Group,
{
    if matcher.r#match(C::definition().dto().definition()) {
        Ok(visitor.on::<C>(C::definition()))
    } else {
        Err(visitor)
    }
}

pub fn maybe_visit_pivot<M, C, CurrencyG, V>(
    buddy: &CurrencyDTO<CurrencyG>,
    matcher: &M,
    visitor: V,
) -> MaybePivotVisitResult<V>
where
    M: Matcher,
    C: Currency + MemberOf<CurrencyG> + MemberOf<V::VisitedG>,
    CurrencyG: Group + MemberOf<V::VisitedG>,
    V: PivotVisitor,
{
    if matcher.r#match(buddy.definition()) {
        Ok(visitor.on::<C, CurrencyG>(buddy))
    } else {
        Err(visitor)
    }
}

pub fn visit_noone<V>(visitor: V) -> MaybePivotVisitResult<V>
where
    V: PivotVisitor,
{
    Err(visitor)
}
