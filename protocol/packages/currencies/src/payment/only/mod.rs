use serde::{Deserialize, Serialize};

use currency::{AnyVisitor, Matcher, MaybeAnyVisitResult, MemberOf};

use super::Group as PaymentGroup;

#[cfg(not(feature = "testing"))]
#[allow(unused_imports)]
pub(crate) use self::impl_mod::definitions::*;

#[cfg(not(feature = "testing"))]
mod impl_mod {
    include!(concat!(env!("OUT_DIR"), "/payment_only.rs"));
}

#[cfg(feature = "testing")]
#[path = "testing.rs"]
mod impl_mod;

#[derive(
    Clone, Copy, Debug, Ord, PartialEq, PartialOrd, Eq, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum Group {}

impl currency::Group for Group {
    const DESCR: &'static str = "payment only";

    type TopG = PaymentGroup;

    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self, V>
    where
        M: Matcher,
        V: AnyVisitor<Self>,
    {
        impl_mod::maybe_visit(matcher, visitor)
    }

    fn maybe_visit_member<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self::TopG, V>
    where
        M: Matcher,
        V: AnyVisitor<Self::TopG>,
    {
        impl_mod::maybe_visit(matcher, visitor)
    }
}

impl MemberOf<Self> for Group {}

impl MemberOf<PaymentGroup> for Group {}
