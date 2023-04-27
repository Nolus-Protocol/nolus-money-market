use std::{error::Error as StdError, result::Result as StdResult};

use serde::{de::DeserializeOwned, Serialize};

use finance::{
    coin::{Amount as CoinAmount, Coin, WithCoin, WithCoinResult},
    currency::{AnyVisitor, AnyVisitorResult, Currency, Group},
};
use sdk::cosmwasm_std::{Addr, BankMsg, Coin as CwCoin, QuerierWrapper};

use crate::{
    batch::Batch,
    coin_legacy::{
        from_cosmwasm_any_impl, from_cosmwasm_impl, maybe_from_cosmwasm_any_impl, to_cosmwasm_impl,
    },
    error::{Error, Result},
};

pub type BalancesResult<Cmd> = StdResult<Option<WithCoinResult<Cmd>>, Error>;

pub trait BankAccountView {
    fn balance<C>(&self) -> Result<Coin<C>>
    where
        C: Currency;

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<Cmd>
    where
        G: Group,
        Cmd: WithCoin,
        Cmd::Output: Aggregate;
}

pub trait BankAccount
where
    Self: BankAccountView + Into<Batch>,
{
    fn send<C>(&mut self, amount: Coin<C>, to: &Addr)
    where
        C: Currency;
}

pub trait FixedAddressSender
where
    Self: Into<Batch>,
{
    fn send<C>(&mut self, amount: Coin<C>)
    where
        C: Currency;
}

/// Ensure a single coin of the specified currency is received by a contract and return it
pub fn received_one<C>(cw_amount: Vec<CwCoin>) -> Result<Coin<C>>
where
    C: Currency,
{
    received_one_impl(
        cw_amount,
        Error::no_funds::<C>,
        Error::unexpected_funds::<C>,
    )
    .and_then(from_cosmwasm_impl)
}

/// Run a command on the first coin of the specified group
pub fn may_received<G, V>(cw_amount: Vec<CwCoin>, mut cmd: V) -> Option<WithCoinResult<V>>
where
    V: WithCoin,
    G: Group,
{
    let mut may_res = None;
    for coin in cw_amount {
        cmd = match from_cosmwasm_any_impl::<G, _>(coin, cmd) {
            Ok(res) => {
                may_res = Some(res);
                break;
            }
            Err(cmd) => cmd,
        }
    }
    may_res
}

struct CoinVisitor<'r, Cmd> {
    amount: CoinAmount,
    cmd: &'r Cmd,
}

impl<'r, Cmd> AnyVisitor for CoinVisitor<'r, Cmd>
where
    Cmd: WithCoin,
    Cmd::Output: Aggregate,
{
    type Output = Cmd::Output;
    type Error = Cmd::Error;

    fn on<C>(self) -> AnyVisitorResult<Self>
    where
        C: Currency + Serialize + DeserializeOwned,
    {
        self.cmd.on(Coin::<C>::new(self.amount))
    }
}

pub struct BankView<'a> {
    account: &'a Addr,
    querier: &'a QuerierWrapper<'a>,
}

impl<'a> BankView<'a> {
    fn account(account: &'a Addr, querier: &'a QuerierWrapper<'a>) -> Self {
        Self { account, querier }
    }
}

impl<'a> BankAccountView for BankView<'a> {
    fn balance<C>(&self) -> Result<Coin<C>>
    where
        C: Currency,
    {
        let coin = self.querier.query_balance(self.account, C::BANK_SYMBOL)?;
        from_cosmwasm_impl(coin)
    }

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<Cmd>
    where
        G: Group,
        Cmd: WithCoin,
        Cmd::Output: Aggregate,
    {
        self.querier
            .query_all_balances(self.account)
            .map(|cw_coins| {
                cw_coins
                    .into_iter()
                    .filter_map(|cw_coin| maybe_from_cosmwasm_any_impl::<G, _>(cw_coin, &cmd))
                    .reduce_results(Aggregate::aggregate)
                    .transpose()
            })
            .map_err(Into::into)
    }
}

pub struct BankStub<View>
where
    View: BankAccountView,
{
    view: View,
    batch: Batch,
}

impl<View> BankStub<View>
where
    View: BankAccountView,
{
    pub fn new(view: View) -> Self {
        Self {
            view,
            batch: Batch::default(),
        }
    }
}

pub fn account<'a>(account: &'a Addr, querier: &'a QuerierWrapper<'a>) -> BankStub<BankView<'a>> {
    BankStub::new(BankView::account(account, querier))
}

pub fn balance<'a, C>(account: &'a Addr, querier: &'a QuerierWrapper<'a>) -> Result<Coin<C>>
where
    C: Currency,
{
    BankView { account, querier }.balance()
}

