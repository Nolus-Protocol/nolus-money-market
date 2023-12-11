use sdk::{
    cosmos_sdk_proto::{cosmos::base::abci::v1beta1::TxMsgData, traits::Message, Any},
    neutron_sdk::bindings::types::ProtobufAny,
};

use crate::result::Result;

#[derive(Default)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq))]
pub struct Transaction {
    msgs: Vec<ProtobufAny>,
}

impl Transaction {
    pub fn add_message<T, M>(&mut self, msg_type: T, msg: M)
    where
        T: Into<String>,
        M: Message,
    {
        let mut buf = Vec::with_capacity(msg.encoded_len());
        msg.encode_raw(&mut buf);

        self.msgs
            .push(ProtobufAny::new(msg_type.into(), buf.into()));
    }

    pub(super) fn into_msgs(self) -> Vec<ProtobufAny> {
        self.msgs
    }
}

pub fn decode_msg_responses(data: &[u8]) -> Result<impl Iterator<Item = Any>> {
    let tx_msg_data = TxMsgData::decode(data)?;
    let responses: Vec<Any> = if !tx_msg_data.msg_responses.is_empty() {
        tx_msg_data.msg_responses
    } else {
        #[allow(deprecated)]
        tx_msg_data
            .data
            .into_iter()
            .map(|msg| Any {
                type_url: msg.msg_type,
                value: msg.data,
            })
            .collect()
    };
    Ok(responses.into_iter())
}

#[cfg(feature = "testing")]
pub fn encode_msg_responses<I>(msgs: I) -> Vec<u8>
where
    I: Iterator<Item = Any>,
{
    let tx = TxMsgData {
        msg_responses: msgs.collect(),
        ..Default::default()
    };
    tx.encode_to_vec()
}

pub fn decode_msg_response<T, M>(resp: Any, _msg_type: T) -> Result<M>
where
    T: Into<String>,
    M: Message + Default,
{
    // let msg_type = msg_type.into();

    // if resp.type_url != msg_type {
    //     return Err(Error::ProtobufInvalidType(msg_type, resp.type_url));
    // }
    M::decode(resp.value.as_slice()).map_err(Into::into)
}

#[cfg(test)]
mod test {
    use base64::{engine::general_purpose, Engine};

    #[test]
    fn decode_post_0_47_response() {
        // https://testnet.mintscan.io/osmosis-testnet/txs/544AF6D53D1E2C3414A56DA1B2FFD84C7CB35ECF0E6EAD414701D87F8E1DF59C
        const RESP: &str = "EkgKOS9vc21vc2lzLnBvb2xtYW5hZ2VyLnYxYmV0YTEuTXNnU3dhcEV4YWN0QW1vdW50SW5SZXNwb25zZRILCgkxODkwNDgzOTISSAo5L29zbW9zaXMucG9vbG1hbmFnZXIudjFiZXRhMS5Nc2dTd2FwRXhhY3RBbW91bnRJblJlc3BvbnNlEgsKCTE4ODkwNTYzMA==";
        decode_msg_responses(RESP);
    }

    #[test]
    fn decode_pre_0_47_response() {
        // https://www.mintscan.io/osmosis/tx/45E31BF8834AEF6512722D2D54F1910F017F829F340A05AE7490962D3F0F80DD?height=12626552
        const RESP: &str = "Cj8KMS9vc21vc2lzLnBvb2xtYW5hZ2VyLnYxYmV0YTEuTXNnU3dhcEV4YWN0QW1vdW50SW4SCgoINTMyNzU5MDcKPwoxL29zbW9zaXMucG9vbG1hbmFnZXIudjFiZXRhMS5Nc2dTd2FwRXhhY3RBbW91bnRJbhIKCggxMzMxODg4OA==";
        decode_msg_responses(RESP);
    }

    fn decode_msg_responses(resp_base64: &str) {
        let resp = general_purpose::STANDARD.decode(resp_base64).unwrap();
        let mut responses = super::decode_msg_responses(&resp).unwrap();
        assert!(dbg!(responses.next()).is_some());
        assert!(dbg!(responses.next()).is_some());
        assert!(responses.next().is_none());
    }
}
