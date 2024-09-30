use serde::{Deserialize, Serialize};

use currency::platform::Nls;
use finance::{
    coin::Coin,
    error::Error as FinanceError,
    price::{self, Price},
    zero::Zero,
};
use lpp_platform::NLpn;
use sdk::{
    cosmwasm_std::{Addr, DepsMut, StdError, StdResult, Storage},
    cw_storage_plus::{Item, Map},
};

use crate::{
    error::{ContractError, Result},
    lpp::NTokenPrice,
};

#[derive(Debug)]
pub struct Deposit {
    addr: Addr,
    data: DepositData,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Default)]
struct DepositData {
    deposited_nlpn: Coin<NLpn>,

    // Rewards
    reward_per_token: Option<Price<NLpn, Nls>>,
    pending_rewards_nls: Coin<Nls>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Default)]
struct DepositsGlobals {
    balance_nlpn: Coin<NLpn>,

    // Rewards
    reward_per_token: Option<Price<NLpn, Nls>>,
}

impl Deposit {
    const DEPOSITS: Map<'static, Addr, DepositData> = Map::new("deposits");
    const GLOBALS: Item<'static, DepositsGlobals> = Item::new("deposits_globals");

    pub fn load_or_default(storage: &dyn Storage, addr: Addr) -> StdResult<Self> {
        let data = Self::DEPOSITS
            .may_load(storage, addr.clone())?
            .unwrap_or_default();

        Ok(Self { addr, data })
    }

    pub fn may_load(storage: &dyn Storage, addr: Addr) -> StdResult<Option<Self>> {
        let result = Self::DEPOSITS
            .may_load(storage, addr.clone())?
            .map(|data| Self { addr, data });
        Ok(result)
    }

    pub fn deposit<Lpn>(
        &mut self,
        storage: &mut dyn Storage,
        amount_lpn: Coin<Lpn>,
        price: NTokenPrice<Lpn>,
    ) -> Result<Coin<NLpn>>
    where
        Lpn: Copy,
    {
        if amount_lpn.is_zero() {
            return Err(ContractError::ZeroDepositFunds);
        }

        Self::GLOBALS
            .may_load(storage)
            .map_err(Into::into)
            .and_then(|may_globals| {
                let mut globals = may_globals.unwrap_or_default();
                self.update_rewards(&globals)
                    .ok_or_else(|| {
                        ContractError::Finance(FinanceError::Overflow(
                            "Oveflow while updating the rewrds".to_string(),
                        ))
                    })
                    .and_then(|()| {
                        price::total(amount_lpn, price.get().inv())
                            .ok_or_else(|| {
                                ContractError::Finance(FinanceError::overflow_err(
                                    "while calculating the total",
                                    amount_lpn,
                                    price.get().inv(),
                                ))
                            })
                            .and_then(|deposited_nlpn| {
                                self.data.deposited_nlpn += deposited_nlpn;
                                Self::DEPOSITS
                                    .save(storage, self.addr.clone(), &self.data)
                                    .map_err(Into::into)
                                    .and_then(|()| {
                                        globals.balance_nlpn = globals
                                            .balance_nlpn
                                            .checked_add(deposited_nlpn)
                                            .ok_or(ContractError::Finance(
                                                FinanceError::overflow_err(
                                                    "while calculating the balance",
                                                    globals.balance_nlpn,
                                                    deposited_nlpn,
                                                ),
                                            ))?;

                                        Self::GLOBALS
                                            .save(storage, &globals)
                                            .map_err(Into::into)
                                            .map(|()| deposited_nlpn)
                                    })
                            })
                    })
            })
    }

    /// return optional reward payment msg in case of deleting account
    pub fn withdraw(
        &mut self,
        storage: &mut dyn Storage,
        amount_nlpn: Coin<NLpn>,
    ) -> Result<Option<Coin<Nls>>> {
        if self.data.deposited_nlpn < amount_nlpn {
            return Err(ContractError::InsufficientBalance);
        }

        let mut globals = Self::GLOBALS.may_load(storage)?.unwrap_or_default();
        self.update_rewards(&globals)
            .ok_or_else(|| {
                ContractError::Finance(FinanceError::Overflow(
                    "Oveflow while updating the rewrds".to_string(),
                ))
            })
            .and_then(|()| {
                self.data.deposited_nlpn -= amount_nlpn;
                globals.balance_nlpn -= amount_nlpn;

                let maybe_reward = if self.data.deposited_nlpn.is_zero() {
                    Self::DEPOSITS.remove(storage, self.addr.clone());
                    Some(self.data.pending_rewards_nls)
                } else {
                    Self::DEPOSITS.save(storage, self.addr.clone(), &self.data)?;
                    None
                };

                Self::GLOBALS.save(storage, &globals)?;

                Ok(maybe_reward)
            })
    }

