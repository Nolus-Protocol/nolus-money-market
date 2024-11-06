use std::{marker::PhantomData, result::Result as StdResult};

use currency::{CurrencyDef, Group, MemberOf};
use finance::coin::{Coin, WithCoin, WithCoinResult};
use sdk::cosmwasm_std::{Addr, BankMsg, Coin as CwCoin, QuerierWrapper};

use crate::{
    batch::Batch,
    coin_legacy::{self, from_cosmwasm_any, maybe_from_cosmwasm_any, to_cosmwasm_impl},
    error::Error,
    result::Result,
};

pub type BalancesResult<G, Cmd> = StdResult<Option<WithCoinResult<G, Cmd>>, Error>;

pub trait BankAccountView {
    fn balance<C, G>(&self) -> Result<Coin<C>>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        G: Group;

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<G, Cmd>
    where
        G: Group,
        Cmd: WithCoin<G> + Clone,
        Cmd::Output: Aggregate;
}

pub trait BankAccount
where
    Self: BankAccountView + Into<Batch>,
{
    fn send<C>(self, amount: Coin<C>, to: Addr) -> Self
    where
        C: CurrencyDef;
}

pub trait FixedAddressSender
where
    Self: Into<Batch>,
{
    fn send<C>(&mut self, amount: Coin<C>)
    where
        C: CurrencyDef;
}

/// Ensure a single coin of the specified currency is received by a contract and return it
pub fn received_one<C>(cw_amount: Vec<CwCoin>) -> Result<Coin<C>>
where
    C: CurrencyDef,
{
    received_one_impl(
        cw_amount,
        || Error::no_funds::<C>(C::definition()),
        || Error::unexpected_funds::<C>(C::definition()),
    )
    .and_then(coin_legacy::from_cosmwasm::<C>)
}

/// Run a command on the first coin of the specified group
pub fn may_received<VisitedG, V>(
    cw_amount: &Vec<CwCoin>,
    mut cmd: V,
) -> Option<WithCoinResult<VisitedG, V>>
where
    VisitedG: Group,
    V: WithCoin<VisitedG>,
{
    let mut may_res = None;
    for coin in cw_amount {
        cmd = match from_cosmwasm_any(coin, cmd) {
            Ok(res) => {
                may_res = Some(res);
                break;
            }
            Err(cmd) => cmd,
        }
    }
    may_res
}

pub struct BankView<'a> {
    account: &'a Addr,
    querier: QuerierWrapper<'a>,
}

impl<'a> BankView<'a> {
    fn account(account: &'a Addr, querier: QuerierWrapper<'a>) -> Self {
        Self { account, querier }
    }
}

