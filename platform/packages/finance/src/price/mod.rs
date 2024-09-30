use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    ops::{Add, AddAssign, Mul},
};

use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    coin::{Amount, Coin},
    error::{Error, Result},
    fraction::Fraction,
    fractionable::HigherRank,
    ratio::{Ratio, Rational},
};

pub mod base;
pub mod dto;

pub const fn total_of<C>(amount: Coin<C>) -> PriceBuilder<C> {
    PriceBuilder(amount)
}

pub struct PriceBuilder<C>(Coin<C>)
where
    C: 'static;

impl<C> PriceBuilder<C>
where
    C: 'static,
{
    pub fn is<QuoteC>(self, to: Coin<QuoteC>) -> Price<C, QuoteC>
    where
        QuoteC: 'static,
    {
        Price::new(self.0, to)
    }
}

type DoubleAmount = <Amount as HigherRank<Amount>>::Type;
type IntermediateAmount = <Amount as HigherRank<Amount>>::Intermediate;

/// Represents the price of a currency in a quote currency, ref: <https://en.wikipedia.org/wiki/Currency_pair>
///
/// The price is always kept in a canonical form of the underlying ratio. The simplifies equality and comparison operations.
/// For example, Price<EUR, USD> 1.15, generally represented as EURUSD or EUR/USD, means that one EUR is exchanged for 1.15 USD.
/// Both amounts a price is composed of should be non-zero.
///
/// Not designed to be used in public APIs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(bound(serialize = "", deserialize = ""))]
pub struct Price<C, QuoteC> {
    amount: Coin<C>,
    amount_quote: Coin<QuoteC>,
}

