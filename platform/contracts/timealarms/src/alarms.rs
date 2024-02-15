use platform::{
    contract,
    dispatcher::{AlarmsDispatcher, Id},
    message::Response as MessageResponse,
};
use sdk::{
    cosmwasm_ext::as_dyn::storage,
    cosmwasm_std::{Addr, Env, QuerierWrapper, Timestamp},
};
use time_oracle::Alarms;

use crate::{
    msg::{AlarmsCount, AlarmsStatusResponse, ExecuteAlarmMsg},
    result::ContractResult,
    ContractError,
};

const ALARMS_NAMESPACE: &str = "alarms";
const ALARMS_IDX_NAMESPACE: &str = "alarms_idx";
const IN_DELIVERY_NAMESPACE: &str = "in_delivery";
const REPLY_ID: Id = 0;
const EVENT_TYPE: &str = "timealarm";

pub(super) struct TimeAlarms<S>
where
    S: storage::Dyn,
{
    time_alarms: Alarms<S>,
}

impl<S> TimeAlarms<S>
where
    S: storage::Dyn,
{
    pub fn new(storage: S) -> Self {
        Self {
            time_alarms: Alarms::new(
                storage,
                ALARMS_NAMESPACE,
                ALARMS_IDX_NAMESPACE,
                IN_DELIVERY_NAMESPACE,
            ),
        }
    }

    pub fn try_any_alarm(&self, ctime: Timestamp) -> Result<AlarmsStatusResponse, ContractError> {
        let remaining_alarms = self
            .time_alarms
            .alarms_selection(ctime)
            .next()
            .transpose()?
            .is_some();

        Ok(AlarmsStatusResponse { remaining_alarms })
    }
}

impl<S> TimeAlarms<S>
where
    S: storage::DynMut,
{
    pub fn try_add(
        &mut self,
        querier: QuerierWrapper<'_>,
        env: &Env,
        subscriber: Addr,
        time: Timestamp,
    ) -> ContractResult<MessageResponse> {
        if time < env.block.time {
            return Err(ContractError::InvalidAlarm(time));
        }

        contract::validate_addr(querier, &subscriber)
            .map_err(ContractError::from)
            .and_then(|()| self.time_alarms.add(subscriber, time).map_err(Into::into))
            .map(|()| Default::default())
    }

    pub fn try_notify(
        &mut self,
        ctime: Timestamp,
        max_count: AlarmsCount,
    ) -> ContractResult<(AlarmsCount, MessageResponse)> {
        self.time_alarms
            .ensure_no_in_delivery_mut()?
            .alarms_selection(ctime)
            .take(max_count.try_into()?)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .try_fold(
                AlarmsDispatcher::new(ExecuteAlarmMsg::TimeAlarm {}, EVENT_TYPE),
                |dispatcher: AlarmsDispatcher<ExecuteAlarmMsg>,
                 subscriber: Addr|
                 -> ContractResult<_> {
                    dispatcher
                        .send_to(subscriber.clone(), REPLY_ID)
                        .map_err(Into::into)
                        .and_then(|dispatcher| {
                            self.time_alarms
                                .out_for_delivery(subscriber)
                                .map(|()| dispatcher)
                                .map_err(Into::into)
                        })
                },
            )
            .map(|dispatcher| (dispatcher.nb_sent(), dispatcher.into()))
    }

    pub fn last_delivered(&mut self) -> ContractResult<()> {
        self.time_alarms.last_delivered().map_err(Into::into)
    }

    pub fn last_failed(&mut self, now: Timestamp) -> ContractResult<()> {
        self.time_alarms.last_failed(now).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::ops::DerefMut;

    use platform::contract;
    use sdk::cosmwasm_std::{
        testing::MockStorage,
        testing::{self, mock_dependencies, MockQuerier},
        Addr, QuerierWrapper, Timestamp,
    };

    use crate::ContractError;

    use super::TimeAlarms;

    #[test]
    fn try_add_invalid_contract_address() {
        let mut deps = mock_dependencies();
        let mut deps = deps.as_mut();
        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(0);

        let msg_sender = Addr::unchecked("some address");
        assert!(TimeAlarms::new(deps.storage.deref_mut())
            .try_add(
                deps.querier,
                &env,
                msg_sender.clone(),
                Timestamp::from_nanos(8),
            )
            .is_err());

        let expected_error: ContractError = contract::validate_addr(deps.querier, &msg_sender)
            .unwrap_err()
            .into();

        let result = TimeAlarms::new(deps.storage)
            .try_add(deps.querier, &env, msg_sender, Timestamp::from_nanos(8))
            .unwrap_err();

        assert_eq!(expected_error, result);
    }

    #[test]
    fn try_add_valid_contract_address() {
        let mut mock_querier = MockQuerier::default();
        mock_querier.update_wasm(contract::testing::valid_contract_handler);
        let querier = QuerierWrapper::new(&mock_querier);

        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(0);

        let msg_sender = Addr::unchecked("some address");
        assert!(TimeAlarms::new(MockStorage::new())
            .try_add(&querier, &env, msg_sender, Timestamp::from_nanos(4))
            .is_ok());
    }

    #[test]
    fn try_add_alarm_in_the_past() {
        let mut mock_querier = MockQuerier::default();
        mock_querier.update_wasm(contract::testing::valid_contract_handler);
        let querier = QuerierWrapper::new(&mock_querier);

        let mut env = testing::mock_env();
        env.block.time = Timestamp::from_seconds(100);

        let msg_sender = Addr::unchecked("some address");
        TimeAlarms::new(MockStorage::new())
            .try_add(deps.querier, &env, msg_sender, Timestamp::from_nanos(4))
            .unwrap_err();
    }
}
