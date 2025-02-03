use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    AnyVisitor, CurrencyDTO, CurrencyDef, Definition, Group, Matcher, MaybeAnyVisitResult,
    MaybePairsVisitorResult, MemberOf, PairsGroup, PairsVisitor,
};

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, JsonSchema,
)]
pub struct Stable(CurrencyDTO<PlatformGroup>);

impl CurrencyDef for Stable {
    type Group = PlatformGroup;

    fn definition() -> &'static Self {
        const INSTANCE: &Stable = &Stable(CurrencyDTO::new(&Definition::new(
            "STABLE",
            "N/A_N/A_N/A",
            "N/A_N/A_N/A",
            0,
        )));
        INSTANCE
    }

    fn dto(&self) -> &CurrencyDTO<Self::Group> {
        &self.0
    }
}
impl PairsGroup for Stable {
    type CommonGroup = PlatformGroup;

    fn maybe_visit<M, V>(_matcher: &M, _visitor: V) -> MaybePairsVisitorResult<V>
    where
        M: Matcher,
        V: PairsVisitor<Pivot = Self>,
    {
        unreachable!("The 'Stable' platform currency used in pairs resolution!")
    }
}

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, JsonSchema,
)]
/// A 'platform'-only 'dex-independent' representation of Nls.
///
/// Intended to be used *only* until the TODO below gets done, and *only* in dex-independent usecases:
/// - LP rewards
/// - Relayers' tips
pub struct Nls(CurrencyDTO<PlatformGroup>);

impl CurrencyDef for Nls {
    type Group = PlatformGroup;

    fn definition() -> &'static Self {
        const INSTANCE: &Nls = &Nls(CurrencyDTO::new(&Definition::new(
            "NLS",
            "unls",
            // TODO Define trait PlatformCurrency as a super trait of Currency and
            // merge NlsPlatform and Nls
            "N/A_N/A_N/A",
            6,
        )));

        INSTANCE
    }

    fn dto(&self) -> &CurrencyDTO<Self::Group> {
        &self.0
    }
}

impl PairsGroup for Nls {
    type CommonGroup = PlatformGroup;

    fn maybe_visit<M, V>(_matcher: &M, _visitor: V) -> MaybePairsVisitorResult<V>
    where
        M: Matcher,
        V: PairsVisitor,
    {
        unreachable!("The 'Nls' platform currency used in pairs resolution!")
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialEq, PartialOrd, Eq, Deserialize)]
pub struct PlatformGroup;
impl Group for PlatformGroup {
    const DESCR: &'static str = "platform currencies";
    type TopG = Self;

    fn currencies() -> impl Iterator<Item = CurrencyDTO<Self>> {
        [Nls::definition().dto(), Stable::definition().dto()]
            .into_iter()
            .copied()
            .map(CurrencyDTO::into_super_group)
    }

    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self, V>
    where
        M: Matcher,
        V: AnyVisitor<Self>,
    {
        Self::maybe_visit_member(matcher, visitor)
    }

    fn maybe_visit_member<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<Self::TopG, V>
    where
        M: Matcher,
        V: AnyVisitor<Self::TopG>,
    {
        crate::maybe_visit_member::<_, Nls, Self::TopG, _>(matcher, visitor)
            .or_else(|v| MaybeAnyVisitResult::Ok(v.on::<Stable>(Stable::definition().dto())))
        // we accept ANY currency to allow any stable@protocol to be a member
    }
}

impl MemberOf<Self> for PlatformGroup {}