impl<C, QuoteC> Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    /// Constructor intended to be used when the preconditions have already been met,
    /// for example when converting from another Price family instance, e.g. PriceDTO
    #[track_caller]
    fn new(amount: Coin<C>, amount_quote: Coin<QuoteC>) -> Self {
        debug_assert_eq!(Ok(()), Self::precondition_check(amount, amount_quote));

        let res = Self::new_inner(amount, amount_quote);

        debug_assert_eq!(Ok(()), res.invariant_held());

        res
    }

    /// Contructor intended to be used with non-validated input,
    /// for example when deserializing from an user request
    #[track_caller]
    fn try_new(amount: Coin<C>, amount_quote: Coin<QuoteC>) -> Result<Self> {
        Self::precondition_check(amount, amount_quote)
            .map(|()| Self::new_inner(amount, amount_quote))
            .and_then(|may_price| may_price.invariant_held().map(|()| may_price))
    }

    fn new_inner(amount: Coin<C>, amount_quote: Coin<QuoteC>) -> Self {
        let (amount_normalized, amount_quote_normalized): (Coin<C>, Coin<QuoteC>) =
            amount.into_coprime_with(amount_quote);

        Self {
            amount: amount_normalized,
            amount_quote: amount_quote_normalized,
        }
    }

    /// Returns a new [`Price`] which represents identity mapped, one to one, currency pair.
    pub const fn identity() -> Self {
        Self {
            amount: Coin::new(1),
            amount_quote: Coin::new(1),
        }
    }

    /// Price(amount, amount_quote) * Ratio(nominator / denominator) = Price(amount * denominator, amount_quote * nominator)
    /// where the pairs (amount, nominator) and (amount_quote, denominator) are transformed into co-prime numbers.
    /// Please note that Price(amount, amount_quote) is like Ratio(amount_quote / amount).
    pub(crate) fn lossy_mul<R>(self, rhs: &R) -> Self
    where
        R: Ratio<Amount>,
    {
        let (amount_normalized, rhs_nominator_normalized) =
            self.amount.into_coprime_with(Coin::<C>::from(rhs.parts()));
        let (amount_quote_normalized, rhs_denominator_normalized) = self
            .amount_quote
            .into_coprime_with(Coin::<QuoteC>::from(rhs.total()));

        let double_amount =
            DoubleAmount::from(amount_normalized) * DoubleAmount::from(rhs_denominator_normalized);
        let double_amount_quote = DoubleAmount::from(amount_quote_normalized)
            * DoubleAmount::from(rhs_nominator_normalized);

        let extra_bits =
            Self::bits_above_max(double_amount).max(Self::bits_above_max(double_amount_quote));

        Price::new(
            Self::trim_down(double_amount, extra_bits).into(),
            Self::trim_down(double_amount_quote, extra_bits).into(),
        )
    }

    pub fn inv(self) -> Price<QuoteC, C> {
        Price {
            amount: self.amount_quote,
            amount_quote: self.amount,
        }
    }

    fn precondition_check(amount: Coin<C>, amount_quote: Coin<QuoteC>) -> Result<()> {
        Self::check(!amount.is_zero(), "The amount should not be zero").and(Self::check(
            !amount_quote.is_zero(),
            "The quote amount should not be zero",
        ))
    }

    fn invariant_held(&self) -> Result<()> {
        Self::precondition_check(self.amount, self.amount_quote).and(Self::check(
            Amount::from(self.amount) == Amount::from(self.amount_quote)
                || !currency::equal::<C, QuoteC>(),
            "The price should be equal to the identity if the currencies match",
        ))
    }

    fn check(invariant: bool, msg: &str) -> Result<()> {
        Error::broken_invariant_if::<Self>(!invariant, msg)
    }

    // TODO consider using 'checked_add' when adding prices to ensure that any potential overflow is safely handled.
    fn checked_add(self, rhs: Self) -> Option<Self> {
        // let a1 = a / gcd(a, c), and c1 = c / gcd(a, c), then
        // b / a + d / c = (b * c1 + d * a1) / (a1 * c1 * gcd(a, c))
        // taking into account that Price is like amount_quote/amount
        let (a1, c1) = self.amount.into_coprime_with(rhs.amount);
        debug_assert_eq!(0, Amount::from(self.amount) % Amount::from(a1));
        debug_assert_eq!(0, Amount::from(rhs.amount) % Amount::from(c1));
        let gcd: Amount = match self.amount.checked_div(a1.into()) {
            None => unreachable!("invariant on amount != 0 should have passed!"),
            Some(gcd) => gcd.into(),
        };
        debug_assert_eq!(Some(gcd.into()), rhs.amount.checked_div(c1.into()));

        let may_b_c1 = self.amount_quote.checked_mul(c1.into());
        let may_d_a1 = rhs.amount_quote.checked_mul(a1.into());
        let may_amount_quote = may_b_c1
            .zip(may_d_a1)
            .and_then(|(b_c1, d_a1)| b_c1.checked_add(d_a1));
        let may_amount = a1
            .checked_mul(c1.into())
            .and_then(|a1_c1| a1_c1.checked_mul(gcd));
        may_amount_quote
            .zip(may_amount)
            .map(|(amount_quote, amount)| Self::new(amount, amount_quote))
    }

    /// Add two prices rounding each of them to 1.10-18, simmilarly to
    /// the precision provided by cosmwasm::Decimal.
    ///
    /// TODO Implement a variable precision algorithm depending on the
    /// value of the prices. The rounding would be done by shifting to
    /// the right both amounts of the price with a bigger denominator
    /// until a * d + b * c and b * d do not overflow.
    fn lossy_add(self, rhs: Self) -> Option<Self> {
        let factor: Coin<C> = Coin::new(1_000_000_000_000_000_000); // 1*10^18

        total(factor, self)
            .and_then(|total_self| {
                total(factor, rhs).and_then(|total_rhs| total_self.checked_add(total_rhs))
            })
            .map(|factored_total| total_of(factor).is(factored_total))
    }

    #[track_caller]
    fn bits_above_max(double_amount: DoubleAmount) -> u32 {
        const BITS_MAX_AMOUNT: u32 = Amount::BITS;
        let higher_half: Amount = IntermediateAmount::try_from(double_amount >> BITS_MAX_AMOUNT)
            .expect("Bigger Amount Higher Rank Type than required!")
            .into();
        BITS_MAX_AMOUNT - higher_half.leading_zeros()
    }

    #[track_caller]
    fn trim_down(double_amount: DoubleAmount, bits: u32) -> Amount {
        debug_assert!(bits <= Amount::BITS);
        let amount: IntermediateAmount = (double_amount >> bits)
            .try_into()
            .expect("insufficient bits to trim");
        let res = amount.into();
        assert!(res > 0, "price overflow during multiplication");
        res
    }
}