impl<View> BankAccountView for BankStub<View>
where
    View: BankAccountView,
{
    fn balance<C>(&self) -> Result<Coin<C>>
    where
        C: Currency,
    {
        self.view.balance()
    }

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<Cmd>
    where
        G: Group,
        Cmd: WithCoin,
        Cmd::Output: Aggregate,
    {
        self.view.balances::<G, Cmd>(cmd)
    }
}

impl<View> BankAccount for BankStub<View>
where
    Self: BankAccountView + Into<Batch>,
    View: BankAccountView,
{
    fn send<C>(&mut self, amount: Coin<C>, to: &Addr)
    where
        C: Currency,
    {
        debug_assert!(!amount.is_zero());
        self.batch.schedule_execute_no_reply(BankMsg::Send {
            to_address: to.into(),
            amount: vec![to_cosmwasm_impl(amount)],
        });
    }
}

impl<View> From<BankStub<View>> for Batch
where
    View: BankAccountView,
{
    fn from(stub: BankStub<View>) -> Self {
        stub.batch
    }
}

fn received_one_impl<NoFundsErr, UnexpFundsErr>(
    cw_amount: Vec<CwCoin>,
    no_funds_err: NoFundsErr,
    unexp_funds_err: UnexpFundsErr,
) -> Result<CwCoin>
where
    NoFundsErr: FnOnce() -> Error,
    UnexpFundsErr: FnOnce() -> Error,
{
    match cw_amount.len() {
        0 => Err(no_funds_err()),
        1 => {
            let first = cw_amount
                .into_iter()
                .next()
                .expect("there is at least a coin");
            Ok(first)
        }
        _ => Err(unexp_funds_err()),
    }
}

pub struct LazySenderStub {
    receiver: Addr,
    amounts: Vec<CwCoin>,
}

impl LazySenderStub {
    pub fn new(receiver: Addr) -> Self {
        Self {
            receiver,
            amounts: Vec::new(),
        }
    }
}

impl FixedAddressSender for LazySenderStub
where
    Self: Into<Batch>,
{
    fn send<C>(&mut self, amount: Coin<C>)
    where
        C: Currency,
    {
        debug_assert!(!amount.is_zero());

        if amount.is_zero() {
            return;
        }

        self.amounts.push(to_cosmwasm_impl(amount));
    }
}

impl From<LazySenderStub> for Batch {
    fn from(stub: LazySenderStub) -> Self {
        let mut batch = Batch::default();

        if !stub.amounts.is_empty() {
            batch.schedule_execute_no_reply(BankMsg::Send {
                to_address: stub.receiver.to_string(),
                amount: stub.amounts,
            });
        }

        batch
    }
}

pub trait Aggregate {
    fn aggregate(self, other: Self) -> Self
    where
        Self: Sized;
}

impl Aggregate for () {
    fn aggregate(self, _: Self) -> Self {}
}

impl Aggregate for Batch {
    fn aggregate(self, other: Self) -> Self {
        self.merge(other)
    }
}

impl<T> Aggregate for Vec<T> {
    fn aggregate(mut self, mut other: Self) -> Self {
        self.append(&mut other);

        self
    }
}

trait ReduceResults
where
    Self: Iterator<Item = StdResult<Self::InnerItem, Self::Error>>,
{
    type InnerItem;
    type Error: StdError;

    fn reduce_results<F>(&mut self, f: F) -> StdResult<Option<Self::InnerItem>, Self::Error>
    where
        F: FnMut(Self::InnerItem, Self::InnerItem) -> Self::InnerItem;
}

impl<I, T, E> ReduceResults for I
where
    I: Iterator<Item = StdResult<T, E>>,
    E: StdError,
{
    type InnerItem = T;
    type Error = E;

    fn reduce_results<F>(&mut self, mut f: F) -> StdResult<Option<T>, E>
    where
        F: FnMut(T, T) -> T,
    {
        Ok(if let Some(mut last) = self.next().transpose()? {
            for item in self {
                last = f(last, item?);
            }

            Some(last)
        } else {
            None
        })
    }
}

#[cfg(test)]
mod test {
    use currency::{
        lease::Atom,
        native::{Native, Nls},
        payment::PaymentGroup,
    };
    use finance::{
        coin::{Amount, Coin, WithCoin, WithCoinResult},
        currency::{Currency, Group, SymbolStatic},
        test::{
            coin::Expect,
            currency::{Dai, TestCurrencies, Usdc},
        },
    };
    use sdk::{
        cosmwasm_std::{coin as cw_coin, Addr, Coin as CwCoin, Empty, QuerierWrapper},
        cw_multi_test::BasicApp,
    };

