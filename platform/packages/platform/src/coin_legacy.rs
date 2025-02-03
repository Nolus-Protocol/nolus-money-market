use std::{marker::PhantomData, result::Result as StdResult};

use currency::{
    AnyVisitor, AnyVisitorResult, BankSymbols, Currency, CurrencyDTO, CurrencyDef, CurrencyVisit,
    Group, GroupVisit, MemberOf, SingleVisitor, Symbol,
};
use finance::coin::{Amount, Coin, CoinDTO, WithCoin, WithCoinResult};
use sdk::cosmwasm_std::Coin as CosmWasmCoin;

use crate::{error::Error, result::Result};

pub(crate) fn from_cosmwasm<C>(coin: &CosmWasmCoin) -> Result<Coin<C>>
where
    C: CurrencyDef,
{
    from_cosmwasm_currency_not_definition::<C, C>(coin)
}

pub(crate) fn from_cosmwasm_currency_not_definition<CDef, COut>(
    coin: &CosmWasmCoin,
) -> Result<Coin<COut>>
where
    CDef: CurrencyDef,
    COut: Currency,
{
    BankSymbols::<CDef::Group>::visit::<CDef, _>(&coin.denom, CoinTransformer(coin, PhantomData))
}

/// Transform CW coin to Nolus coin and then return the [WithCoin] result
///
/// If seeking for the corresponding Nolus coin is not successfull an `Err(v)` is returned.
/// Otherwise, an `Ok(v.on(Nolus_coin))` is returned
pub(crate) fn from_cosmwasm_seek_any<VisitedG, V>(
    coin: &CosmWasmCoin,
    v: V,
) -> StdResult<WithCoinResult<VisitedG, V>, V>
where
    VisitedG: Group,
    V: WithCoin<VisitedG>,
{
    BankSymbols::maybe_visit_any(
        &coin.denom,
        CoinTransformerAny(coin, PhantomData::<VisitedG>, v),
    )
    .map_err(|transformer| transformer.2)
}

/// Transform CW coin to Nolus coin and then return the [WithCoin] result
///
/// If seeking for the corresponding Nolus coin is not successfull a [V::Error::from(Error::NotInCurrencyGroup)]  is returned.
/// Otherwise, an `v.on(Nolus_coin)` is returned
pub(crate) fn from_cosmwasm_any<VisitedG, V>(
    coin: &CosmWasmCoin,
    v: V,
) -> Result<WithCoinResult<VisitedG, V>>
where
    VisitedG: Group,
    V: WithCoin<VisitedG>,
{
    BankSymbols::visit_any(
        &coin.denom,
        CoinTransformerAny(coin, PhantomData::<VisitedG>, v),
    )
    .map_err(Into::into)
}

#[cfg(any(test, feature = "testing"))]
pub fn to_cosmwasm<C>(coin: Coin<C>) -> CosmWasmCoin
where
    C: CurrencyDef,
{
    to_cosmwasm_impl(coin)
}

#[cfg(any(test, feature = "testing"))]
pub fn to_cosmwasm_on_dex<C>(coin: Coin<C>) -> CosmWasmCoin
where
    C: CurrencyDef,
{
    use currency::DexSymbols;

    to_cosmwasm_on_network_impl::<C, DexSymbols<C::Group>>(coin)
}

pub(crate) fn to_cosmwasm_impl<C>(coin: Coin<C>) -> CosmWasmCoin
where
    C: CurrencyDef,
{
    to_cosmwasm_on_network_impl::<C, BankSymbols<C::Group>>(coin)
}

pub fn to_cosmwasm_on_network<S>(coin_dto: &CoinDTO<S::Group>) -> Result<CosmWasmCoin>
where
    S: Symbol,
{
    struct CoinTransformer<CM>(PhantomData<CM>);
    impl<S> WithCoin<S::Group> for CoinTransformer<S>
    where
        S: Symbol,
    {
        type Output = CosmWasmCoin;
        type Error = Error;

        fn on<C>(self, coin: Coin<C>) -> WithCoinResult<S::Group, Self>
        where
            C: CurrencyDef,
            C::Group: MemberOf<S::Group>,
        {
            Ok(to_cosmwasm_on_network_impl::<C, S>(coin))
        }
    }
    coin_dto.with_coin(CoinTransformer(PhantomData::<S>))
}

fn to_cosmwasm_on_network_impl<C, S>(coin: Coin<C>) -> CosmWasmCoin
where
    C: CurrencyDef,
    C::Group: MemberOf<S::Group>,
    S: Symbol,
{
    CosmWasmCoin::new(
        Amount::from(coin),
        S::symbol(C::definition().dto().definition()),
    )
}

struct CoinTransformer<'a, COut>(&'a CosmWasmCoin, PhantomData<COut>);

impl<CDef, COut> SingleVisitor<CDef> for CoinTransformer<'_, COut>
where
    CDef: CurrencyDef,
    COut: 'static,
{
    type Output = Coin<COut>;

    type Error = Error;

    fn on(self) -> Result<Self::Output> {
        Ok(from_cosmwasm_internal::<CDef, _>(self.0))
    }
}

struct CoinTransformerAny<'a, VisitedG, V>(&'a CosmWasmCoin, PhantomData<VisitedG>, V);