impl<C, QuoteC> Clone for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, QuoteC> Copy for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
}

impl<C, QuoteC> Eq for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
}

impl<C, QuoteC> PartialEq for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.amount == other.amount && self.amount_quote == other.amount_quote
    }
}

impl<C, QuoteC> PartialOrd for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // a/b < c/d if and only if a * d < b * c
        // Please note that Price(amount, amount_quote) is like Ratio(amount_quote / amount).

        let a: DoubleAmount = self.amount_quote.into();
        let d: DoubleAmount = other.amount.into();

        let b: DoubleAmount = self.amount.into();
        let c: DoubleAmount = other.amount_quote.into();
        (a * d).partial_cmp(&(b * c))
    }
}

impl<C, QuoteC> Add<Price<C, QuoteC>> for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    type Output = Price<C, QuoteC>;

    fn add(self, rhs: Price<C, QuoteC>) -> Self::Output {
        self.checked_add(rhs)
            .or_else(|| self.lossy_add(rhs))
            .expect("should not observe huge prices")
    }
}

impl<C, QuoteC> AddAssign<Price<C, QuoteC>> for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
{
    #[track_caller]
    fn add_assign(&mut self, rhs: Price<C, QuoteC>) {
        *self = self.add(rhs);
    }
}

// TODO for completeness implement the Sub and SubAssign counterparts

impl<C, QuoteC, QuoteQuoteC> Mul<Price<QuoteC, QuoteQuoteC>> for Price<C, QuoteC>
where
    C: 'static,
    QuoteC: 'static,
    QuoteQuoteC: 'static,
{
    type Output = Price<C, QuoteQuoteC>;

    #[track_caller]
    fn mul(self, rhs: Price<QuoteC, QuoteQuoteC>) -> Self::Output {
        // Price(a, b) * Price(c, d) = Price(a, d) * Rational(b / c)
        // Please note that Price(amount, amount_quote) is like Ratio(amount_quote / amount).

        Self::Output::new(self.amount, rhs.amount_quote)
            .lossy_mul(&Rational::new(self.amount_quote, rhs.amount))
    }
}

impl<C, QuoteC> Display for Price<C, QuoteC> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}/{}", self.amount, self.amount_quote)
    }
}

/// Calculates the amount of given coins in another currency, referred here as `quote currency`
///
/// For example, total(10 EUR, 1.01 EURUSD) = 10.1 USD
pub fn total<C, QuoteC>(of: Coin<C>, price: Price<C, QuoteC>) -> Option<Coin<QuoteC>>
where
    C: 'static,
    QuoteC: 'static,
{
    let ratio_impl = Rational::new(of, price.amount);
    Fraction::<Coin<C>>::of(&ratio_impl, price.amount_quote)
}

#[cfg(test)]
mod test {
    use std::ops::{Add, AddAssign, Mul};

    use currency::test::{SubGroupTestC10, SuperGroupTestC1, SuperGroupTestC2};
    use sdk::cosmwasm_std::{Uint128, Uint256};

    use crate::{
        coin::{Amount, Coin as CoinT},
        price::{self, Price},
        ratio::Rational,
    };

    type QuoteQuoteCoin = CoinT<SubGroupTestC10>;
    type QuoteCoin = CoinT<SuperGroupTestC1>;
    type Coin = CoinT<SuperGroupTestC2>;

