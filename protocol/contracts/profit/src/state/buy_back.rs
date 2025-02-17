use serde::{Deserialize, Serialize};

use currencies::{Native, Nls, PaymentGroup};
use currency::CurrencyDTO;
use dex::{
    Account, CoinVisitor, ContractInSwap, Enterable, IterNext, IterState, Response as DexResponse,
    StateLocalOut, SwapTask,
};
use finance::{
    coin::{Coin, CoinDTO},
    duration::Duration,
};
use oracle::stub::SwapPath;
use platform::{
    bank::{self, BankAccountView},
    message::Response as PlatformResponse,
};
use sdk::cosmwasm_std::{Addr, Env, QuerierWrapper, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{msg::ConfigResponse, profit::Profit, result::ContractResult};

use super::{
    idle::Idle, resp_delivery::ForwardToDexEntry, Config, ConfigManagement, ProfitCurrencies,
    State, StateEnum, SwapClient,
};

#[derive(Serialize, Deserialize)]
pub(super) struct BuyBack {
    profit_contract: Addr,
    config: Config,
    account: Account,
    coins: Vec<CoinDTO<PaymentGroup>>,
}

impl BuyBack {
    /// Until [issue #7](https://github.com/nolus-protocol/nolus-money-market/issues/7)
    /// is closed, best action is to verify the pinkie-promise
    /// to not pass in [native currencies](Native) via a debug
    /// assertion.
    pub fn new(
        profit_contract: Addr,
        config: Config,
        account: Account,
        coins: Vec<CoinDTO<PaymentGroup>>,
    ) -> Self {
        debug_assert!(
            coins
                .iter()
                .all(|not_native: &CoinDTO<PaymentGroup>| not_native.currency()
                    != currency::dto::<Nls, PaymentGroup>()),
            "{:?}",
            coins
        );

        Self {
            profit_contract,
            config,
            account,
            coins,
        }
    }
}

impl SwapTask for BuyBack {
    type InG = PaymentGroup;
    type OutG = Native;
    type InOutG = PaymentGroup;
    type Label = String;
    type StateResponse = ConfigResponse;
    type Result = ContractResult<DexResponse<State>>;

    fn label(&self) -> Self::Label {
        String::from("BuyBack")
    }

    fn dex_account(&self) -> &Account {
        &self.account
    }

    fn oracle(&self) -> &impl SwapPath<Self::InOutG> {
        self.config.oracle()
    }

    fn time_alarm(&self) -> &TimeAlarmsRef {
        self.config.time_alarms()
    }

    fn out_currency(&self) -> CurrencyDTO<Self::OutG> {
        currency::dto::<Nls, Self::OutG>()
    }

    fn on_coins<Visitor>(&self, visitor: &mut Visitor) -> Result<IterState, Visitor::Error>
    where
        Visitor: CoinVisitor<GIn = Self::InG, Result = IterNext>,
    {
        let mut coins_iter = self.coins.iter();

        TryFind::try_find(&mut coins_iter, |coin| {
            visitor
                .visit(coin)
                .map(|result| matches!(result, IterNext::Stop))
        })
        .map(|_| {
            if coins_iter.as_slice().is_empty() {
                IterState::Complete
            } else {
                IterState::Incomplete
            }
        })
    }

    fn finish(
        self,
        _: CoinDTO<Self::OutG>,
        env: &Env,
        querier: QuerierWrapper<'_>,
    ) -> Self::Result {
        let account = bank::account(&self.profit_contract, querier);

        let balance_nls: Coin<Nls> = account.balance::<_, Native>()?;

        let bank_response: PlatformResponse =
            Profit::transfer_nls(account, self.config.treasury().clone(), balance_nls, env);

        let next_state: Idle = Idle::new(self.config, self.account);

        Ok(DexResponse::<State> {
            response: next_state
                .enter(env.block.time, querier)
                .map(PlatformResponse::messages_only)
                .map(|state_response: PlatformResponse| state_response.merge_with(bank_response))?,
            next_state: State(StateEnum::Idle(next_state)),
        })
    }
}

impl<DexState> ContractInSwap<DexState> for BuyBack {
    type StateResponse = <Self as SwapTask>::StateResponse;

    fn state(
        self,
        _: Timestamp,
        _due_projection: Duration,
        _: QuerierWrapper<'_>,
    ) -> <Self as SwapTask>::StateResponse {
        ConfigResponse {
            cadence_hours: self.config.cadence_hours(),
        }
    }
}

impl ConfigManagement for StateLocalOut<BuyBack, ProfitCurrencies, SwapClient, ForwardToDexEntry> {}

trait TryFind
where
    Self: Iterator + Sized,
{
    fn try_find<F, E>(&mut self, mut f: F) -> Result<Option<Self::Item>, E>
    where
        F: FnMut(&Self::Item) -> Result<bool, E>,
    {
        self.find_map(move |item| match f(&item) {
            Ok(true) => Some(Ok(item)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .transpose()
    }
}

impl<I> TryFind for I where I: Iterator {}

#[cfg(all(feature = "internal.test.contract", test))]
mod tests {
    use currencies::{
        testing::{PaymentC3, PaymentC4, PaymentC5, PaymentC6, PaymentC7},
        Lpn, Lpns, PaymentGroup,
    };
    use currency::{never::Never, Group, MemberOf};
    use dex::{CoinVisitor, IterNext, IterState, SwapTask as _};
    use finance::coin::{Coin, CoinDTO};

    use super::BuyBack;

    fn buy_back_instance(coins: Vec<CoinDTO<PaymentGroup>>) -> BuyBack {
        use dex::{Account, ConnectionParams, Ics20Channel};
        use oracle_platform::OracleRef;
        use platform::ica::HostAccount;
        use sdk::cosmwasm_std::Addr;
        use timealarms::stub::TimeAlarmsRef;

        use crate::state::Config;

        BuyBack::new(
            Addr::unchecked("DEADCODE"),
            Config::new(
                24,
                Addr::unchecked("DEADCODE"),
                OracleRef::<Lpn, Lpns>::unchecked(Addr::unchecked("DEADCODE")),
                TimeAlarmsRef::unchecked("DEADCODE"),
            ),
            Account::unchecked(
                Addr::unchecked("DEADCODE"),
                HostAccount::try_from(String::from("DEADCODE"))
                    .expect("Address should be a non-empty string"),
                ConnectionParams {
                    connection_id: String::from("DEADCODE"),
                    transfer_channel: Ics20Channel {
                        local_endpoint: String::from("DEADCODE"),
                        remote_endpoint: String::from("DEADCODE"),
                    },
                },
            ),
            coins,
        )
    }

    struct Visitor {
        stop_after: Option<usize>,
    }

    impl Visitor {
        fn new(stop_after: Option<usize>) -> Self {
            Self { stop_after }
        }
    }

    impl CoinVisitor for Visitor {
        type GIn = PaymentGroup;

        type Result = IterNext;

        type Error = Never;

        fn visit<G>(&mut self, _: &CoinDTO<G>) -> Result<Self::Result, Self::Error>
        where
            G: Group + MemberOf<Self::GIn>,
        {
            if let Some(stop_after) = &mut self.stop_after {
                if *stop_after == 0 {
                    return Ok(IterNext::Stop);
                }

                *stop_after -= 1;
            }

            Ok(IterNext::Continue)
        }
    }

    #[test]
    fn always_continue() {
        let buy_back: BuyBack = buy_back_instance(vec![
            Coin::<PaymentC7>::new(100).into(),
            Coin::<PaymentC4>::new(200).into(),
        ]);

        assert_eq!(
            buy_back.on_coins(&mut Visitor::new(None)).unwrap(),
            IterState::Complete
        );
    }

    #[test]
    fn stop_on_first() {
        let buy_back: BuyBack = buy_back_instance(vec![
            Coin::<PaymentC3>::new(100).into(),
            Coin::<Lpn>::new(200).into(),
        ]);

        assert_eq!(
            buy_back.on_coins(&mut Visitor::new(Some(0))).unwrap(),
            IterState::Incomplete
        );
    }

    #[test]
    fn stop_on_second() {
        let buy_back: BuyBack = buy_back_instance(vec![
            Coin::<PaymentC6>::new(100).into(),
            Coin::<PaymentC5>::new(200).into(),
        ]);

        assert_eq!(
            buy_back.on_coins(&mut Visitor::new(Some(1))).unwrap(),
            IterState::Complete
        );
    }
}
