use crate::{
    batch::{Batch, Emit, Emitter},
    error::Error,
    message::Response as MessageResponse,
};
use sdk::cosmwasm_std::Addr;
use serde::Serialize;

pub type Id = u64;
pub type AlarmsCount = u32;

pub struct AlarmsDispatcher<M> {
    message: M,
    batch: Batch,
    emitter: Emitter,
}

const EVENT_KEY: &str = "receiver";

impl<M> AlarmsDispatcher<M>
where
    M: Serialize + Copy,
{
    pub fn new<T>(message: M, event_type: T) -> Self
    where
        T: Into<String>,
    {
        Self {
            message,
            batch: Default::default(),
            emitter: Emitter::of_type(event_type),
        }
    }

    pub fn send_to(mut self, receiver: Addr, reply_id: Id) -> Result<Self, Error> {
        self.emitter = self.emitter.emit(EVENT_KEY, receiver.clone());
        self.batch = self.batch.schedule_execute_wasm_reply_always_no_funds(
            receiver,
            &self.message,
            reply_id,
        )?;

        Ok(self)
    }

    pub fn nb_sent(&self) -> AlarmsCount {
        self.batch
            .len()
            .try_into()
            .expect("used with alarms less than or equal to AlarmsCount::MAX")
    }
}

impl<M> From<AlarmsDispatcher<M>> for MessageResponse {
    fn from(value: AlarmsDispatcher<M>) -> Self {
        MessageResponse::messages_with_events(value.batch, value.emitter)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{dispatcher::EVENT_KEY, response};

    use sdk::{
        cosmwasm_ext::Response as CwResponse,
        cosmwasm_std::{Addr, Event, ReplyOn},
    };

    const EVENT_TYPE: &str = "test_event";

    #[test]
    fn empty() {
        let alarms = AlarmsDispatcher::new(3, EVENT_TYPE);
        assert_eq!(alarms.nb_sent(), 0);
        let d: CwResponse = response::response_only_messages(alarms);
        assert!(!d.events.is_empty());
        assert_eq!(Event::new(EVENT_TYPE), d.events[0]);
        assert!(d.messages.is_empty());
    }

    #[test]
    fn one_alarm() {
        let d = AlarmsDispatcher::new(1, EVENT_TYPE);
        assert_eq!(d.nb_sent(), 0);
        let receiver = Addr::unchecked("time_alarm receiver");

        let d = d.send_to(receiver.clone(), Id::MAX).unwrap();
        assert_eq!(d.nb_sent(), 1);

        let r: CwResponse = response::response_only_messages(d);
        assert!(!r.events.is_empty());
        assert_eq!(
            Event::new(EVENT_TYPE).add_attribute(EVENT_KEY, receiver),
            r.events[0]
        );

        assert!(!r.messages.is_empty());
        let msg = &r.messages[0];
        assert_eq!(ReplyOn::Always, msg.reply_on);
    }
}
