use serde::{Deserialize, Serialize};

use currency::{AnyVisitor, Group, Matcher, MaybeAnyVisitResult, SymbolSlice};
use sdk::schemars::{self, JsonSchema};

#[cfg(feature = "astroport")]
use self::astroport as impl_mod;
#[cfg(feature = "osmosis")]
use self::osmosis as impl_mod;

#[cfg(feature = "astroport")]
pub(crate) mod astroport;
#[cfg(feature = "osmosis")]
pub(crate) mod osmosis;

#[derive(Clone, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PaymentOnlyGroup {}

impl Group for PaymentOnlyGroup {
    const DESCR: &'static str = "payment only";

    fn maybe_visit<M, V>(matcher: &M, symbol: &SymbolSlice, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher + ?Sized,
        V: AnyVisitor,
    {
        impl_mod::maybe_visit(matcher, symbol, visitor)
    }
}
