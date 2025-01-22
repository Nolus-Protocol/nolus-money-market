use serde::{Deserialize, Serialize};

use currency::{
    AnyVisitor, Matcher, MaybeAnyVisitResult, MaybePairsVisitorResult, MemberOf, PairsGroup,
    PairsVisitor,
};

use crate::payment::Group as PaymentGroup;

pub use self::impl_mod::definitions::Lpn;

#[cfg(not(feature = "testing"))]
mod impl_mod {
    include!(concat!(env!("OUT_DIR"), "/lpn.rs"));
}

#[cfg(feature = "testing")]
#[path = "testing.rs"]
mod impl_mod;

#[derive(Clone, Copy, Debug, Ord, PartialEq, PartialOrd, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum Group {}

impl currency::Group for Group {
    const DESCR: &'static str = "lpns";

    type TopG = PaymentGroup;

    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self, V>
    where
        M: Matcher,
        V: AnyVisitor<Self>,
    {
        use currency::maybe_visit_member as visit;

        visit::<_, Lpn, _, _>(matcher, visitor)
    }

    fn maybe_visit_member<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self::TopG, V>
    where
        M: Matcher,
        V: AnyVisitor<Self::TopG>,
    {
        use currency::maybe_visit_member as visit;

        visit::<_, Lpn, _, _>(matcher, visitor)
    }
}

impl MemberOf<Self> for Group {}

impl MemberOf<PaymentGroup> for Group {}

impl PairsGroup for Lpn {
    type CommonGroup = PaymentGroup;

    #[inline]
    fn maybe_visit<M, V>(_matcher: &M, visitor: V) -> MaybePairsVisitorResult<V>
    where
        M: Matcher,
        V: PairsVisitor<Pivot = Self>,
    {
        currency::visit_noone(visitor)
    }
}

#[cfg(test)]
mod test {
    use currency::CurrencyDef as _;

    use crate::{
        native::Nls,
        test_impl::{
            maybe_visit_on_bank_symbol_err, maybe_visit_on_bank_symbol_impl,
            maybe_visit_on_ticker_err, maybe_visit_on_ticker_impl,
        },
    };

    use super::{Group as Lpns, Lpn};

    #[test]
    fn maybe_visit_on_ticker() {
        maybe_visit_on_ticker_impl::<Lpn, Lpns>();
        maybe_visit_on_ticker_err::<Lpn, Lpns>(Lpn::bank());
        maybe_visit_on_ticker_err::<Lpn, Lpns>(Nls::ticker());
    }

    #[test]
    fn maybe_visit_on_bank_symbol() {
        maybe_visit_on_bank_symbol_impl::<Lpn, Lpns>();
        maybe_visit_on_bank_symbol_err::<Lpn, Lpns>(Lpn::ticker());
        maybe_visit_on_bank_symbol_err::<Lpn, Lpns>(Nls::bank());
    }
}
