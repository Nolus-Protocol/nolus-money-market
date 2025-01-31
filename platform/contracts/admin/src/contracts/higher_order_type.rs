use std::marker::PhantomData;

use sdk::schemars::{self, JsonSchema};

pub trait HigherOrderType {
    type Of<T>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, JsonSchema)]
pub enum Identity {}

impl HigherOrderType for Identity {
    type Of<T> = T;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, JsonSchema)]
pub enum Option {}

impl HigherOrderType for Option {
    type Of<T> = core::option::Option<T>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct Compose<Outer, Inner> {
    _outer: PhantomData<Outer>,
    _inner: PhantomData<Inner>,
}

impl<Outer, Inner> HigherOrderType for Compose<Outer, Inner>
where
    Outer: HigherOrderType,
    Inner: HigherOrderType,
{
    type Of<T> = Outer::Of<Inner::Of<T>>;
}
