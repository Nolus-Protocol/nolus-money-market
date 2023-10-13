use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    currency::{AnyVisitor, Group, MaybeAnyVisitResult},
    lease::LeaseGroup,
    lpn::Lpns,
    native::Native,
    Matcher, SymbolSlice,
};

#[cfg(dex = "osmosis")]
pub(crate) mod osmosis;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PaymentGroup {}

impl Group for PaymentGroup {
    const DESCR: &'static str = "payment";

    fn maybe_visit<M, V>(matcher: &M, symbol: &SymbolSlice, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher + ?Sized,
        V: AnyVisitor,
    {
        LeaseGroup::maybe_visit(matcher, symbol, visitor)
            .or_else(|v| Lpns::maybe_visit(matcher, symbol, v))
            .or_else(|v| Native::maybe_visit(matcher, symbol, v))
    }
}
