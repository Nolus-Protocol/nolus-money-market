use std::fmt::Debug;

use crate::{CurrencyDTO, CurrencyDef};

use super::{matcher::Matcher, AnyVisitor, AnyVisitorResult};

pub trait Group
where
    Self: Copy + Clone + Debug + Ord + PartialEq + MemberOf<Self>,
    Self: MemberOf<Self::TopG>,
{
    const DESCR: &'static str;
    type TopG: Group<TopG = Self::TopG>;

    /// All direct or indirect member currencies of this group
    fn currencies() -> impl Iterator<Item = CurrencyDTO<Self>>;

    // TODO it seems this could be taken out from here into a simple algo-function
    // Visit this group directly by a visitor
    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self, V>
    where
        M: Matcher,
        V: AnyVisitor<Self>;

    // TODO it seems this could be taken out from here into a simple algo-function
    // Visit this group since it is a member, or a sub-group, of another that is being visited
    fn maybe_visit_member<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self::TopG, V>
    where
        M: Matcher,
        V: AnyVisitor<Self::TopG>;
}

pub type MaybeAnyVisitResult<VisitedG, V> = Result<AnyVisitorResult<VisitedG, V>, V>;

pub trait MemberOf<G>
where
    G: Group,
{
}

impl<G, C> MemberOf<G> for C
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
{
}
