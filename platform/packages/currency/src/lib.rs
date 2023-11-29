use std::{any::TypeId, fmt::Debug};

use sdk::schemars::JsonSchema;

use crate::error::{Error, Result};

pub use self::{
    from_symbol::{CurrencyVisit, MaybeVisitResult, SingleVisitor},
    from_symbol_any::{
        visit_any_on_tickers, AnyVisitor, AnyVisitorPair, AnyVisitorPairResult, AnyVisitorResult,
        GroupVisit,
    },
    group::{Group, MaybeAnyVisitResult},
    matcher::{BankSymbols, DexSymbols, Matcher, Symbol, Symbols, Tickers},
    nls::{Native as NativePlatform, NlsPlatform},
};

pub mod error;
mod from_symbol;
mod from_symbol_any;
mod group;
mod matcher;
mod nls;
#[cfg(any(test, feature = "testing"))]
pub mod test;

pub type SymbolSlice = str;
pub type SymbolStatic = &'static SymbolSlice;
pub type SymbolOwned = String;

// Not extending Serialize + DeserializeOwbed since the serde derive implementations fail to
// satisfy trait bounds with regards of the lifetimes
// Foe example, https://stackoverflow.com/questions/70774093/generic-type-that-implements-deserializeowned
pub trait Currency: Debug + Copy + Ord + JsonSchema + Sized + 'static {
    /// Identifier of the currency
    const TICKER: SymbolStatic;

    /// Symbol at the Nolus network used by the Cosmos-SDK modules, mainly the Banking one
    const BANK_SYMBOL: SymbolStatic;

    /// Symbol at the Dex network
    const DEX_SYMBOL: SymbolStatic;
}

pub fn equal<C1, C2>() -> bool
where
    C1: 'static,
    C2: 'static,
{
    TypeId::of::<C1>() == TypeId::of::<C2>()
}

pub fn validate_member<C, G>() -> Result<()>
where
    C: Currency,
    G: Group,
{
    validate::<G>(C::TICKER)
}

pub fn validate<G>(ticker: &SymbolSlice) -> Result<()>
where
    G: Group,
{
    struct SupportedLeaseCurrency {}
    impl AnyVisitor for SupportedLeaseCurrency {
        type Error = Error;
        type Output = ();
        fn on<C>(self) -> Result<Self::Output>
        where
            C: Currency,
        {
            Ok(())
        }
    }
    Tickers.visit_any::<G, _>(ticker, SupportedLeaseCurrency {})
}

pub fn maybe_visit_any<M, C, V>(
    matcher: &M,
    symbol: &SymbolSlice,
    visitor: V,
) -> MaybeAnyVisitResult<V>
where
    M: Matcher + ?Sized,
    C: Currency,
    V: AnyVisitor,
{
    if matcher.match_::<C>(symbol) {
        Ok(visitor.on::<C>())
    } else {
        Err(visitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::Error,
        test::{SubGroup, SubGroupTestC1, SuperGroup, SuperGroupTestC1, SuperGroupTestC2},
        Currency, Tickers,
    };

    #[test]
    fn validate() {
        assert_eq!(
            Ok(()),
            super::validate::<SuperGroup>(SuperGroupTestC1::TICKER)
        );
        assert_eq!(
            Ok(()),
            super::validate::<SuperGroup>(SuperGroupTestC2::TICKER)
        );
        assert_eq!(
            Err(Error::not_in_currency_group::<_, Tickers, SuperGroup>(
                SubGroupTestC1::TICKER
            )),
            super::validate::<SuperGroup>(SubGroupTestC1::TICKER)
        );
        assert_eq!(Ok(()), super::validate::<SubGroup>(SubGroupTestC1::TICKER));
    }
}
