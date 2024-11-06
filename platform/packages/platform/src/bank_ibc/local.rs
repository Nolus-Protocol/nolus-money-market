use currency::{platform::Nls, BankSymbols, Group};
use finance::coin::{Coin, CoinDTO};
use sdk::{
    cosmwasm_std::{Addr, Coin as CwCoin, Timestamp},
    neutron_sdk::{
        bindings::msg::{IbcFee, NeutronMsg},
        sudo::msg::RequestPacketTimeoutHeight,
    },
};

use crate::{batch::Batch, coin_legacy, ica::HostAccount, result::Result};

pub struct Sender<'c> {
    channel: &'c str,
    sender: Addr,
    receiver: HostAccount,
    timeout: Timestamp,
    ack_tip: CwCoin,
    timeout_tip: CwCoin,
    amounts: Vec<CwCoin>,
    memo: String,
}

impl<'c> Sender<'c> {
    pub fn new(
        channel: &'c str,
        sender: Addr,
        receiver: HostAccount,
        timeout: Timestamp,
        ack_tip: Coin<Nls>,
        timeout_tip: Coin<Nls>,
        memo: String,
    ) -> Self {
        Self {
            channel,
            sender,
            receiver,
            timeout,
            ack_tip: coin_legacy::to_cosmwasm_impl(ack_tip),
            timeout_tip: coin_legacy::to_cosmwasm_impl(timeout_tip),
            amounts: vec![],
            memo,
        }
    }

    pub fn send<G>(&mut self, amount: &CoinDTO<G>) -> Result<()>
    where
        G: Group,
    {
        coin_legacy::to_cosmwasm_on_network::<BankSymbols<G>>(amount).map(|coin| {
            self.amounts.push(coin);
        })
    }

    fn into_ibc_msgs(self) -> impl Iterator<Item = NeutronMsg> + 'c {
        let Self {
            channel,
            sender,
            receiver,
            timeout,
            ack_tip,
            timeout_tip,
            amounts,
            memo,
        } = self;

        amounts.into_iter().map(move |amount: CwCoin| {
            new_msg(
                channel,
                sender.clone(),
                receiver.clone(),
                amount,
                timeout,
                (ack_tip.clone(), timeout_tip.clone()),
                memo.clone(),
            )
        })
    }
}

const ICS20_PORT_AT_NOLUS: &str = "transfer";

fn new_msg(
    channel: &str,
    sender: Addr,
    receiver: HostAccount,
    amount: CwCoin,
    timeout: Timestamp,
    (ack_tip, timeout_tip): (CwCoin, CwCoin),
    memo: String,
) -> NeutronMsg {
    let timeout_height = RequestPacketTimeoutHeight {
        revision_height: None,
        revision_number: None,
    };
    NeutronMsg::IbcTransfer {
        source_port: ICS20_PORT_AT_NOLUS.into(),
        source_channel: channel.into(),
        token: amount,
        sender: sender.into(),
        receiver: receiver.into(),
        timeout_height,
        timeout_timestamp: timeout.nanos(),
        fee: IbcFee {
            recv_fee: vec![],
            ack_fee: vec![ack_tip],
            timeout_fee: vec![timeout_tip],
        },
        memo,
    }
}

impl<'c> From<Sender<'c>> for Batch {
    fn from(sender: Sender<'c>) -> Self {
        sender
            .into_ibc_msgs()
            .fold(Self::default(), Self::schedule_execute_no_reply)
    }
}

#[cfg(test)]
mod test {
    use currency::{
        platform::Nls,
        test::{SubGroupTestC10, SuperGroup, SuperGroupTestC1},
    };
    use finance::coin::Coin;
    use sdk::cosmwasm_std::{Addr, Timestamp};

    use crate::{
        bank_ibc::local::{new_msg, Sender},
        batch::Batch,
        coin_legacy::{self},
        ica::HostAccount,
    };

    #[test]
    fn send() {
        let channel = "channel-0";
        let sender = Addr::unchecked("sender");
        let receiver = HostAccount::try_from(String::from("receiver")).unwrap();
        let timeout = Timestamp::from_seconds(100);
        let ack_fee = Coin::<Nls>::new(100);
        let timeout_fee = Coin::<Nls>::new(50);
        let mut funds_sender = Sender::new(
            channel,
            sender.clone(),
            receiver.clone(),
            timeout,
            ack_fee,
            timeout_fee,
            "MEMO".into(),
        );

        let coin1: Coin<SubGroupTestC10> = 234214.into();
        let coin2: Coin<SuperGroupTestC1> = 234214.into();
        funds_sender.send::<SuperGroup>(&coin1.into()).unwrap();
        funds_sender.send::<SuperGroup>(&coin2.into()).unwrap();

        assert_eq!(Batch::from(funds_sender), {
            Batch::default()
                .schedule_execute_no_reply(new_msg(
                    channel,
                    sender.clone(),
                    receiver.clone(),
                    coin_legacy::to_cosmwasm_impl(coin1),
                    timeout,
                    (
                        coin_legacy::to_cosmwasm_impl(ack_fee),
                        coin_legacy::to_cosmwasm_impl(timeout_fee),
                    ),
                    "MEMO".into(),
                ))
                .schedule_execute_no_reply(new_msg(
                    channel,
                    sender,
                    receiver,
                    coin_legacy::to_cosmwasm_impl(coin2),
                    timeout,
                    (
                        coin_legacy::to_cosmwasm_impl(ack_fee),
                        coin_legacy::to_cosmwasm_impl(timeout_fee),
                    ),
                    "MEMO".into(),
                ))
        });
    }
}
