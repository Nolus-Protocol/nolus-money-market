use finance::coin::Coin;
use lpp_platform::NLpn;
use serde::Serialize;

use currency::CurrencyDef;
use platform::{
    bank::{self, BankAccount},
    batch::Batch,
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Storage};

use crate::{
    error::{ContractError, Result},
    lpp::{LiquidityPool, LppBalances},
    msg::RewardsResponse,
    state::Deposit,
};

pub(super) fn try_distribute_rewards(
    deps: DepsMut<'_>,
    info: MessageInfo,
) -> Result<MessageResponse> {
    bank::received_one(info.funds)
        .map_err(Into::into)
        .and_then(|amount| Deposit::distribute_rewards(deps, amount))
        .map(|()| Default::default())
}

pub(super) fn try_claim_rewards(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    other_recipient: Option<Addr>,
) -> Result<MessageResponse> {
    let recipient = other_recipient
        .map(|recipient| deps.api.addr_validate(recipient.as_str()))
        .transpose()?
        .unwrap_or_else(|| info.sender.clone());

    let mut deposit =
        Deposit::may_load(deps.storage, info.sender)?.ok_or(ContractError::NoDeposit {})?;

    let reward = deposit.claim_rewards(deps.storage)?;

    if reward.is_zero() {
        return Err(ContractError::NoRewards {});
    }

    let bank = bank::account(&env.contract.address, deps.querier).send(reward, recipient);
    let batch: Batch = bank.into();
    Ok(batch.into())
}

pub(super) fn query_lpp_balance<Lpn>(deps: Deps<'_>, env: Env) -> Result<LppBalances<Lpn>>
where
    Lpn: 'static + CurrencyDef + Serialize,
{
    LiquidityPool::<Lpn>::load(deps.storage).and_then(|lpp| lpp.query_lpp_balance(&deps, &env))
}

pub(super) fn query_total_rewards(storage: &dyn Storage) -> Result<Coin<NLpn>> {
    Deposit::balance_nlpn(storage).map_err(Into::into)
}

pub(super) fn query_rewards(storage: &dyn Storage, addr: Addr) -> Result<RewardsResponse> {
    let rewards = Deposit::may_load(storage, addr)?
        .ok_or(ContractError::NoDeposit {})?
        .query_rewards(storage)?;

    Ok(RewardsResponse { rewards })
}

#[cfg(test)]
mod test {
    use access_control::ContractOwnerAccess;
    use finance::percent::{bound::BoundToHundredPercent, Percent};
    use platform::contract::Code;
    use sdk::cosmwasm_std::{
        testing::{mock_dependencies, mock_env, MOCK_CONTRACT_ADDR},
        Addr,
    };

    use crate::{
        borrow::InterestRate,
        contract::{
            lender, rewards,
            test::{self, TheCurrency},
        },
        error::ContractError,
        lpp::LiquidityPool,
        state::Config,
    };

    const BASE_INTEREST_RATE: Percent = Percent::from_permille(70);
    const UTILIZATION_OPTIMAL: Percent = Percent::from_permille(700);
    const ADDON_OPTIMAL_INTEREST_RATE: Percent = Percent::from_permille(20);
    const DEFAULT_MIN_UTILIZATION: BoundToHundredPercent = BoundToHundredPercent::ZERO;

    #[test]
    fn test_claim_zero_rewards() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let mut lpp_balance = 0;
        let deposit = 20_000;

        ContractOwnerAccess::new(deps.as_mut().storage)
            .grant_to(&Addr::unchecked("admin"))
            .unwrap();

        LiquidityPool::<TheCurrency>::store(
            deps.as_mut().storage,
            Config::new_unchecked(
                Code::unchecked(1000u64),
                InterestRate::new(
                    BASE_INTEREST_RATE,
                    UTILIZATION_OPTIMAL,
                    ADDON_OPTIMAL_INTEREST_RATE,
                )
                .expect("Couldn't construct interest rate value!"),
                DEFAULT_MIN_UTILIZATION,
            ),
        )
        .unwrap();

        // no deposit
        let info = test::lender_msg_no_funds();
        let response = super::try_claim_rewards(deps.as_mut(), env.clone(), info, None);
        assert_eq!(response, Err(ContractError::NoDeposit {}));

        lpp_balance += deposit;
        let info = test::lender_msg_with_funds(deposit);
        deps.querier
            .bank
            .update_balance(MOCK_CONTRACT_ADDR, vec![test::cwcoin(lpp_balance)]);
        lender::try_deposit::<TheCurrency>(deps.as_mut(), env.clone(), info).unwrap();

        // pending rewards == 0
        let info = test::lender_msg_no_funds();
        let response = rewards::try_claim_rewards(deps.as_mut(), env, info, None);
        assert_eq!(response, Err(ContractError::NoRewards {}));
    }
}
