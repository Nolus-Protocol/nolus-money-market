use serde::{Deserialize, Serialize};

use crate::{
    ForwardToInner, SwapExactIn, SwapExactInPostRecoverIca, SwapExactInPreRecoverIca,
    SwapExactInRecoverIca, SwapExactInRecoverIcaRespDelivery, SwapExactInRespDelivery,
    TransferInFinish, TransferInInit, TransferInInitPostRecoverIca, TransferInInitPreRecoverIca,
    TransferInInitRecoverIca, TransferInInitRecoverIcaRespDelivery, TransferInInitRespDelivery,
    TransferOut, TransferOutRespDelivery,
};

use super::swap_task::SwapTask as SwapTaskT;

#[derive(Serialize, Deserialize)]
pub enum State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
where
    SwapTask: SwapTaskT,
{
    TransferOut(TransferOut<SwapTask, Self>),
    TransferOutRespDelivery(TransferOutRespDelivery<SwapTask, Self, ForwardToInnerMsg>),
    SwapExactIn(SwapExactIn<SwapTask, Self>),
    SwapExactInRespDelivery(SwapExactInRespDelivery<SwapTask, Self, ForwardToInnerMsg>),
    SwapExactInPreRecoverIca(SwapExactInPreRecoverIca<SwapTask, Self>),
    SwapExactInRecoverIca(SwapExactInRecoverIca<SwapTask, Self>),
    SwapExactInRecoverIcaRespDelivery(
        SwapExactInRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>,
    ),
    SwapExactInPostRecoverIca(SwapExactInPostRecoverIca<SwapTask, Self>),
    TransferInInit(TransferInInit<SwapTask, Self>),
    TransferInInitRespDelivery(TransferInInitRespDelivery<SwapTask, Self, ForwardToInnerMsg>),
    TransferInInitPreRecoverIca(TransferInInitPreRecoverIca<SwapTask, Self>),
    TransferInInitRecoverIca(TransferInInitRecoverIca<SwapTask, Self>),
    TransferInInitRecoverIcaRespDelivery(
        TransferInInitRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>,
    ),
    TransferInInitPostRecoverIca(TransferInInitPostRecoverIca<SwapTask, Self>),
    TransferInFinish(TransferInFinish<SwapTask, Self>),
}

pub type StartLocalLocalState<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> =
    TransferOut<SwapTask, State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>>;
pub type StartRemoteLocalState<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> =
    SwapExactIn<SwapTask, State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>>;
pub type StartTransferInState<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> =
    TransferInInit<SwapTask, State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>>;

pub fn start_local_local<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>(
    spec: SwapTask,
) -> StartLocalLocalState<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
where
    SwapTask: SwapTaskT,
    ForwardToInnerMsg: ForwardToInner,
{
    StartLocalLocalState::new(spec)
}

pub fn start_remote_local<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>(
    spec: SwapTask,
) -> StartRemoteLocalState<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
where
    SwapTask: SwapTaskT,
    ForwardToInnerMsg: ForwardToInner,
{
    StartRemoteLocalState::new(spec)
}

mod impl_into {
    use crate::{
        swap_task::SwapTask as SwapTaskT, ForwardToInner, SwapExactIn, SwapExactInPostRecoverIca,
        SwapExactInPreRecoverIca, SwapExactInRecoverIca, SwapExactInRecoverIcaRespDelivery,
        TransferInFinish, TransferInInit, TransferInInitPostRecoverIca,
        TransferInInitPreRecoverIca, TransferInInitRecoverIca,
        TransferInInitRecoverIcaRespDelivery, TransferInInitRespDelivery, TransferOut,
        TransferOutRespDelivery,
    };

