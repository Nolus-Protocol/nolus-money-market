use dex::ForwardToInner;
use serde::{Deserialize, Serialize};

use crate::msg::ExecuteMsg;

#[derive(Serialize, Deserialize)]
pub(crate) struct ForwardToDexEntry {}

impl ForwardToInner for ForwardToDexEntry {
    type Msg = ExecuteMsg;

    fn msg() -> Self::Msg {
        ExecuteMsg::DexCallback()
    }
}