    #[test]
    fn new_c16n() {
        let amount = 13;
        let amount_quote = 15;
        let factor = 32;
        assert_eq!(
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote)),
            Price::new(
                Coin::new(amount * factor),
                QuoteCoin::new(amount_quote * factor)
            )
        );
    }

    #[test]
    fn eq() {
        let amount = 13;
        let amount_quote = 15;
        assert_ne!(
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote)),
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote + 1))
        );
        assert_ne!(
            Price::new(Coin::new(amount - 1), QuoteCoin::new(amount_quote)),
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote))
        );

        assert_eq!(
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote)),
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote))
        );

        assert_eq!(
            Price::new(QuoteCoin::new(amount_quote), Coin::new(amount)),
            Price::new(Coin::new(amount), QuoteCoin::new(amount_quote)).inv()
        );
    }

    #[test]
    fn ord() {
        ord_impl(13, 15);
    }

    #[test]
    fn ord_max() {
        ord_impl(Amount::MAX, 1);
        ord_impl(Amount::MAX, Amount::MAX - 2);
    }

    #[test]
    fn total() {
        let amount_quote = 647;
        let amount = 48;
        let price = price::total_of(amount.into()).is(amount_quote.into());
        let factor = 17;
        let coin_quote = QuoteCoin::new(amount_quote * factor);
        let coin = Coin::new(amount * factor);

        assert_eq!(coin_quote, super::total(coin, price).unwrap());
        assert_eq!(coin, super::total(coin_quote, price.inv()).unwrap());
    }

    #[test]
    fn total_rounding() {
        let amount_quote = 647;
        let amount = 48;
        let price = super::total_of(amount.into()).is(amount_quote.into());
        let coin_quote = QuoteCoin::new(633);

        // 47 * 647 / 48 -> 633.5208333333334
        let coin_in = Coin::new(47);
        assert_eq!(coin_quote, super::total(coin_in, price).unwrap());

        // 633 * 48 / 647 -> 46.9613601236476
        let coin_out = Coin::new(46);
        assert_eq!(coin_out, super::total(coin_quote, price.inv()).unwrap());
    }

    #[test]
    fn total_max() {
        total_max_impl(1, 1, Amount::MAX, Amount::MAX);
        total_max_impl(100, 100, Amount::MAX, Amount::MAX);
        total_max_impl(50, 100, Amount::MAX - 1, (Amount::MAX - 1) / 2);
    }

    #[test]
    fn total_overflow() {
        let price = price::total_of::<SuperGroupTestC2>(1.into())
            .is::<SuperGroupTestC1>((Amount::MAX / 2 + 1).into());
        assert!(super::total(2.into(), price).is_none());
    }

    #[test]
    fn add_no_round() {
        add_impl(c(1), q(2), c(5), q(10), c(1), q(4));
        add_impl(c(2), q(1), c(10), q(5), c(1), q(1));
        add_impl(c(2), q(3), c(10), q(14), c(10), q(29));
    }

    #[test]
    fn add_round() {
        add_impl(c(Amount::MAX), q(1), c(1), q(1), c(1), q(1));
    }

    #[test]
    fn lossy_add_no_round() {
        lossy_add_impl(c(1), q(2), c(5), q(10), c(1), q(4));
        lossy_add_impl(c(2), q(1), c(10), q(5), c(1), q(1));
        lossy_add_impl(c(2), q(3), c(10), q(14), c(10), q(29));
    }

    #[test]
    fn lossy_add_round() {
        // 1/3 + 2/7 = 13/21 that is 0.(619047)*...
        let amount_exp = 1_000_000_000_000_000_000;
        let quote_exp = 619_047_619_047_619_047;
        lossy_add_impl(c(3), q(1), c(7), q(2), c(amount_exp), q(quote_exp));
        lossy_add_impl(
            c(amount_exp),
            q(quote_exp),
            c(3),
            q(1),
            c(amount_exp),
            q(quote_exp + 333_333_333_333_333_333),
        );
        lossy_add_impl(
            c(amount_exp + 1),
            q(quote_exp),
            c(21),
            q(1),
            c(amount_exp / 5),
            q(133_333_333_333_333_333),
        );

        lossy_add_impl(c(amount_exp + 1), q(1), c(1), q(1), c(1), q(1));

        lossy_add_impl(c(Amount::MAX), q(1), c(1), q(1), c(1), q(1));
    }

    #[test]
    fn lossy_add_overflow() {
        // 2^128 / FACTOR (10^18) / 2^64 ~ 18.446744073709553
        let p1 = price::total_of(c(1)).is(q(u128::from(u64::MAX) * 19u128));
        let p2 = Price::identity();
        assert!(p1.lossy_add(p2).is_none());
    }

    #[test]
    fn lossy_mul_no_round() {
        lossy_mul_impl(c(1), q(2), q(2), qq(1), c(1), qq(1));
        lossy_mul_impl(c(2), q(3), q(18), qq(5), c(12), qq(5));
        lossy_mul_impl(c(7), q(3), q(11), qq(21), c(11), qq(9));
        lossy_mul_impl(c(7), q(3), q(11), qq(23), c(7 * 11), qq(3 * 23));

        let big_int = u128::MAX - 1;
        assert!(big_int % 3 != 0 && big_int % 11 != 0);
        lossy_mul_impl(c(big_int), q(3), q(11), qq(big_int), c(11), qq(3));

        assert_eq!(0, u128::MAX % 5);
        lossy_mul_impl(c(u128::MAX), q(2), q(3), qq(5), c(u128::MAX / 5 * 3), qq(2));
    }

    #[test]
    fn lossy_mul_few_shifts() {
        lossy_mul_shifts_impl(5, 1);
        lossy_mul_shifts_impl(5, 2);
        lossy_mul_shifts_impl(5, 7);
        lossy_mul_shifts_impl(5, 16);
        lossy_mul_shifts_impl(5, 63);
    }

    #[test]
    #[should_panic = "overflow"]
    fn lossy_mul_overflow() {
        const SHIFTS: u8 = 23;
        let a1 = u128::MAX - 1;
        let q1 = 7;
        let a2: Amount = 1 << SHIFTS;
        let q2 = a2 / q1 - 1; // the aim is q1 * q2 < a2

        assert!(a1 % q1 != 0);
        assert!(a1 % q2 != 0);
        assert!(a2 % q1 != 0);
        assert!(a2 % q2 != 0);

        assert!(shift_product(a1, a2, SHIFTS) == 0 || shift_product(q1, q2, SHIFTS) == 0);
        let price1 = price::total_of(c(a1)).is(q(q1));
        let price2 = price::total_of(q(a2)).is(qq(q2));
        _ = price1.mul(price2);
    }

    fn c(a: Amount) -> Coin {
        Coin::new(a)
    }

    fn q(a: Amount) -> QuoteCoin {
        QuoteCoin::new(a)
    }

    fn qq(a: Amount) -> QuoteQuoteCoin {
        QuoteQuoteCoin::new(a)
    }

    fn ord_impl(amount: Amount, amount_quote: Amount) {
        let price1 = Price::new(amount.into(), QuoteCoin::new(amount_quote));
        let price2 = Price::new(amount.into(), QuoteCoin::new(amount_quote + 1));
        assert!(price1 < price2);

        let total1 = super::total(Coin::new(amount), price1).unwrap();
        let total2 = super::total(Coin::new(amount), price2).unwrap();
        assert!(total1 < total2);
        assert_eq!(QuoteCoin::new(amount_quote), total1);
    }

    fn total_max_impl(
        amount: Amount,
        price_amount: Amount,
        price_amount_quote: Amount,
        expected: Amount,
    ) {
        let price = price::total_of(price_amount.into()).is(price_amount_quote.into());
        let expected = QuoteCoin::new(expected);
        let input = Coin::new(amount);

        assert_eq!(expected, super::total(input, price).unwrap());
        assert_eq!(input, super::total(expected, price.inv()).unwrap());
    }

    fn add_impl(
        amount1: Coin,
        quote1: QuoteCoin,
        amount2: Coin,
        quote2: QuoteCoin,
        amount_exp: Coin,
        quote_exp: QuoteCoin,
    ) {
        let price1 = price::total_of(amount1).is(quote1);
        let price2 = price::total_of(amount2).is(quote2);
        let exp = price::total_of(amount_exp).is(quote_exp);
        assert_eq!(exp, price1.add(price2));
        assert!({
            price1.checked_add(price2).map_or_else(
                || Some(exp) == price1.lossy_add(price2),
                |v| v == price1.add(price2),
            )
        });
        assert!(Some(exp) == price1.lossy_add(price2));
        assert!(exp >= price1);
        assert!(exp >= price2);

        let mut price3 = price1;
        price3.add_assign(price2);
        assert_eq!(exp, price3);
    }

    #[track_caller]
    fn lossy_add_impl(
        amount1: Coin,
        quote1: QuoteCoin,
        amount2: Coin,
        quote2: QuoteCoin,
        amount_exp: Coin,
        quote_exp: QuoteCoin,
    ) {
        let price1 = price::total_of(amount1).is(quote1);
        let price2 = price::total_of(amount2).is(quote2);
        let exp = price::total_of(amount_exp).is(quote_exp);
        assert_eq!(Some(exp), price1.lossy_add(price2));
        assert!(exp <= price1.add(price2));
    }

    fn shift_product<A1, A2>(a1: A1, a2: A2, shifts: u8) -> Amount
    where
        A1: Into<Uint256>,
        A2: Into<Uint256>,
    {
        Uint128::try_from((a1.into() * a2.into()) >> u32::from(shifts))
            .expect("Incorrect test setup")
            .into()
    }

    fn lossy_mul_impl(
        amount1: Coin,
        quote1: QuoteCoin,
        amount2: QuoteCoin,
        quote2: QuoteQuoteCoin,
        amount_exp: Coin,
        quote_exp: QuoteQuoteCoin,
    ) {
        let price1 = price::total_of(amount1).is(quote1);
        let price2 = price::total_of(amount2).is(quote2);
        let exp = price::total_of(amount_exp).is(quote_exp);
        assert_eq!(exp, price1.mul(price2));

        let price3 = price::total_of(amount1).is(quote2);
        let ratio = Rational::new(quote1, amount2);
        assert_eq!(exp, price3.lossy_mul(&ratio));
    }

    fn lossy_mul_shifts_impl(q1: Amount, shifts: u8) {
        let a1 = u128::MAX - 1;
        let a2: Amount = 1 << shifts;
        let q2 = a2 / q1 + 3; // the aim is q1 * q2 > a2

        assert!(a1 % q1 != 0);
        assert!(a1 % q2 != 0);
        assert!(a2 % q1 != 0);
        assert!(a2 % q2 != 0);
        assert_eq!(1, a2 >> shifts);

        let a_exp = shift_product(a1, a2, shifts);
        let q_exp = shift_product(q1, q2, shifts);
        lossy_mul_impl(c(a1), q(q1), q(a2), qq(q2), c(a_exp), qq(q_exp));
    }
}

