use std::marker::PhantomData;

/// A right-open interval that may be left-unbound.
///
/// By default, the interval is ascending, i.e. its start <= end. It could be inverted, though,
/// to get its corresponding interval with start >= end.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
pub struct RightOpenRange<T, O> {
    start: Option<T>,
    end: T,
    _ordering: PhantomData<O>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
pub struct Ascending {}

#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug))]
pub struct Descending {}

impl<T> RightOpenRange<T, Ascending>
where
    T: Copy + Ord,
{
    pub fn up_to(below: T) -> Self {
        Self::new_ascending(None, below)
    }

    fn new_ascending(start: Option<T>, end: T) -> Self {
        let obj = Self {
            start,
            end,
            _ordering: PhantomData,
        };
        debug_assert!(obj.invariant());
        obj
    }

    pub fn may_above_or_equal(&self) -> Option<&T> {
        self.start.as_ref()
    }

    pub fn below(&self) -> &T {
        &self.end
    }

    pub fn contains(&self, t: &T) -> bool {
        self.may_above_or_equal().map_or(true, |start| start <= t) && t < &self.end
    }

    pub fn invert<R, MapFn>(self, mut map_fn: MapFn) -> RightOpenRange<R, Descending>
    where
        R: Ord,
        MapFn: FnMut(T) -> R,
    {
        RightOpenRange::new_descending(self.start.map(&mut map_fn), map_fn(self.end))
    }

    /// Relative complement of (-inf., `to`) in self
    ///
    /// Ref: https://en.wikipedia.org/wiki/Complement_(set_theory)#Relative_complement
    pub fn cut_to(self, to: T) -> Self {
        let start = Some(to.clamp(self.start.unwrap_or(to).min(self.end), self.end));
        Self::new_ascending(start, self.end)
    }

    /// Relative complement of [`from`, +inf.) in self
    ///
    /// Ref: https://en.wikipedia.org/wiki/Complement_(set_theory)#Relative_complement
    pub fn cut_from(self, from: T) -> Self {
        let end = from.clamp(self.start.unwrap_or(from).min(self.end), self.end);
        Self::new_ascending(self.start, end)
    }

    #[cfg(debug_assertions)]
    pub fn map<F, U>(self, map_fn: F) -> RightOpenRange<U, Ascending>
    where
        F: Fn(T) -> U,
        U: Copy + Ord,
    {
        RightOpenRange::new_ascending(self.start.map(&map_fn), map_fn(self.end))
    }

    fn invariant(&self) -> bool {
        self.may_above_or_equal()
            .map_or(true, |start| start <= &self.end)
    }
}

impl<T> RightOpenRange<T, Descending>
where
    T: Ord,
{
    pub fn from(above: T) -> Self {
        Self::new_descending(None, above)
    }

    fn new_descending(start: Option<T>, end: T) -> Self {
        let obj = Self {
            start,
            end,
            _ordering: PhantomData,
        };
        debug_assert!(obj.invariant());
        obj
    }

    pub fn may_below_or_equal(&self) -> Option<&T> {
        self.start.as_ref()
    }

    pub fn above(&self) -> &T {
        &self.end
    }

    fn invariant(&self) -> bool {
        self.may_below_or_equal()
            .map_or(true, |start| start >= &self.end)
    }
}

#[cfg(test)]
mod test {
    use currency::test::{SubGroupTestC6, SuperGroupTestC1};

    use crate::{
        coin::{Amount, Coin},
        price::{self, Price},
    };

    use super::RightOpenRange;

    #[test]
    fn contains_unbound() {
        let below = 20;
        let r = RightOpenRange::up_to(below);
        assert!(r.contains(&10));
        assert!(r.contains(&(below - 1)));
        assert!(!r.contains(&below));
    }

    #[test]
    fn contains_bound() {
        let above = 10;
        let below = 20;
        let r = RightOpenRange::up_to(below).cut_to(above);
        assert!(!r.contains(&(above - 1)));
        assert!(r.contains(&above));
        assert!(r.contains(&(below - 1)));
        assert!(!r.contains(&below));
    }

    #[test]
    fn invert() {
        let below = 20;
        let price_range = RightOpenRange::up_to(below).invert(amount_to_price);
        assert_eq!(None, price_range.may_below_or_equal());
        assert_eq!(&amount_to_price(below), price_range.above());
    }

    #[test]
    fn cut_from_unbound() {
        let below = 20;
        let cut_from = 10;
        let range = RightOpenRange::up_to(below);
        {
            let range_cut = range.cut_from(cut_from);
            assert_eq!(RightOpenRange::up_to(cut_from), range_cut);
            assert_eq!(None, range_cut.may_above_or_equal());
            assert_eq!(&cut_from, range_cut.below());
        }

        assert_eq!(range, range.cut_from(below));
        assert_eq!(range, range.cut_from(below + cut_from));
    }

    #[test]
    fn cut_from_bound() {
        let above = 2;
        let below = 20;
        let cut_from = 10;
        let range = RightOpenRange::up_to(below).cut_to(above);
        {
            let range_cut = range.cut_from(cut_from);
            assert_eq!(RightOpenRange::up_to(cut_from).cut_to(above), range_cut);
            assert_eq!(Some(above).as_ref(), range_cut.may_above_or_equal());
            assert_eq!(&cut_from, range_cut.below());
        }

        assert_eq!(range, range.cut_from(below));
        assert_eq!(range, range.cut_from(below + cut_from));
    }

    #[test]
    fn cut_to_unbound() {
        let below = 20;
        let cut_to = 10;
        let range = RightOpenRange::up_to(below);
        {
            let range_cut = range.cut_to(cut_to);
            assert_eq!(RightOpenRange::up_to(below).cut_to(cut_to), range_cut);
            assert_eq!(Some(cut_to).as_ref(), range_cut.may_above_or_equal());
            assert_eq!(&below, range_cut.below());
        }

        {
            let range_cut = range.cut_to(below);
            assert_eq!(Some(below).as_ref(), range_cut.may_above_or_equal());
            assert_eq!(&below, range_cut.below());
        }

        {
            let range_cut = range.cut_to(below + cut_to);
            assert_eq!(Some(below).as_ref(), range_cut.may_above_or_equal());
            assert_eq!(&below, range_cut.below());
        }
    }

    fn amount_to_price(amount: Amount) -> Price<SuperGroupTestC1, SubGroupTestC6> {
        price::total_of(Coin::from(amount)).is(Coin::from(3u128))
    }
}
