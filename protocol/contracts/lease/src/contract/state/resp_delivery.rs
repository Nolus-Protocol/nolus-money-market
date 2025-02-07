use serde::{Deserialize, Serialize};

use dex::ForwardToInner;

use crate::api::ExecuteMsg;

#[derive(Serialize, Deserialize)]
pub(crate) struct ForwardToDexEntry {}

impl ForwardToInner for ForwardToDexEntry {
    type Msg = ExecuteMsg;

    fn msg() -> Self::Msg {
        ExecuteMsg::DexCallback()
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ForwardToDexEntryContinue {}
impl ForwardToInner for ForwardToDexEntryContinue {
    type Msg = ExecuteMsg;

    fn msg() -> Self::Msg {
        ExecuteMsg::DexCallbackContinue()
    }
}
