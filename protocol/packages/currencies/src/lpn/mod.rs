use serde::{Deserialize, Serialize};

use currency::{AnyVisitor, Group, Matcher, MaybeAnyVisitResult, SymbolSlice};
use sdk::schemars::{self, JsonSchema};

#[cfg(feature = "astroport")]
use self::astroport as impl_mod;
#[cfg(feature = "osmosis")]
use self::osmosis as impl_mod;
#[cfg(feature = "osmosis-osmosis-usdc_noble")]
use self::osmosis_osmosis_usdc_noble as impl_mod;

#[cfg(feature = "astroport")]
pub(crate) mod astroport;
#[cfg(feature = "osmosis")]
pub(crate) mod osmosis;
#[cfg(feature = "osmosis-osmosis-usdc_noble")]
pub(crate) mod osmosis_osmosis_usdc_noble;

#[derive(Clone, Debug, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Lpns {}

impl Group for Lpns {
    const DESCR: &'static str = "lpns";

    fn maybe_visit<M, V>(matcher: &M, symbol: &SymbolSlice, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher + ?Sized,
        V: AnyVisitor,
    {
        impl_mod::maybe_visit(matcher, symbol, visitor)
    }
}