    use super::{State, SwapExactInRespDelivery};

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> From<TransferOut<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferOut<SwapTask, Self>) -> Self {
            Self::TransferOut(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferOutRespDelivery<SwapTask, Self, ForwardToInnerMsg>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferOutRespDelivery<SwapTask, Self, ForwardToInnerMsg>) -> Self {
            Self::TransferOutRespDelivery(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> From<SwapExactIn<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
    {
        fn from(value: SwapExactIn<SwapTask, Self>) -> Self {
            Self::SwapExactIn(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<SwapExactInRespDelivery<SwapTask, Self, ForwardToInnerMsg>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
    {
        fn from(value: SwapExactInRespDelivery<SwapTask, Self, ForwardToInnerMsg>) -> Self {
            Self::SwapExactInRespDelivery(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<SwapExactInRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
    {
        fn from(
            value: SwapExactInRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>,
        ) -> Self {
            Self::SwapExactInRecoverIcaRespDelivery(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<SwapExactInPreRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: SwapExactInPreRecoverIca<SwapTask, Self>) -> Self {
            Self::SwapExactInPreRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<SwapExactInRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: SwapExactInRecoverIca<SwapTask, Self>) -> Self {
            Self::SwapExactInRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<SwapExactInPostRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: SwapExactInPostRecoverIca<SwapTask, Self>) -> Self {
            Self::SwapExactInPostRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInit<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInInit<SwapTask, Self>) -> Self {
            Self::TransferInInit(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInitRespDelivery<SwapTask, Self, ForwardToInnerMsg>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInInitRespDelivery<SwapTask, Self, ForwardToInnerMsg>) -> Self {
            Self::TransferInInitRespDelivery(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInitRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(
            value: TransferInInitRecoverIcaRespDelivery<SwapTask, Self, ForwardToInnerContinueMsg>,
        ) -> Self {
            Self::TransferInInitRecoverIcaRespDelivery(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInitPreRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInInitPreRecoverIca<SwapTask, Self>) -> Self {
            Self::TransferInInitPreRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInitRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInInitRecoverIca<SwapTask, Self>) -> Self {
            Self::TransferInInitRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInInitPostRecoverIca<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInInitPostRecoverIca<SwapTask, Self>) -> Self {
            Self::TransferInInitPostRecoverIca(value)
        }
    }

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        From<TransferInFinish<SwapTask, Self>>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn from(value: TransferInFinish<SwapTask, Self>) -> Self {
            Self::TransferInFinish(value)
        }
    }
}

mod impl_handler {
    use sdk::cosmwasm_std::{Binary, Deps, DepsMut, Env, Reply};

    use crate::{
        response::{ContinueResult, Result},
        swap_task::SwapTask as SwapTaskT,
        Handler,
    };

    use super::{ForwardToInner, State};

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> Handler
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        SwapTask::OutG: Clone,
        ForwardToInnerMsg: ForwardToInner,
        ForwardToInnerContinueMsg: ForwardToInner,
    {
        type Response = Self;
        type SwapResult = SwapTask::Result;

        fn on_open_ica(
            self,
            counterparty_version: String,
            deps: Deps<'_>,
            env: Env,
        ) -> ContinueResult<Self> {
            match self {
                State::TransferOut(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferOutRespDelivery(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::SwapExactIn(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::SwapExactInRespDelivery(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::SwapExactInRecoverIca(inner) => {
                    crate::forward_to_inner_ica::<_, ForwardToInnerContinueMsg, Self>(
                        inner,
                        counterparty_version,
                        env,
                    )
                }
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInInit(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInInitRespDelivery(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInInitRecoverIca(inner) => {
                    crate::forward_to_inner_ica::<_, ForwardToInnerContinueMsg, Self>(
                        inner,
                        counterparty_version,
                        env,
                    )
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
                State::TransferInFinish(inner) => {
                    Handler::on_open_ica(inner, counterparty_version, deps, env)
                }
            }
        }

        fn on_response(self, response: Binary, deps: Deps<'_>, env: Env) -> Result<Self> {
            match self {
                State::TransferOut(inner) => {
                    crate::forward_to_inner::<_, ForwardToInnerMsg, Self>(inner, response, env)
                }
                State::TransferOutRespDelivery(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::SwapExactIn(inner) => {
                    crate::forward_to_inner::<_, ForwardToInnerMsg, Self>(inner, response, env)
                }
                State::SwapExactInRespDelivery(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::SwapExactInRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInInit(inner) => {
                    crate::forward_to_inner::<_, ForwardToInnerMsg, Self>(inner, response, env)
                }

                State::TransferInInitRespDelivery(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInInitRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
                State::TransferInFinish(inner) => {
                    Handler::on_response(inner, response, deps, env).map_into()
                }
            }
        }

        fn on_error(self, deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
            match self {
                State::TransferOut(inner) => Handler::on_error(inner, deps, env),
                State::TransferOutRespDelivery(inner) => Handler::on_error(inner, deps, env),
                State::SwapExactIn(inner) => Handler::on_error(inner, deps, env),
                State::SwapExactInRespDelivery(inner) => Handler::on_error(inner, deps, env),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_error(inner, deps, env)
                }
                State::SwapExactInPreRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::SwapExactInRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::SwapExactInPostRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::TransferInInit(inner) => Handler::on_error(inner, deps, env),
                State::TransferInInitRespDelivery(inner) => Handler::on_error(inner, deps, env),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_error(inner, deps, env)
                }
                State::TransferInInitPreRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::TransferInInitRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::TransferInInitPostRecoverIca(inner) => Handler::on_error(inner, deps, env),
                State::TransferInFinish(inner) => Handler::on_error(inner, deps, env),
            }
        }

        fn on_timeout(self, deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
            match self {
                State::TransferOut(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferOutRespDelivery(inner) => Handler::on_timeout(inner, deps, env),
                State::SwapExactIn(inner) => Handler::on_timeout(inner, deps, env),
                State::SwapExactInRespDelivery(inner) => Handler::on_timeout(inner, deps, env),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_timeout(inner, deps, env)
                }
                State::SwapExactInPreRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::SwapExactInRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::SwapExactInPostRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInInit(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInInitRespDelivery(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_timeout(inner, deps, env)
                }
                State::TransferInInitPreRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInInitRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInInitPostRecoverIca(inner) => Handler::on_timeout(inner, deps, env),
                State::TransferInFinish(inner) => Handler::on_timeout(inner, deps, env),
            }
        }

        fn on_inner(self, deps: Deps<'_>, env: Env) -> Result<Self> {
            match self {
                State::TransferOut(inner) => Handler::on_inner(inner, deps, env).map_into(),
                State::TransferOutRespDelivery(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::SwapExactIn(inner) => Handler::on_inner(inner, deps, env).map_into(),
                State::SwapExactInRespDelivery(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::SwapExactInRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInInit(inner) => Handler::on_inner(inner, deps, env).map_into(),
                State::TransferInInitRespDelivery(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::on_inner(inner, deps, env).map_into()
                }
                State::TransferInFinish(inner) => Handler::on_inner(inner, deps, env).map_into(),
            }
        }

        fn on_inner_continue(self, deps: Deps<'_>, env: Env) -> ContinueResult<Self> {
            match self {
                State::TransferOut(inner) => Handler::on_inner_continue(inner, deps, env),
                State::TransferOutRespDelivery(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::SwapExactIn(inner) => Handler::on_inner_continue(inner, deps, env),
                State::SwapExactInRespDelivery(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::SwapExactInRecoverIca(inner) => Handler::on_inner_continue(inner, deps, env),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInInit(inner) => Handler::on_inner_continue(inner, deps, env),
                State::TransferInInitRespDelivery(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInInitRecoverIca(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::on_inner_continue(inner, deps, env)
                }
                State::TransferInFinish(inner) => Handler::on_inner_continue(inner, deps, env),
            }
        }

        fn heal(self, deps: Deps<'_>, env: Env) -> Result<Self> {
            match self {
                State::TransferOut(inner) => Handler::heal(inner, deps, env).map_into(),
                State::TransferOutRespDelivery(inner) => Handler::heal(inner, deps, env).map_into(),
                State::SwapExactIn(inner) => Handler::heal(inner, deps, env).map_into(),
                State::SwapExactInRespDelivery(inner) => Handler::heal(inner, deps, env).map_into(),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::SwapExactInRecoverIca(inner) => Handler::heal(inner, deps, env).map_into(),
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInInit(inner) => Handler::heal(inner, deps, env).map_into(),
                State::TransferInInitRespDelivery(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIca(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::heal(inner, deps, env).map_into()
                }
                State::TransferInFinish(inner) => Handler::heal(inner, deps, env).map_into(),
            }
        }

        fn reply(self, deps: &mut DepsMut<'_>, env: Env, msg: Reply) -> ContinueResult<Self> {
            match self {
                State::TransferOut(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferOutRespDelivery(inner) => Handler::reply(inner, deps, env, msg),
                State::SwapExactIn(inner) => Handler::reply(inner, deps, env, msg),
                State::SwapExactInRespDelivery(inner) => Handler::reply(inner, deps, env, msg),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::reply(inner, deps, env, msg)
                }
                State::SwapExactInPreRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::SwapExactInRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::SwapExactInPostRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInInit(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInInitRespDelivery(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::reply(inner, deps, env, msg)
                }
                State::TransferInInitPreRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInInitRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInInitPostRecoverIca(inner) => Handler::reply(inner, deps, env, msg),
                State::TransferInFinish(inner) => Handler::reply(inner, deps, env, msg),
            }
        }

        fn on_time_alarm(self, deps: Deps<'_>, env: Env) -> Result<Self> {
            match self {
                State::TransferOut(inner) => Handler::on_time_alarm(inner, deps, env).map_into(),
                State::TransferOutRespDelivery(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::SwapExactIn(inner) => Handler::on_time_alarm(inner, deps, env).map_into(),
                State::SwapExactInRespDelivery(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::SwapExactInPreRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::SwapExactInRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::SwapExactInPostRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInInit(inner) => Handler::on_time_alarm(inner, deps, env).map_into(),
                State::TransferInInitRespDelivery(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInInitPreRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInInitPostRecoverIca(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
                State::TransferInFinish(inner) => {
                    Handler::on_time_alarm(inner, deps, env).map_into()
                }
            }
        }
    }
}

mod impl_contract {
    use sdk::cosmwasm_std::{QuerierWrapper, Timestamp};

    use crate::{
        swap_task::SwapTask as SwapTaskT, Contract, ContractInSwap, ForwardToInner, SwapState,
        TransferInFinishState, TransferInInitState, TransferOutState,
    };

    use super::State;

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> Contract
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT
            + ContractInSwap<TransferOutState, <SwapTask as SwapTaskT>::StateResponse>
            + ContractInSwap<SwapState, <SwapTask as SwapTaskT>::StateResponse>
            + ContractInSwap<TransferInInitState, <SwapTask as SwapTaskT>::StateResponse>
            + ContractInSwap<TransferInFinishState, <SwapTask as SwapTaskT>::StateResponse>,
        ForwardToInnerMsg: ForwardToInner,
    {
        type StateResponse = SwapTask::StateResponse;

        fn state(self, now: Timestamp, querier: &QuerierWrapper<'_>) -> Self::StateResponse {
            match self {
                State::TransferOut(inner) => Contract::state(inner, now, querier),
                State::TransferOutRespDelivery(inner) => Contract::state(inner, now, querier),
                State::SwapExactIn(inner) => Contract::state(inner, now, querier),
                State::SwapExactInRespDelivery(inner) => Contract::state(inner, now, querier),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    Contract::state(inner, now, querier)
                }
                State::SwapExactInPreRecoverIca(inner) => Contract::state(inner, now, querier),
                State::SwapExactInRecoverIca(inner) => Contract::state(inner, now, querier),
                State::SwapExactInPostRecoverIca(inner) => Contract::state(inner, now, querier),
                State::TransferInInit(inner) => Contract::state(inner, now, querier),
                State::TransferInInitRespDelivery(inner) => Contract::state(inner, now, querier),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    Contract::state(inner, now, querier)
                }
                State::TransferInInitPreRecoverIca(inner) => Contract::state(inner, now, querier),
                State::TransferInInitRecoverIca(inner) => Contract::state(inner, now, querier),
                State::TransferInInitPostRecoverIca(inner) => Contract::state(inner, now, querier),
                State::TransferInFinish(inner) => Contract::state(inner, now, querier),
            }
        }
    }
}

mod impl_display {
    use std::fmt::Display;

    use super::State;
    use crate::swap_task::SwapTask as SwapTaskT;

    impl<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg> Display
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                State::TransferOut(inner) => Display::fmt(inner, f),
                State::TransferOutRespDelivery(inner) => Display::fmt(inner, f),
                State::SwapExactIn(inner) => Display::fmt(inner, f),
                State::SwapExactInRespDelivery(inner) => Display::fmt(inner, f),
                State::SwapExactInRecoverIcaRespDelivery(inner) => Display::fmt(inner, f),
                State::SwapExactInPreRecoverIca(inner) => Display::fmt(inner, f),
                State::SwapExactInRecoverIca(inner) => Display::fmt(inner, f),
                State::SwapExactInPostRecoverIca(inner) => Display::fmt(inner, f),
                State::TransferInInit(inner) => Display::fmt(inner, f),
                State::TransferInInitRespDelivery(inner) => Display::fmt(inner, f),
                State::TransferInInitRecoverIcaRespDelivery(inner) => Display::fmt(inner, f),
                State::TransferInInitPreRecoverIca(inner) => Display::fmt(inner, f),
                State::TransferInInitRecoverIca(inner) => Display::fmt(inner, f),
                State::TransferInInitPostRecoverIca(inner) => Display::fmt(inner, f),
                State::TransferInFinish(inner) => Display::fmt(inner, f),
            }
        }
    }
}

#[cfg(feature = "migration")]
mod impl_migration {

    use super::State;
    use crate::{
        migration::MigrateSpec, swap_task::SwapTask as SwapTaskT, ForwardToInner, InspectSpec,
    };

    impl<SwapTask, SwapTaskNew, SEnumNew, ForwardToInnerMsg, ForwardToInnerContinueMsg>
        MigrateSpec<SwapTask, SwapTaskNew, SEnumNew>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
        SwapTaskNew: SwapTaskT<OutG = SwapTask::OutG, Result = SwapTask::Result>,
    {
        type Out = State<SwapTaskNew, ForwardToInnerMsg, ForwardToInnerContinueMsg>;

        fn migrate_spec<MigrateFn>(self, migrate_fn: MigrateFn) -> Self::Out
        where
            MigrateFn: FnOnce(SwapTask) -> SwapTaskNew,
        {
            match self {
                State::TransferOut(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferOutRespDelivery(inner) => inner.migrate_spec(migrate_fn).into(),
                State::SwapExactIn(inner) => inner.migrate_spec(migrate_fn).into(),
                State::SwapExactInRespDelivery(inner) => inner.migrate_spec(migrate_fn).into(),
                State::SwapExactInRecoverIcaRespDelivery(inner) => {
                    inner.migrate_spec(migrate_fn).into()
                }
                State::SwapExactInPreRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::SwapExactInRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::SwapExactInPostRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInInit(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInInitRespDelivery(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    inner.migrate_spec(migrate_fn).into()
                }
                State::TransferInInitPreRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInInitRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInInitPostRecoverIca(inner) => inner.migrate_spec(migrate_fn).into(),
                State::TransferInFinish(inner) => inner.migrate_spec(migrate_fn).into(),
            }
        }
    }

    impl<SwapTask, R, ForwardToInnerMsg, ForwardToInnerContinueMsg> InspectSpec<SwapTask, R>
        for State<SwapTask, ForwardToInnerMsg, ForwardToInnerContinueMsg>
    where
        SwapTask: SwapTaskT,
        ForwardToInnerMsg: ForwardToInner,
    {
        fn inspect_spec<InspectFn>(&self, inspect_fn: InspectFn) -> R
        where
            InspectFn: FnOnce(&SwapTask) -> R,
        {
            match self {
                State::TransferOut(inner) => inner.inspect_spec(inspect_fn),
                State::TransferOutRespDelivery(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactIn(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactInRespDelivery(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactInRecoverIcaRespDelivery(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactInPreRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactInRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::SwapExactInPostRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInInit(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInInitRespDelivery(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInInitRecoverIcaRespDelivery(inner) => {
                    inner.inspect_spec(inspect_fn)
                }
                State::TransferInInitPreRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInInitRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInInitPostRecoverIca(inner) => inner.inspect_spec(inspect_fn),
                State::TransferInFinish(inner) => inner.inspect_spec(inspect_fn),
            }
        }
    }
}