#[cfg(test)]
mod test_invariant {
    use currency::{
        test::{SuperGroupTestC1, SuperGroupTestC2},
        Currency,
    };

    use crate::{coin::Coin, price::Price};

    #[test]
    #[should_panic = "zero"]
    fn base_zero() {
        new_invalid(
            Coin::<SuperGroupTestC1>::new(0),
            Coin::<SuperGroupTestC2>::new(5),
        );
    }

    #[test]
    #[should_panic = "zero"]
    fn quote_zero() {
        new_invalid(
            Coin::<SuperGroupTestC1>::new(10),
            Coin::<SuperGroupTestC2>::new(0),
        );
    }

    #[test]
    #[should_panic = "should be equal to the identity if the currencies match"]
    fn currencies_match() {
        new_invalid(
            Coin::<SuperGroupTestC2>::new(4),
            Coin::<SuperGroupTestC2>::new(5),
        );
    }

    #[test]
    fn currencies_match_ok() {
        assert_eq!(
            Price::identity(),
            Price::new(
                Coin::<SuperGroupTestC2>::new(4),
                Coin::<SuperGroupTestC2>::new(4)
            )
        );
    }

    fn new_invalid<C, QuoteC>(base: Coin<C>, quote: Coin<QuoteC>)
    where
        C: Currency,
        QuoteC: Currency,
    {
        let _p = Price::new(base, quote);
        #[cfg(not(debug_assertions))]
        {
            _p.invariant_held().expect("should have returned an error");
        }
    }
}
