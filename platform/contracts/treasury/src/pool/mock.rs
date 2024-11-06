use finance::{duration::Duration, percent::Percent};
use lpp_platform::CoinStable;
use platform::{
    batch::{Batch, Emit, Emitter},
    message::Response,
};
use sdk::cosmwasm_std::{Addr, StdError};

use crate::ContractError;

use super::Pool;

enum DistributeRewards {
    None,
    Pass,
    Fail,
}

pub struct MockPool {
    balance: CoinStable,
    apr: Percent,
    period: Duration,
    rewards_result: DistributeRewards,
}

impl MockPool {
    pub fn reward_none(balance: CoinStable) -> Self {
        Self {
            balance,
            apr: Default::default(),
            period: Default::default(),
            rewards_result: DistributeRewards::None,
        }
    }

    pub fn reward_ok(balance: CoinStable, apr: Percent, period: Duration) -> Self {
        Self {
            balance,
            apr,
            period,
            rewards_result: DistributeRewards::Pass,
        }
    }

    pub fn reward_fail(balance: CoinStable, apr: Percent, period: Duration) -> Self {
        Self {
            balance,
            apr,
            period,
            rewards_result: DistributeRewards::Fail,
        }
    }
}

impl Pool for MockPool {
    fn balance(&self) -> CoinStable {
        self.balance
    }

    fn distribute_rewards(self, apr: Percent, period: Duration) -> Result<Response, ContractError> {
        let res = match self.rewards_result {
            DistributeRewards::None => {
                unreachable!("calling Pool::distribute_rewards is not expected")
            }
            DistributeRewards::Pass => Batch::default()
                .schedule_execute_wasm_no_reply_no_funds(Addr::unchecked("DEADCODE"), "msg1")
                .map_err(ContractError::SerializeResponse)
                .map(|msgs| {
                    let events =
                        Emitter::of_type("test-distribution").emit_percent_amount("attr_apr", apr);
                    Response::messages_with_events(msgs, events)
                }),
            DistributeRewards::Fail => Err(ContractError::DistributeLppReward(
                lpp_platform::error::Error::Std(StdError::generic_err("Error from the MockPool")),
            )),
        };
        assert_eq!(self.apr, apr);
        assert_eq!(self.period, period);
        res
    }
}
