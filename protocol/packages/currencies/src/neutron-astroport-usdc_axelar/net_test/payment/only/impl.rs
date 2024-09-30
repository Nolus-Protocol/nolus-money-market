use currency::{AnyVisitor, Matcher, MaybeAnyVisitResult};

pub(super) fn maybe_visit<M, V>(_matcher: &M, visitor: V) -> MaybeAnyVisitResult<V>
where
    M: Matcher,
    V: AnyVisitor,
{
    currency::visit_noone(visitor)
}