    pub fn distribute_rewards(deps: DepsMut<'_>, rewards: Coin<Nls>) -> Result<()> {
        let mut globals = Self::GLOBALS.may_load(deps.storage)?.unwrap_or_default();

        if globals.balance_nlpn.is_zero() {
            return Err(ContractError::ZeroBalanceRewards {});
        }

        if rewards.is_zero() {
            return Err(ContractError::ZeroRewardsFunds {});
        }

        let partial_price = price::total_of(globals.balance_nlpn).is(rewards);

        if let Some(ref mut reward_per_token) = globals.reward_per_token {
            *reward_per_token += partial_price;
        } else {
            globals.reward_per_token = Some(partial_price);
        }

        Ok(Self::GLOBALS.save(deps.storage, &globals)?)
    }

    fn update_rewards(&mut self, globals: &DepositsGlobals) -> Option<()> {
        self.calculate_reward(globals).map(|reward| {
            self.data.pending_rewards_nls = reward;
            self.data.reward_per_token = globals.reward_per_token;
        })
    }

    fn calculate_reward(&self, globals: &DepositsGlobals) -> Option<Coin<Nls>> {
        let deposit = &self.data;

        let may_global_reward = globals.reward_per_token.map_or_else(
            || Some(Coin::ZERO),
            |price| price::total(deposit.deposited_nlpn, price),
        );

        let may_deposit_reward = deposit.reward_per_token.map_or_else(
            || Some(Coin::ZERO),
            |price| price::total(deposit.deposited_nlpn, price),
        );

        may_global_reward.and_then(|global_reward| {
            may_deposit_reward
                .map(|deposit_reward| deposit.pending_rewards_nls + global_reward - deposit_reward)
        })
    }

    /// query accounted rewards
    pub fn query_rewards(&self, storage: &dyn Storage) -> StdResult<Coin<Nls>> {
        let globals = Self::GLOBALS.may_load(storage)?.unwrap_or_default();
        self.calculate_reward(&globals)
            .ok_or_else(|| StdError::generic_err("Failed to calculate rewards"))
    }

    /// pay accounted rewards to the deposit owner or optional recipient
    pub fn claim_rewards(&mut self, storage: &mut dyn Storage) -> StdResult<Coin<Nls>> {
        let globals = Self::GLOBALS.may_load(storage)?.unwrap_or_default();
        self.update_rewards(&globals)
            .ok_or_else(|| StdError::generic_err("Failed to update rewards"))
            .and_then(|()| {
                let reward = self.data.pending_rewards_nls;
                self.data.pending_rewards_nls = Coin::ZERO;

                Self::DEPOSITS
                    .save(storage, self.addr.clone(), &self.data)
                    .map_err(Into::into)
                    .map(|()| reward)
            })
    }

    /// lpp derivative tokens balance
    pub fn balance_nlpn(storage: &dyn Storage) -> StdResult<Coin<NLpn>> {
        Ok(Self::GLOBALS
            .may_load(storage)?
            .unwrap_or_default()
            .balance_nlpn)
    }

    /// deposit derivative tokens balance
    pub fn query_balance_nlpn(storage: &dyn Storage, addr: Addr) -> StdResult<Option<Coin<NLpn>>> {
        let maybe_balance = Self::DEPOSITS
            .may_load(storage, addr)?
            .map(|data| data.deposited_nlpn);
        Ok(maybe_balance)
    }
}

#[cfg(test)]
mod test {
    use currencies::Lpn;
    use sdk::cosmwasm_std::testing;

    use crate::lpp::NTokenPrice;

    use super::*;

    type TheCurrency = Lpn;

