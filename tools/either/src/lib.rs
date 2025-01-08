use std::io;

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    #[inline]
    pub fn map_left<F, NewL>(self, f: F) -> Either<NewL, R>
    where
        F: FnOnce(L) -> NewL,
    {
        match self {
            Either::Left(left) => Either::Left(f(left)),
            Either::Right(right) => Either::Right(right),
        }
    }
}

impl<L, R> Iterator for Either<L, R>
where
    L: Iterator,
    R: Iterator<Item = L::Item>,
{
    type Item = L::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Either::Left(inner) => inner.next(),
            Either::Right(inner) => inner.next(),
        }
    }
}

impl<L, R> io::Write for Either<L, R>
where
    L: io::Write,
    R: io::Write,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Either::Left(inner) => inner.write(buf),
            Either::Right(inner) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Either::Left(inner) => inner.flush(),
            Either::Right(inner) => inner.flush(),
        }
    }
}
