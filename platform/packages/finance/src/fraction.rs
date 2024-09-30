use std::fmt::Display;

use crate::fractionable::Fractionable;

pub trait Fraction<U> {
    fn of<A>(&self, whole: A) -> Option<A>
    where
        A: Fractionable<U> + Display + Clone;
}
