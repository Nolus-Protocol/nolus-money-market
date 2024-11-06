use serde::Serialize;

use currency::CurrencyDef;
use finance::{coin::Coin, zero::Zero};
use lpp_platform::NLpn;
use platform::{
    bank::{self, BankAccount},
    batch::Batch,
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Storage, Uint128};

use crate::{
    error::{ContractError, Result},
    event,
    lpp::LiquidityPool,
    msg::{BalanceResponse, PriceResponse},
    state::Deposit,
};

pub(super) fn try_deposit<Lpn>(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
) -> Result<MessageResponse>
where
    Lpn: 'static + CurrencyDef,
{
    let lender_addr = info.sender;
    let pending_deposit = bank::received_one(info.funds)?;

    let lpp = LiquidityPool::<Lpn>::load(deps.storage)?;

    if lpp
        .deposit_capacity(deps.querier, &env, pending_deposit)?
        .map(|capacity| pending_deposit > capacity)
        .unwrap_or_default()
    {
        return Err(ContractError::UtilizationBelowMinimalRates);
    }

    let price = lpp.calculate_price(&deps.as_ref(), &env, pending_deposit)?;

    let receipts = Deposit::load_or_default(deps.storage, lender_addr.clone())?.deposit(
        deps.storage,
        pending_deposit,
        price,
    )?;

    Ok(event::emit_deposit(env, lender_addr, pending_deposit, receipts).into())
}

