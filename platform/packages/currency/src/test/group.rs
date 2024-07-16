use serde::Deserialize;

use crate::{group::MemberOf, AnyVisitor, Group, Matcher, MaybeAnyVisitResult};

pub type SuperGroupTestC1 = impl_::TestC1;
pub type SuperGroupTestC2 = impl_::TestC2;
pub type SuperGroupTestC3 = impl_::TestC3;
pub type SuperGroupTestC4 = impl_::TestC4;
pub type SuperGroupTestC5 = impl_::TestC5;
pub type SuperGroupTestC6 = impl_::TestC6;

pub type SubGroupTestC1 = impl_::TestC10;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct SuperGroup {}
impl Group for SuperGroup {
    const DESCR: &'static str = "super_group";

    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher<Group = Self>,
        V: AnyVisitor<VisitedG = Self>,
    {
        crate::maybe_visit_any::<_, SuperGroupTestC1, _>(matcher, visitor)
            .or_else(|visitor| crate::maybe_visit_any::<_, SuperGroupTestC2, _>(matcher, visitor))
            .or_else(|visitor| crate::maybe_visit_any::<_, SuperGroupTestC3, _>(matcher, visitor))
            .or_else(|visitor| crate::maybe_visit_any::<_, SuperGroupTestC4, _>(matcher, visitor))
            .or_else(|visitor| crate::maybe_visit_any::<_, SuperGroupTestC5, _>(matcher, visitor))
            .or_else(|visitor| crate::maybe_visit_any::<_, SuperGroupTestC6, _>(matcher, visitor))
            .or_else(|visitor| SubGroup::maybe_visit_member(matcher, visitor))
    }

    fn maybe_visit_member<M, V>(_matcher: &M, _visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: ?Sized,
        V: AnyVisitor,
    {
        unreachable!()
    }
}
impl MemberOf<Self> for SuperGroup {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct SubGroup {}
impl Group for SubGroup {
    const DESCR: &'static str = "sub_group";

    fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher<Group = Self>,
        V: AnyVisitor<VisitedG = Self>,
    {
        Self::maybe_visit_member(matcher, visitor)
    }

    fn maybe_visit_member<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<V>
    where
        M: Matcher,
        V: AnyVisitor,
        Self: MemberOf<V::VisitedG> + MemberOf<M::Group>,
    {
        crate::maybe_visit_any::<_, SubGroupTestC1, _>(matcher, visitor)
    }
}
impl MemberOf<Self> for SubGroup {}
impl MemberOf<SuperGroup> for SubGroup {}

mod impl_ {
    use serde::{Deserialize, Serialize};

    use crate::{Currency, SymbolStatic};

    use super::{SubGroup, SuperGroup};

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC1;

    impl Currency for TestC1 {
        type Group = SuperGroup;

        const TICKER: SymbolStatic = "ticker#1";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#1";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#1";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC2;

    impl Currency for TestC2 {
        type Group = SuperGroup;

        const TICKER: SymbolStatic = "ticker#2";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#2";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#2";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC3;

    impl Currency for TestC3 {
        type Group = SuperGroup;

        const TICKER: SymbolStatic = "ticker#3";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#3";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#3";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC4;

    impl Currency for TestC4 {
        type Group = SuperGroup;

        const TICKER: SymbolStatic = "ticker#4";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#4";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#4";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC5;

    impl Currency for TestC5 {
        type Group = SuperGroup;

        const TICKER: SymbolStatic = "ticker#5";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#5";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#5";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC6;

    impl Currency for TestC6 {
        type Group = SubGroup;

        const TICKER: SymbolStatic = "ticker#6";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#6";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#6";

        const DECIMAL_DIGITS: u8 = 0;
    }

    #[derive(
        Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize,
    )]
    pub struct TestC10;
    impl Currency for TestC10 {
        type Group = SubGroup;

        const TICKER: SymbolStatic = "ticker#10";

        const BANK_SYMBOL: SymbolStatic = "ibc/bank_ticker#10";

        const DEX_SYMBOL: SymbolStatic = "ibc/dex_ticker#10";

        const DECIMAL_DIGITS: u8 = 0;
    }
}
