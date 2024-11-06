use currency::platform::{Nls, PlatformGroup};
use finance::coin::{Coin, CoinDTO};
use platform::{
    batch::{Batch, Emit, Emitter},
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::{Addr, Env, QuerierWrapper};

use crate::{
    error::Result,
    msg::{ExecuteMsg, QueryMsg},
    CoinStable, Lpp,
};

pub struct Stub<'a> {
    lpp: Addr,
    querier: QuerierWrapper<'a>,
    env: &'a Env,
}

impl<'a> Stub<'a> {
    pub(crate) fn new(lpp: Addr, querier: QuerierWrapper<'a>, env: &'a Env) -> Self {
        Self { lpp, querier, env }
    }
}

impl<'a> Lpp for Stub<'a> {
    fn balance(&self, oracle: Addr) -> Result<CoinStable> {
        self.querier
            .query_wasm_smart::<CoinDTO<PlatformGroup>>(
                &self.lpp,
                &(QueryMsg::StableBalance {
                    oracle_addr: oracle,
                }),
            )
            .map_err(Into::into)
            .map(|dto| {
                dto.try_into()
                    .unwrap_or_else(|_| unreachable!("Each stable is member of the plaform group!"))
            })
    }

    fn distribute(self, reward: Coin<Nls>) -> Result<MessageResponse> {
        if reward.is_zero() {
            return Ok(Default::default());
        }

        Batch::default()
            .schedule_execute_wasm_no_reply(
                self.lpp.clone(),
                &ExecuteMsg::DistributeRewards {},
                Some(reward),
            )
            .map_err(Into::into)
            .map(|msgs| {
                let events = Emitter::of_type("tr-rewards")
                    .emit_tx_info(self.env)
                    .emit_to_string_value("to", self.lpp)
                    .emit_coin("rewards", reward);

                MessageResponse::messages_with_events(msgs, events)
            })
    }
}

#[cfg(test)]
mod test {
    use platform::message::Response as MessageResponse;
    use sdk::cosmwasm_std::{
        testing::{self, MockQuerier},
        Addr, QuerierWrapper,
    };

    use crate::Lpp;

    use super::Stub;

    #[test]
    fn ditribute_no_reward() {
        let mock_querier = MockQuerier::default();
        let env = testing::mock_env();
        let querier = QuerierWrapper::new(&mock_querier);
        let lpp_addr = Addr::unchecked("LPP");
        let stub = Stub::new(lpp_addr, querier, &env);
        assert_eq!(Ok(MessageResponse::default()), stub.distribute(0.into()));
    }
}