impl<'a> BankAccountView for BankView<'a> {
    fn balance<C, G>(&self) -> Result<Coin<C>>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        G: Group,
    {
        self.querier
            .query_balance(self.account, C::definition().dto().definition().bank_symbol)
            .map_err(Error::CosmWasmQueryBalance)
            .and_then(coin_legacy::from_cosmwasm_currency_not_definition::<C, C>)
    }

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<G, Cmd>
    where
        G: Group,
        Cmd: WithCoin<G> + Clone,
        Cmd::Output: Aggregate,
    {
        self.querier
            .query_all_balances(self.account)
            .map_err(Error::CosmWasmQueryAllBalances)
            .map(|cw_coins| {
                cw_coins
                    .into_iter()
                    .filter_map(|cw_coin| maybe_from_cosmwasm_any::<G, _>(cw_coin, cmd.clone()))
                    .reduce_results(Aggregate::aggregate)
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
    fn new(view: View) -> Self {
        Self {
            view,
            batch: Batch::default(),
        }
    }

    #[cfg(feature = "testing")]
    pub fn with_view(view: View) -> Self {
        Self::new(view)
    }
}

pub fn account<'a>(account: &'a Addr, querier: QuerierWrapper<'a>) -> BankStub<BankView<'a>> {
    BankStub::new(BankView::account(account, querier))
}

pub fn balance<'a, C, G>(account: &'a Addr, querier: QuerierWrapper<'a>) -> Result<Coin<C>>
where
    C: CurrencyDef,
    C::Group: MemberOf<G>,
    G: Group,
{
    BankView { account, querier }.balance()
}

/// Send all coins to a recipient
pub fn bank_send_all<G>(from: &Addr, to: Addr, querier: QuerierWrapper<'_>) -> Result<Batch>
where
    G: Group,
{
    #[derive(Clone)]
    struct SendAny<G> {
        to: Addr,
        _g: PhantomData<G>,
    }

    impl<G> WithCoin<G> for SendAny<G>
    where
        G: Group,
    {
        type Output = Batch;

        type Error = Error;

        fn on<C>(self, coin: Coin<C>) -> WithCoinResult<G, Self>
        where
            C: CurrencyDef,
            C::Group: MemberOf<G> + MemberOf<G::TopG>,
        {
            let mut sender = LazySenderStub::new(self.to);
            sender.send(coin);
            Ok(sender.into())
        }
    }

    let from_account = account(from, querier);
    from_account
        .balances(SendAny::<G> {
            to,
            _g: PhantomData,
        })
        .and_then(|may_batch| {
            // TODO eliminate the `Result::and_then` once `Result::flatten` gets stabilized
            may_batch.transpose().map(Option::unwrap_or_default)
        })
}

/// Send a single coin to a recepient
#[cfg(feature = "testing")]
pub fn bank_send<C>(to: Addr, amount: Coin<C>) -> Batch
where
    C: CurrencyDef,
{
    bank_send_impl(Batch::default(), to, &[amount])
}

impl<View> BankAccountView for BankStub<View>
where
    View: BankAccountView,
{
    fn balance<C, G>(&self) -> Result<Coin<C>>
    where
        C: CurrencyDef,
        C::Group: MemberOf<G>,
        G: Group,
    {
        self.view.balance()
    }

    fn balances<G, Cmd>(&self, cmd: Cmd) -> BalancesResult<G, Cmd>
    where
        G: Group,
        Cmd: WithCoin<G> + Clone,
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
    fn send<C>(self, amount: Coin<C>, to: Addr) -> Self
    where
        C: CurrencyDef,
    {
        debug_assert!(!amount.is_zero());
        Self {
            view: self.view,
            batch: bank_send_impl(self.batch, to, &[amount]),
        }
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

fn bank_send_impl<C>(batch: Batch, to: Addr, amount: &[Coin<C>]) -> Batch
where
    C: CurrencyDef,
{
    bank_send_cosmwasm(
        batch,
        to,
        amount
            .iter()
            .map(|coin| to_cosmwasm_impl(coin.to_owned()))
            .collect(),
    )
}

fn bank_send_cosmwasm(batch: Batch, to: Addr, amount: Vec<CwCoin>) -> Batch {
    batch.schedule_execute_no_reply(BankMsg::Send {
        amount,
        to_address: to.into(),
    })
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
        C: CurrencyDef,
    {
        if !amount.is_zero() {
            self.amounts.push(to_cosmwasm_impl(amount));
        }
    }
}

impl From<LazySenderStub> for Batch {
    fn from(stub: LazySenderStub) -> Self {
        if !stub.amounts.is_empty() {
            bank_send_cosmwasm(Batch::default(), stub.receiver, stub.amounts)
        } else {
            Batch::default()
        }
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

/// Temporary replacement for functionality similar to
/// [`Iterator::try_reduce`] until the feature is stabilized.
trait ReduceResults
where
    Self: Iterator<Item = StdResult<Self::InnerItem, Self::Error>>,
{
    type InnerItem;
    type Error;

    fn reduce_results<F>(&mut self, f: F) -> Option<StdResult<Self::InnerItem, Self::Error>>
    where
        F: FnMut(Self::InnerItem, Self::InnerItem) -> Self::InnerItem;
}

impl<I, T, E> ReduceResults for I
where
    I: Iterator<Item = StdResult<T, E>>,
{
    type InnerItem = T;
    type Error = E;

    fn reduce_results<F>(&mut self, mut f: F) -> Option<StdResult<T, E>>
    where
        F: FnMut(T, T) -> T,
    {
        self.next().map(|first: StdResult<T, E>| {
            first.and_then(|first: T| {
                self.try_fold(first, |acc: T, element: StdResult<T, E>| {
                    element.map(|element: T| f(acc, element))
                })
            })
        })
    }
}

#[cfg(test)]
mod test {

    use currency::{
        test::{
            SubGroup, SubGroupTestC10, SubGroupTestC6, SuperGroup, SuperGroupTestC1,
            SuperGroupTestC4,
        },
        CurrencyDTO, CurrencyDef, Group, MemberOf,
    };
    use finance::{
        coin::{Amount, Coin, WithCoin, WithCoinResult},
        test::coin::Expect,
    };
    use sdk::{
        cosmwasm_std::{coin as cw_coin, Addr, Coin as CwCoin, QuerierWrapper},
        cw_multi_test::BasicApp,
        testing,
    };

    use crate::{coin_legacy, error::Error};

    use super::{may_received, BankAccountView as _, BankView, ReduceResults as _};

    type TheCurrency = SubGroupTestC10;
    type ExtraCurrency = SuperGroupTestC1;

    const AMOUNT: Amount = 42;
    const USER: &str = "user";

    #[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
    #[error("Test error")]
    struct TestError;

    #[test]
    fn reduce_results_empty() {
        assert_eq!(
            [Ok::<(), TestError>(()); 0]
                .into_iter()
                .reduce_results(|(), ()| unreachable!()),
            None
        );
    }

    #[test]
    fn reduce_results_1_ok() {
        assert_eq!(
            [Ok::<u8, TestError>(1)]
                .into_iter()
                .reduce_results(|_, _| unreachable!()),
            Some(Ok(1))
        );
    }

    #[test]
    fn reduce_results_3_ok() {
        assert_eq!(
            [Ok::<u8, TestError>(1), Ok(2), Ok(3)]
                .into_iter()
                .reduce_results(|acc, element| acc + element),
            Some(Ok(6))
        );
    }

    #[test]
    fn reduce_results_1_err() {
        assert_eq!(
            [Err::<u8, TestError>(TestError)]
                .into_iter()
                .reduce_results(|_, _| unreachable!()),
            Some(Err(TestError))
        );
    }

    #[test]
    fn reduce_results_1_ok_1_err() {
        assert_eq!(
            [Ok::<u8, TestError>(1), Err(TestError)]
                .into_iter()
                .reduce_results(|_, _| unreachable!()),
            Some(Err(TestError))
        );
    }

    #[test]
    fn reduce_results_1_err_1_ok() {
        assert_eq!(
            [Err::<u8, TestError>(TestError), Ok(2)]
                .into_iter()
                .reduce_results(|_, _| unreachable!()),
            Some(Err(TestError))
        );
    }

    #[test]
    fn reduce_results_2_ok_1_err_1_ok() {
        assert_eq!(
            [Ok::<u8, TestError>(1), Ok(2), Err(TestError), Ok(4)]
                .into_iter()
                .reduce_results(|acc, element| {
                    assert_ne!(element, 4);

                    acc + element
                }),
            Some(Err(TestError))
        );
    }

    #[test]
    fn may_received_no_input() {
        assert_eq!(
            None,
            may_received(&vec![], Expect(Coin::<TheCurrency>::from(AMOUNT)))
        );
    }

    #[test]
    fn may_received_not_in_group() {
        let coin_1 = Coin::<ExtraCurrency>::new(AMOUNT);
        let in_coin_1 = coin_legacy::to_cosmwasm(coin_1);

        let coin_2 = Coin::<ExtraCurrency>::new(AMOUNT + AMOUNT);
        let in_coin_2 = coin_legacy::to_cosmwasm(coin_2);

        assert_eq!(
            None,
            may_received(
                &vec![in_coin_1, in_coin_2],
                Expect(Coin::<TheCurrency>::new(AMOUNT))
            )
        );
    }

    #[test]
    fn may_received_in_group() {
        let coin = Coin::<TheCurrency>::new(AMOUNT);
        let in_coin_1 = coin_legacy::to_cosmwasm(coin);
        assert_eq!(Some(Ok(true)), may_received(&vec![in_coin_1], Expect(coin)));
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
            may_received(
                &vec![in_coin_1.clone(), in_coin_2.clone(), in_coin_3.clone()],
                Expect(coin_2)
            )
        );
        assert_eq!(
            Some(Ok(true)),
            may_received(&vec![in_coin_1, in_coin_3, in_coin_2], Expect(coin_3),)
        );
    }

    #[derive(Clone)]
    struct Cmd<G>
    where
        G: Group,
    {
        expected: Option<CurrencyDTO<G>>,
    }

    impl<G> Cmd<G>
    where
        G: Group,
    {
        pub fn expected<C>() -> Cmd<G>
        where
            C: CurrencyDef<Group = G>,
        {
            Cmd::<C::Group> {
                expected: Some(*C::definition().dto()),
            }
        }

        pub const fn expected_none() -> Self {
            Self { expected: None }
        }

        fn validate(&self, balances_result: Option<Result<(), Error>>)
        where
            G: Group,
        {
            if self.expected.is_some() {
                assert_eq!(Some(Ok(())), balances_result)
            } else {
                assert_eq!(None, balances_result)
            }
        }
    }

    impl<G> WithCoin<G> for Cmd<G>
    where
        G: Group,
    {
        type Output = ();
        type Error = Error;

        fn on<C>(self, _: Coin<C>) -> WithCoinResult<G, Self>
        where
            C: CurrencyDef,
            C::Group: MemberOf<G>,
        {
            assert_eq!(
                Some(C::definition().dto().into_super_group::<G>()),
                self.expected
            );

            Ok(())
        }
    }

    fn total_balance_tester<G>(coins: Vec<CwCoin>, mock: Cmd<G>)
    where
        G: Group,
    {
        let user = testing::user(USER);

        let app = BasicApp::new(|router, _, storage| {
            router.bank.init_balance(storage, &user, coins).unwrap();
        });
        let querier: QuerierWrapper<'_> = app.wrap();

        let bank_view: BankView<'_> = BankView::account(&user, querier);

        let result = bank_view.balances::<G, _>(mock.clone()).unwrap();
        mock.validate(result);
    }

    #[test]
    fn total_balance_empty() {
        total_balance_tester::<SubGroup>(vec![], Cmd::expected_none());
    }

    #[test]
    fn total_balance_same_group() {
        total_balance_tester::<SubGroup>(
            vec![cw_coin(100, SubGroupTestC10::bank())],
            Cmd::<SubGroup>::expected::<SubGroupTestC10>(),
        );
    }

    #[test]
    fn total_balance_different_group() {
        total_balance_tester::<SubGroup>(
            vec![cw_coin(100, SuperGroupTestC1::bank())],
            Cmd::expected_none(),
        );
    }

    #[test]
    fn total_balance_mixed_group() {
        total_balance_tester::<SubGroup>(
            vec![
                cw_coin(100, SuperGroupTestC1::ticker()),
                cw_coin(100, SubGroupTestC10::bank()),
            ],
            Cmd::<SubGroup>::expected::<SubGroupTestC10>(),
        );
    }

    #[test]
    fn send_all_none() {
        send_all_tester::<SuperGroup>(vec![], 0);
    }

    #[test]
    fn send_all_subgroup() {
        send_all_tester::<SubGroup>(vec![cw_coin(200, SuperGroupTestC4::bank())], 0);

        send_all_tester::<SubGroup>(vec![cw_coin(100, SubGroupTestC10::dex())], 0);

        send_all_tester::<SubGroup>(
            vec![
                cw_coin(100, SuperGroupTestC1::ticker()),
                cw_coin(100, SubGroupTestC10::bank()),
                cw_coin(200, SuperGroupTestC4::bank()),
            ],
            1,
        );
    }

    #[test]
    fn send_all_supergroup() {
        send_all_tester::<SuperGroup>(vec![cw_coin(200, SuperGroupTestC4::dex())], 0);

        send_all_tester::<SuperGroup>(vec![cw_coin(100, SubGroupTestC10::bank())], 1);

        send_all_tester::<SuperGroup>(
            vec![
                cw_coin(100, SubGroupTestC10::bank()),
                cw_coin(100, SuperGroupTestC1::ticker()),
                cw_coin(100, SubGroupTestC6::bank()),
                cw_coin(200, SuperGroupTestC4::bank()),
            ],
            3,
        );
    }

    #[track_caller]
    fn send_all_tester<G>(coins: Vec<CwCoin>, exp_coins_nb: usize)
    where
        G: Group,
    {
        let from: Addr = testing::user(USER);
        let to = from.clone();

        let app = BasicApp::new(|router, _, storage| {
            router.bank.init_balance(storage, &from, coins).unwrap();
        });
        let querier: QuerierWrapper<'_> = app.wrap();

        let msgs = super::bank_send_all::<G>(&from, to, querier).unwrap();
        assert_eq!(exp_coins_nb, msgs.len());
    }
}
