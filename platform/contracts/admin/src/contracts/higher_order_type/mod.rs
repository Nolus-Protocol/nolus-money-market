use std::marker::PhantomData;

#[cfg(feature = "contract")]
mod impl_mod;

pub trait FirstOrderType<DerivedFrom>
where
    Self: Sized,
    DerivedFrom: HigherOrderType<Of<Self::Unit> = Self>,
{
    type Unit;
}

pub trait HigherOrderType
where
    Self: Sized,
{
    type Of<Unit>: FirstOrderType<Self, Unit = Unit>;
}

#[cfg(feature = "contract")]
pub(super) trait TryForEach
where
    Self: HigherOrderType,
{
    fn try_for_each<Unit, F, Err>(this: Self::Of<Unit>, f: F) -> Result<(), Err>
    where
        F: FnMut(Unit) -> Result<(), Err>;
}

#[cfg(feature = "contract")]
pub(super) trait TryForEachPair
where
    Self: TryForEach + Zip,
{
    fn try_for_each_pair<LeftUnit, RightUnit, F, Err>(
        left: Self::Of<LeftUnit>,
        right: Self::Of<RightUnit>,
        mut f: F,
    ) -> Result<(), Err>
    where
        F: FnMut(LeftUnit, RightUnit) -> Result<(), Err>,
    {
        Self::try_for_each(Self::zip(left, right), |(left, right)| f(left, right))
    }
}

#[cfg(feature = "contract")]
impl<T> TryForEachPair for T where T: TryForEach + Zip {}

#[cfg(feature = "contract")]
pub(super) trait Map
where
    Self: HigherOrderType,
{
    fn map<Unit, F, MappedUnit>(this: Self::Of<Unit>, f: F) -> Self::Of<MappedUnit>
    where
        F: FnMut(Unit) -> MappedUnit;
}

#[cfg(feature = "contract")]
pub(super) trait MapAsRef
where
    Self: HigherOrderType,
{
    fn map_as_ref<Unit>(this: &Self::Of<Unit>) -> Self::Of<&Unit>;
}

#[cfg(feature = "contract")]
pub(super) trait Zip
where
    Self: HigherOrderType,
{
    fn zip<LeftUnit, RightUnit>(
        left: Self::Of<LeftUnit>,
        right: Self::Of<RightUnit>,
    ) -> Self::Of<(LeftUnit, RightUnit)>;
}

impl<T> FirstOrderType<Identity> for T {
    type Unit = Self;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Identity {}

impl HigherOrderType for Identity {
    type Of<Unit> = Unit;
}

impl<Left, Right> FirstOrderType<HigherOrderTuple<false, Left>> for (Left, Right) {
    type Unit = Right;
}

impl<Left, Right> FirstOrderType<HigherOrderTuple<true, Right>> for (Left, Right) {
    type Unit = Left;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HigherOrderTuple<const BOUND_RIGHT: bool, Bound> {
    _bound: PhantomData<Bound>,
}

impl<Left> HigherOrderType for HigherOrderTuple<false, Left> {
    type Of<Right> = (Left, Right);
}

impl<Right> HigherOrderType for HigherOrderTuple<true, Right> {
    type Of<Left> = (Left, Right);
}

impl<T> FirstOrderType<Option> for core::option::Option<T> {
    type Unit = T;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Option {}

impl HigherOrderType for Option {
    type Of<Unit> = core::option::Option<Unit>;
}

impl<T, Outer, Inner> FirstOrderType<Compose<Outer, Inner>> for T
where
    Self: FirstOrderType<Outer, Unit: FirstOrderType<Inner>>,
    Outer: HigherOrderType<Of<<Self as FirstOrderType<Outer>>::Unit> = Self>,
    Inner: HigherOrderType<
        Of<<<Self as FirstOrderType<Outer>>::Unit as FirstOrderType<Inner>>::Unit> = <Self as FirstOrderType<Outer>>::Unit,
    >,
{
    type Unit = <<Self as FirstOrderType<Outer>>::Unit as FirstOrderType<Inner>>::Unit;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compose<Outer, Inner> {
    _outer: PhantomData<Outer>,
    _inner: PhantomData<Inner>,
}

impl<Outer, Inner> HigherOrderType for Compose<Outer, Inner>
where
    Outer: HigherOrderType,
    Inner: HigherOrderType,
{
    type Of<Unit> = Outer::Of<Inner::Of<Unit>>;
}