impl<VisitedG, V> AnyVisitor<VisitedG> for CoinTransformerAny<'_, VisitedG, V>
where
    VisitedG: Group,
    V: WithCoin<VisitedG>,
{
    type Output = V::Output;
    type Error = V::Error;

    fn on<C>(self, _def: &CurrencyDTO<C::Group>) -> AnyVisitorResult<VisitedG, Self>
    where
        C: CurrencyDef,
        C::Group: MemberOf<VisitedG> + MemberOf<VisitedG::TopG>,
    {
        self.2.on(from_cosmwasm_internal::<C, C>(self.0))
    }
}

fn from_cosmwasm_internal<CDef, COut>(coin: &CosmWasmCoin) -> Coin<COut>
where
    CDef: CurrencyDef,
    COut: 'static,
{
    debug_assert_eq!(
        CDef::definition().dto().definition().bank_symbol,
        coin.denom
    );
    assert!(currency::equal::<COut, CDef>());
    Amount::from(coin.amount).into()
}

#[cfg(test)]
mod test {
    use currency::{
        test::{SuperGroup, SuperGroupTestC1, SuperGroupTestC2},
        BankSymbols, CurrencyDef,
    };
    use finance::test::coin;
    use sdk::cosmwasm_std::Coin as CosmWasmCoin;

    use crate::error::Error;

    use super::{to_cosmwasm_impl, Coin};

    #[test]
    fn test_add() {
        let c1 = Coin::<SuperGroupTestC2>::new(10);
        let c2 = Coin::<SuperGroupTestC2>::new(20);
        let c12 = Coin::<SuperGroupTestC2>::new(30);
        assert_eq!(c12, c1 + c2);
    }

    #[test]
    fn from_cosmwasm() {
        let c1 = super::from_cosmwasm::<SuperGroupTestC2>(&CosmWasmCoin::new(
            12u8,
            SuperGroupTestC2::bank(),
        ));
        assert_eq!(Ok(Coin::<SuperGroupTestC2>::new(12)), c1);
    }
    #[test]
    fn from_cosmwasm_unexpected() {
        let c1 = super::from_cosmwasm::<SuperGroupTestC2>(&CosmWasmCoin::new(
            12u8,
            SuperGroupTestC1::bank(),
        ));

        assert_eq!(
            c1,
            Err(Error::Currency(
                currency::error::Error::unexpected_symbol::<_, BankSymbols::<SuperGroup>>(
                    SuperGroupTestC1::bank(),
                    SuperGroupTestC2::definition().dto().definition()
                )
            )),
        );

        let c2 = super::from_cosmwasm::<SuperGroupTestC1>(&CosmWasmCoin::new(
            12u8,
            SuperGroupTestC2::bank(),
        ));

        assert_eq!(
            c2,
            Err(Error::Currency(
                currency::error::Error::unexpected_symbol::<_, BankSymbols::<SuperGroup>>(
                    SuperGroupTestC2::bank(),
                    SuperGroupTestC1::definition().dto().definition()
                )
            )),
        );
    }

    #[test]
    fn from_cosmwasm_any_impl() {
        let amount = 42;
        type TheCurrency = SuperGroupTestC1;
        assert_eq!(
            Ok(Ok(true)),
            super::from_cosmwasm_seek_any(
                &CosmWasmCoin::new(amount, SuperGroupTestC1::bank()),
                coin::Expect(Coin::<TheCurrency>::from(amount))
            )
        );
    }

    #[test]
    fn from_cosmwasm_any_impl_err() {
        let amount = 42;
        type TheCurrency = SuperGroupTestC1;
        type AnotherCurrency = SuperGroupTestC2;
        assert_eq!(
            Ok(Ok(false)),
            super::from_cosmwasm_seek_any(
                &CosmWasmCoin::new(amount + 1, SuperGroupTestC1::bank()),
                coin::Expect(Coin::<TheCurrency>::from(amount))
            )
        );
        assert_eq!(
            Ok(Ok(false)),
            super::from_cosmwasm_seek_any(
                &CosmWasmCoin::new(amount, SuperGroupTestC1::bank()),
                coin::Expect(Coin::<AnotherCurrency>::from(amount))
            )
        );
        let with_coin = coin::Expect(Coin::<TheCurrency>::from(amount));
        assert_eq!(
            Err(with_coin.clone()),
            super::from_cosmwasm_seek_any(
                &CosmWasmCoin::new(amount, SuperGroupTestC1::dex()),
                with_coin
            )
        );
    }

    #[test]
    fn to_cosmwasm() {
        let amount = 326;
        assert_eq!(
            CosmWasmCoin::new(amount, SuperGroupTestC2::bank()),
            to_cosmwasm_impl(Coin::<SuperGroupTestC2>::new(amount))
        );
        assert_eq!(
            CosmWasmCoin::new(amount, SuperGroupTestC1::bank()),
            to_cosmwasm_impl(Coin::<SuperGroupTestC1>::new(amount))
        );
    }

    #[test]
    fn from_to_cosmwasm() {
        let c_nls = Coin::<SuperGroupTestC2>::new(24563);
        assert_eq!(Ok(c_nls), super::from_cosmwasm(&to_cosmwasm_impl(c_nls)));

        let c_usdc = Coin::<SuperGroupTestC1>::new(u128::MAX);
        assert_eq!(Ok(c_usdc), super::from_cosmwasm(&to_cosmwasm_impl(c_usdc)));
    }
}