    #[test]
    fn test_deposit_and_withdraw() {
        let mut deps = testing::mock_dependencies();
        let addr1 = Addr::unchecked("depositor1");
        let addr2 = Addr::unchecked("depositor2");
        let price = NTokenPrice::<TheCurrency>::mock(Coin::new(1), Coin::new(1));

        let mut deposit1 =
            Deposit::load_or_default(deps.as_ref().storage, addr1.clone()).expect("should load");
        deposit1
            .deposit(deps.as_mut().storage, 1000u128.into(), price)
            .expect("should deposit");

        Deposit::distribute_rewards(deps.as_mut(), Coin::new(1000))
            .expect("should distribute rewards");

        let price = NTokenPrice::<TheCurrency>::mock(Coin::new(1), Coin::new(2));
        let mut deposit2 =
            Deposit::load_or_default(deps.as_ref().storage, addr2.clone()).expect("should load");
        deposit2
            .deposit(deps.as_mut().storage, 1000u128.into(), price)
            .expect("should deposit");

        let balance_nlpn =
            Deposit::balance_nlpn(deps.as_ref().storage).expect("should query balance_nlpn");
        assert_eq!(balance_nlpn, 1500u128.into());

        let balance2 = Deposit::query_balance_nlpn(deps.as_ref().storage, addr2)
            .expect("should query deposit balance_nlpn")
            .expect("should be some balance");
        assert_eq!(balance2, 500u128.into());

        let reward = deposit1
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(1000));

        let reward = deposit2
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(0));

        Deposit::distribute_rewards(deps.as_mut(), Coin::new(1500))
            .expect("should distribute rewards");

        let reward = deposit1
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(2000));

        let reward = deposit2
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(500));

        let some_rewards = deposit1
            .withdraw(deps.as_mut().storage, 500u128.into())
            .expect("should withdraw");
        assert!(some_rewards.is_none());

        let amount = deposit1
            .claim_rewards(deps.as_mut().storage)
            .expect("should claim rewards");
        assert_eq!(amount, Coin::<Nls>::new(2000));

        let amount = deposit2
            .claim_rewards(deps.as_mut().storage)
            .expect("should claim rewards");
        assert_eq!(amount, Coin::<Nls>::new(500));

        Deposit::distribute_rewards(deps.as_mut(), Coin::new(1000))
            .expect("should distribute rewards");

        let reward = deposit1
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(500));

        let reward = deposit2
            .query_rewards(deps.as_ref().storage)
            .expect("should query rewards");

        assert_eq!(reward, Coin::new(500));

        // withdraw all, return rewards, close deposit
        let rewards = deposit1
            .withdraw(deps.as_mut().storage, 500u128.into())
            .expect("should withdraw")
            .expect("should be some rewards");
        assert_eq!(rewards, Coin::<Nls>::new(500));
        let response =
            Deposit::query_balance_nlpn(deps.as_mut().storage, addr1).expect("should query");
        assert!(response.is_none());
    }

    #[test]
    fn test_query_rewards_zero_balance() {
        let mut deps = testing::mock_dependencies();
        let price = NTokenPrice::<TheCurrency>::mock(Coin::new(1), Coin::new(1));
        let addr = Addr::unchecked("depositor");

        let mut deposit =
            Deposit::load_or_default(deps.as_ref().storage, addr).expect("should load");

        // balance_nls = 0, balance_nlpn = 0
        let rewards = deposit
            .query_rewards(deps.as_ref().storage)
            .expect("should query");
        assert_eq!(Coin::<Nls>::new(0), rewards);

        // balance_nls = 0, balance_nlpn != 0
        deposit
            .deposit(deps.as_mut().storage, Coin::<Lpn>::new(1000), price)
            .expect("should deposit");

        let rewards = deposit
            .query_rewards(deps.as_ref().storage)
            .expect("should query");
        assert_eq!(Coin::<Nls>::new(0), rewards);
    }

    #[test]
    fn test_zero_funds_rewards() {
        let mut deps = testing::mock_dependencies();
        let price = NTokenPrice::<TheCurrency>::mock(Coin::new(1), Coin::new(1));
        let addr = Addr::unchecked("depositor");

        let mut deposit =
            Deposit::load_or_default(deps.as_ref().storage, addr).expect("should load");

        deposit
            .deposit(deps.as_mut().storage, Coin::<Lpn>::new(1000), price)
            .expect("should deposit");

        // shouldn't change anything
        Deposit::distribute_rewards(deps.as_mut(), Coin::new(0)).unwrap_err();
    }

    #[test]
    fn test_zero_balance_distribute_rewards() {
        let mut deps = testing::mock_dependencies();
        let rewards = Coin::new(1000);

        Deposit::distribute_rewards(deps.as_mut(), rewards).unwrap_err();
    }
}