    use crate::{
        bank::{BankAccountView, BankView},
        coin_legacy,
        error::Error,
    };

    use super::may_received;

    type TheCurrency = Usdc;
    type ExtraCurrency = Dai;

    const AMOUNT: Amount = 42;

    #[test]
    fn may_received_no_input() {
        assert_eq!(
            None,
            may_received::<TestCurrencies, _>(vec![], Expect(Coin::<TheCurrency>::from(AMOUNT)))
        );
    }

    #[test]
    fn may_received_not_in_group() {
        let coin = Coin::<ExtraCurrency>::new(AMOUNT);
        let in_coin_1 = coin_legacy::to_cosmwasm(coin);

        #[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
        struct MyNiceCurrency {}
        impl Currency for MyNiceCurrency {
            const BANK_SYMBOL: SymbolStatic = "wdd";
            const DEX_SYMBOL: SymbolStatic = "dex3rdf";
            const TICKER: SymbolStatic = "ticedc";
        }
        let in_coin_2 = coin_legacy::to_cosmwasm(Coin::<MyNiceCurrency>::new(AMOUNT));

        assert_eq!(
            None,
            may_received::<TestCurrencies, _>(vec![in_coin_1, in_coin_2], Expect(coin))
        );
    }

    #[test]
    fn may_received_in_group() {
        let coin = Coin::<TheCurrency>::new(AMOUNT);
        let in_coin_1 = coin_legacy::to_cosmwasm(coin);
        assert_eq!(
            Some(Ok(true)),
            may_received::<TestCurrencies, _>(vec![in_coin_1], Expect(coin))
        );
    }

    #[test]
    fn may_received_in_group_others_arround() {
        let in_coin_1 = coin_legacy::to_cosmwasm(Coin::<ExtraCurrency>::new(AMOUNT + AMOUNT));

        let coin_2 = Coin::<TheCurrency>::new(AMOUNT);
        let in_coin_2 = coin_legacy::to_cosmwasm(coin_2);

        let coin_3 = Coin::<TheCurrency>::new(AMOUNT + AMOUNT);
        let in_coin_3 = coin_legacy::to_cosmwasm(coin_3);
        assert_eq!(
            Some(Ok(true)),
            may_received::<TestCurrencies, _>(
                vec![in_coin_1.clone(), in_coin_2.clone(), in_coin_3.clone()],
                Expect(coin_2)
            )
        );
        assert_eq!(
            Some(Ok(true)),
            may_received::<TestCurrencies, _>(
                vec![in_coin_1, in_coin_3, in_coin_2],
                Expect(coin_3),
            )
        );
    }

    struct Cmd<'r> {
        expected: &'r [&'static str],
    }

    impl<'r> Cmd<'r> {
        pub const fn new(expected: &'r [&'static str]) -> Self {
            Self { expected }
        }
    }

    impl WithCoin for Cmd<'_> {
        type Output = ();
        type Error = Error;

        fn on<C>(&self, _: Coin<C>) -> WithCoinResult<Self>
        where
            C: Currency,
        {
            assert!(self.expected.contains(&C::BANK_SYMBOL));

            Ok(())
        }
    }

    fn total_balance_tester<G>(coins: Vec<CwCoin>, expected: &[&'static str])
    where
        G: Group,
    {
        let addr: Addr = Addr::unchecked("user");

        let app: BasicApp<Empty, Empty> = sdk::cw_multi_test::App::new(|router, _, storage| {
            router.bank.init_balance(storage, &addr, coins).unwrap();
        });
        let querier: QuerierWrapper<'_> = app.wrap();

        let bank_view: BankView<'_> = BankView::account(&addr, &querier);

        let cmd: Cmd<'_> = Cmd::new(expected);

        assert_eq!(
            bank_view.balances::<G, Cmd<'_>>(cmd).unwrap().is_none(),
            expected.is_empty()
        );
    }

    #[test]
    fn total_balance_empty() {
        total_balance_tester::<PaymentGroup>(vec![], &[]);
    }

    #[test]
    fn total_balance_same_group() {
        total_balance_tester::<PaymentGroup>(
            vec![cw_coin(100, Atom::BANK_SYMBOL)],
            &[Atom::BANK_SYMBOL],
        );
    }

    #[test]
    fn total_balance_different_group() {
        total_balance_tester::<Native>(vec![cw_coin(100, Usdc::BANK_SYMBOL)], &[]);
    }

    #[test]
    fn total_balance_mixed_group() {
        total_balance_tester::<Native>(
            vec![cw_coin(100, Usdc::TICKER), cw_coin(100, Nls::BANK_SYMBOL)],
            &[Nls::BANK_SYMBOL],
        );
    }
}