pub(super) fn deposit_capacity<Lpn>(deps: Deps<'_>, env: Env) -> Result<Option<Coin<Lpn>>>
where
    Lpn: 'static + CurrencyDef + Serialize,
{
    LiquidityPool::<Lpn>::load(deps.storage)
        .and_then(|lpp: LiquidityPool<Lpn>| lpp.deposit_capacity(deps.querier, &env, Coin::ZERO))
}

pub(super) fn try_withdraw<Lpn>(
    deps: DepsMut<'_>,
    env: Env,
    info: MessageInfo,
    amount_nlpn: Uint128,
) -> Result<MessageResponse>
where
    Lpn: 'static + CurrencyDef,
{
    if amount_nlpn.is_zero() {
        return Err(ContractError::ZeroWithdrawFunds);
    }

    let lender_addr = info.sender;
    let amount_nlpn: Coin<NLpn> = amount_nlpn.u128().into();

    let lpp = LiquidityPool::<Lpn>::load(deps.storage)?;
    let payment_lpn = lpp.withdraw_lpn(&deps.as_ref(), &env, amount_nlpn)?;

    let maybe_reward = Deposit::may_load(deps.storage, lender_addr.clone())?
        .ok_or(ContractError::NoDeposit {})?
        .withdraw(deps.storage, amount_nlpn)?;

    let bank =
        bank::account(&env.contract.address, deps.querier).send(payment_lpn, lender_addr.clone());

    let bank = if let Some(reward) = maybe_reward {
        if !reward.is_zero() {
            bank.send(reward, lender_addr.clone())
        } else {
            bank
        }
    } else {
        bank
    };

    let batch: Batch = bank.into();
    Ok(MessageResponse::messages_with_events(
        batch,
        event::emit_withdraw(
            env,
            lender_addr,
            payment_lpn,
            amount_nlpn,
            maybe_reward.is_some(),
        ),
    ))
}

pub fn query_ntoken_price<Lpn>(deps: Deps<'_>, env: Env) -> Result<PriceResponse<Lpn>>
where
    Lpn: CurrencyDef,
{
    LiquidityPool::load(deps.storage).and_then(|lpp| {
        lpp.calculate_price(&deps, &env, Coin::default())
            .map(Into::into)
    })
}

pub fn query_balance(storage: &dyn Storage, addr: Addr) -> Result<BalanceResponse> {
    let balance: u128 = Deposit::query_balance_nlpn(storage, addr)?
        .unwrap_or_default()
        .into();
    Ok(BalanceResponse {
        balance: balance.into(),
    })
}

#[cfg(test)]
mod test {
    use std::ops::DerefMut as _;

    use access_control::ContractOwnerAccess;
    use finance::percent::{bound::BoundToHundredPercent, Percent};
    use platform::contract::Code;
    use sdk::cosmwasm_std::{Addr, Storage};

    use crate::{borrow::InterestRate, contract::test::TheCurrency, state::Config};

    use super::LiquidityPool;

    const BASE_INTEREST_RATE: Percent = Percent::from_permille(70);
    const UTILIZATION_OPTIMAL: Percent = Percent::from_permille(700);
    const ADDON_OPTIMAL_INTEREST_RATE: Percent = Percent::from_permille(20);
    const DEFAULT_MIN_UTILIZATION: BoundToHundredPercent = BoundToHundredPercent::ZERO;

    fn setup_storage(mut storage: &mut dyn Storage, min_utilization: BoundToHundredPercent) {
        ContractOwnerAccess::new(storage.deref_mut())
            .grant_to(&Addr::unchecked("admin"))
            .unwrap();

        LiquidityPool::<TheCurrency>::store(
            storage,
            Config::new_unchecked(
                Code::unchecked(0xDEADC0DE_u64),
                InterestRate::new(
                    BASE_INTEREST_RATE,
                    UTILIZATION_OPTIMAL,
                    ADDON_OPTIMAL_INTEREST_RATE,
                )
                .expect("Couldn't construct interest rate value!"),
                min_utilization,
            ),
        )
        .unwrap();
    }

    mod deposit_withdraw_price {
        use finance::coin::Amount;
        use sdk::cosmwasm_std::{
            testing::{self, MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR},
            Env, OwnedDeps,
        };

        use crate::contract::test;

        use super::{TheCurrency, DEFAULT_MIN_UTILIZATION};

        const DEPOSIT: Amount = 100;

        fn test_case<F>(initial_lpp_balance: Amount, f: F)
        where
            F: FnOnce(OwnedDeps<MockStorage, MockApi, MockQuerier>, Env),
        {
            let mut deps = testing::mock_dependencies();
            let env = testing::mock_env();

            super::setup_storage(deps.as_mut().storage, DEFAULT_MIN_UTILIZATION);

            deps.querier
                .bank
                .update_balance(MOCK_CONTRACT_ADDR, vec![test::cwcoin(initial_lpp_balance)]);

            f(deps, env)
        }

        mod deposit {
            use crate::contract::{lender, test};

            use super::{TheCurrency, DEPOSIT};

            #[test]
            fn test_deposit_zero() {
                super::test_case(0, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_no_funds(),
                    )
                    .unwrap_err();
                })
            }

            #[test]
            fn test_deposit() {
                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    assert_eq!(
                        lender::query_balance(deps.as_ref().storage, test::lender())
                            .unwrap()
                            .balance
                            .u128(),
                        DEPOSIT
                    );
                })
            }
        }

        mod withdraw {
            use finance::coin::Amount;
            use sdk::cosmwasm_std::Uint128;

            use crate::contract::{lender, test};

            use super::{TheCurrency, DEPOSIT};

            #[test]
            fn test_withdraw_zero() {
                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env.clone(),
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    lender::try_withdraw::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_no_funds(),
                        Uint128::default(),
                    )
                    .unwrap_err();
                })
            }

            #[test]
            fn test_partial_withdraw() {
                const WITHDRAWN: Amount = DEPOSIT >> 1;
                const LEFTOVER: Amount = DEPOSIT - WITHDRAWN;

                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env.clone(),
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    lender::try_withdraw::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_no_funds(),
                        WITHDRAWN.into(),
                    )
                    .unwrap();

                    assert_eq!(
                        lender::query_balance(deps.as_ref().storage, test::lender())
                            .unwrap()
                            .balance
                            .u128(),
                        LEFTOVER
                    );
                })
            }

            #[test]
            fn test_full_withdraw() {
                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env.clone(),
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    lender::try_withdraw::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_no_funds(),
                        DEPOSIT.into(),
                    )
                    .unwrap();

                    assert_eq!(
                        lender::query_balance(deps.as_ref().storage, test::lender())
                            .unwrap()
                            .balance
                            .u128(),
                        0
                    );
                })
            }

            #[test]
            fn test_overwithdraw() {
                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env.clone(),
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    lender::try_withdraw::<TheCurrency>(
                        deps.as_mut(),
                        env,
                        test::lender_msg_no_funds(),
                        (DEPOSIT << 1).into(),
                    )
                    .unwrap_err();
                })
            }
        }

        mod nlpn_price {
            use finance::{
                coin::{Amount, Coin},
                price::{self, Price},
            };
            use lpp_platform::NLpn;
            use sdk::cosmwasm_std::testing::MOCK_CONTRACT_ADDR;

            use crate::contract::{lender, test};

            use super::{TheCurrency, DEPOSIT};

            #[test]
            fn test_nlpn_price() {
                const INTEREST: Amount = DEPOSIT >> 2;

                super::test_case(DEPOSIT, |mut deps, env| {
                    lender::try_deposit::<TheCurrency>(
                        deps.as_mut(),
                        env.clone(),
                        test::lender_msg_with_funds(DEPOSIT),
                    )
                    .unwrap();

                    assert_eq!(
                        lender::query_ntoken_price::<TheCurrency>(deps.as_ref(), env.clone())
                            .unwrap()
                            .0,
                        Price::identity(),
                    );

                    deps.querier
                        .bank
                        .update_balance(MOCK_CONTRACT_ADDR, vec![test::cwcoin(DEPOSIT + INTEREST)])
                        .unwrap();

                    let nlpn_price: Price<NLpn, TheCurrency> =
                        lender::query_ntoken_price::<TheCurrency>(deps.as_ref(), env)
                            .unwrap()
                            .0;

                    let coin: Coin<NLpn> = Coin::new(1_000_000);

                    assert_eq!(
                        price::total(coin, nlpn_price),
                        price::total(
                            coin,
                            price::total_of(DEPOSIT.into()).is((DEPOSIT + INTEREST).into())
                        ),
                    );
                })
            }
        }
    }

    mod min_utilization {
        use finance::{
            coin::Amount,
            percent::{bound::BoundToHundredPercent, Percent},
        };
        use sdk::cosmwasm_std::{
            testing::{self, MOCK_CONTRACT_ADDR},
            Addr, MessageInfo,
        };

        use crate::contract::{lender, test};

        use super::{LiquidityPool, TheCurrency};

        fn test_case(
            lpp_balance_at_deposit: Amount,
            borrowed: Amount,
            deposit: Amount,
            min_utilization: BoundToHundredPercent,
            expect_error: bool,
        ) {
            let mut deps = testing::mock_dependencies();
            let env = testing::mock_env();

            super::setup_storage(deps.as_mut().storage, min_utilization);

            if borrowed != 0 {
                deps.querier
                    .bank
                    .update_balance(MOCK_CONTRACT_ADDR, vec![test::cwcoin(borrowed)]);

                LiquidityPool::<TheCurrency>::load(deps.as_ref().storage)
                    .unwrap()
                    .try_open_loan(
                        &mut deps.as_mut(),
                        &env,
                        Addr::unchecked("lease"),
                        borrowed.into(),
                    )
                    .unwrap();
            }

            deps.querier.bank.update_balance(
                MOCK_CONTRACT_ADDR,
                vec![test::cwcoin(lpp_balance_at_deposit + deposit)],
            );

            let info = MessageInfo {
                sender: Addr::unchecked("lender1"),
                funds: vec![test::cwcoin(deposit)],
            };

            let result = lender::try_deposit::<TheCurrency>(deps.as_mut(), env, info);

            assert_eq!(result.is_err(), expect_error, "{result:#?}");
        }

        #[test]
        fn test_no_leases() {
            test_case(
                0,
                0,
                100,
                Percent::from_permille(500).try_into().unwrap(),
                true,
            );
        }

        #[test]
        fn test_below_before_deposit() {
            test_case(
                100,
                0,
                100,
                Percent::from_permille(500).try_into().unwrap(),
                true,
            );
        }

        #[test]
        fn test_below_on_pending_deposit() {
            test_case(
                50,
                50,
                100,
                Percent::from_permille(500).try_into().unwrap(),
                true,
            );
        }

        #[test]
        fn test_at_limit_on_pending_deposit() {
            test_case(
                0,
                50,
                50,
                Percent::from_permille(500).try_into().unwrap(),
                false,
            );
        }

        #[test]
        fn test_at_limit_after_deposit() {
            test_case(
                0,
                50,
                50,
                Percent::from_permille(500).try_into().unwrap(),
                false,
            );
        }

        #[test]
        fn test_above_after_deposit() {
            test_case(
                0,
                100,
                50,
                Percent::from_permille(500).try_into().unwrap(),
                false,
            );
        }

        #[test]
        fn test_uncapped() {
            test_case(50, 0, 50, BoundToHundredPercent::ZERO, false);
        }
    }
}
