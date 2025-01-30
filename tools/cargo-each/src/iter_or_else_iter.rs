pub(crate) struct IterOrElseIter<Iter, AltIter>
where
    Iter: IntoIterator,
    AltIter: IntoIterator<Item = Iter::Item>,
{
    iter: Option<Iter::IntoIter>,
    alt_iter: Option<AltIter::IntoIter>,
}

impl<Iter, AltIter> IterOrElseIter<Iter, AltIter>
where
    Iter: IntoIterator,
    AltIter: IntoIterator<Item = Iter::Item>,
{
    pub fn new(iter: Iter, alt_iter: AltIter) -> Self {
        Self {
            iter: Some(iter.into_iter()),
            alt_iter: Some(alt_iter.into_iter()),
        }
    }
}

impl<Iter, AltIter> Iterator for IterOrElseIter<Iter, AltIter>
where
    Iter: IntoIterator,
    AltIter: IntoIterator<Item = Iter::Item>,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        #[inline]
        fn next_and_clear_other<Iter, Other>(
            iter: &mut Option<Iter>,
            other: &mut Option<Other>,
        ) -> Option<Iter::Item>
        where
            Iter: Iterator,
        {
            iter.as_mut().and_then(Iterator::next).inspect(|_| {
                *other = None;
            })
        }

        next_and_clear_other(&mut self.iter, &mut self.alt_iter)
            .or_else(|| next_and_clear_other(&mut self.alt_iter, &mut self.iter))
    }
}

#[test]
fn test_neither() {
    assert_eq!(IterOrElseIter::new(None::<()>, None).next(), None);
}

#[test]
fn test_primary_only() {
    let mut iter = IterOrElseIter::new([1_u8, 2], None);

    assert_eq!(iter.next(), Some(1));

    assert_eq!(iter.next(), Some(2));

    assert_eq!(iter.next(), None);
}

#[test]
fn test_secondary_only() {
    let mut iter = IterOrElseIter::new(None, [3_u8, 4, 5]);

    assert_eq!(iter.next(), Some(3));

    assert_eq!(iter.next(), Some(4));

    assert_eq!(iter.next(), Some(5));

    assert_eq!(iter.next(), None);
}

#[test]
fn test_both() {
    let mut iter = IterOrElseIter::new([1_u8, 2], [3, 4, 5]);

    assert_eq!(iter.next(), Some(1));

    assert_eq!(iter.next(), Some(2));

    assert_eq!(iter.next(), None);
}
