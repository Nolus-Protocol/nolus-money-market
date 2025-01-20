pub trait HigherOrderType {
    type Of<T>;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Identity;

impl HigherOrderType for Identity {
    type Of<T> = T;
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Option;

impl HigherOrderType for Option {
    type Of<T> = core::option::Option<T>;
}
