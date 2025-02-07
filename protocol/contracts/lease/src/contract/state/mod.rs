use std::str;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

use finance::duration::Duration;
use platform::{batch::Batch, message::Response as MessageResponse};
use sdk::{
    cosmwasm_std::{Binary, Env, MessageInfo, QuerierWrapper, Reply, Storage, Timestamp},
    cw_storage_plus::Item,
};
use swap::Impl;

use crate::{
    api::{
        open::NewLeaseContract,
        position::{ClosePolicyChange, PositionClose},
        query::StateResponse,
    },
    contract::api::Contract,
    error::ContractResult,
};

pub(crate) use self::handler::{Handler, Response};
use self::{dex::State as DexState, lease::State as LeaseState};

mod closed;
mod dex;
mod drain;
mod event;
mod handler;
mod lease;
mod liquidated;
mod opened;
mod opening;
mod paid;
mod resp_delivery;

type RequestLoan = LeaseState<opening::request_loan::RequestLoan>;

type BuyAsset = DexState<opening::buy_asset::DexState>;

type OpenedActive = LeaseState<opened::active::Active>;

type BuyLpn = DexState<opened::repay::buy_lpn::DexState>;

type PartialLiquidation = DexState<opened::close::liquidation::partial::DexState>;

type FullLiquidation = DexState<opened::close::liquidation::full::DexState>;

type PartialClose = DexState<opened::close::customer_close::partial::DexState>;

type FullClose = DexState<opened::close::customer_close::full::DexState>;

type PaidActive = LeaseState<paid::Active>;

type ClosingTransferIn = DexState<paid::transfer_in::DexState>;

type Closed = LeaseState<closed::Closed>;

type Liquidated = LeaseState<liquidated::Liquidated>;

type SwapResult = ContractResult<Response>;

type SwapClient = Impl;

#[enum_dispatch(Contract)]
#[derive(Serialize, Deserialize)]
pub(crate) enum State {
    RequestLoan,
    BuyAsset,
    OpenedActive,
    BuyLpn,
    PartialLiquidation,
    FullLiquidation,
    PartialClose,
    FullClose,
    PaidActive,
    ClosingTransferIn,
    Closed,
    Liquidated,
}

const STATE_DB_ITEM: Item<State> = Item::new("state");

pub(super) fn load(storage: &dyn Storage) -> ContractResult<State> {
    STATE_DB_ITEM.load(storage).map_err(Into::into)
}

pub(super) fn save(storage: &mut dyn Storage, next_state: &State) -> ContractResult<()> {
    STATE_DB_ITEM.save(storage, next_state).map_err(Into::into)
}

pub(super) fn new_lease(
    querier: QuerierWrapper<'_>,
    info: MessageInfo,
    spec: NewLeaseContract,
) -> ContractResult<(Batch, State)> {
    opening::request_loan::RequestLoan::new(querier, info, spec)
        .map(|(batch, start_state)| (batch, start_state.into()))
}

fn ignore_msg<S>(state: S) -> ContractResult<Response>
where
    S: Into<State>,
{
    Ok(Response::from(MessageResponse::default(), state))
}

mod impl_from {
    use super::{
        BuyAsset, BuyLpn, Closed, ClosingTransferIn, FullClose, FullLiquidation, Liquidated,
        OpenedActive, PaidActive, PartialClose, PartialLiquidation, RequestLoan, State,
    };

    impl From<super::opening::request_loan::RequestLoan> for State {
        fn from(value: super::opening::request_loan::RequestLoan) -> Self {
            RequestLoan::new(value).into()
        }
    }

    impl From<super::opening::buy_asset::DexState> for State {
        fn from(value: super::opening::buy_asset::DexState) -> Self {
            BuyAsset::new(value).into()
        }
    }

    impl From<super::opened::active::Active> for State {
        fn from(value: super::opened::active::Active) -> Self {
            OpenedActive::new(value).into()
        }
    }

    impl From<super::opened::repay::buy_lpn::DexState> for State {
        fn from(value: super::opened::repay::buy_lpn::DexState) -> Self {
            BuyLpn::new(value).into()
        }
    }

    impl From<super::opened::close::liquidation::partial::DexState> for State {
        fn from(value: super::opened::close::liquidation::partial::DexState) -> Self {
            PartialLiquidation::new(value).into()
        }
    }

    impl From<super::opened::close::liquidation::full::DexState> for State {
        fn from(value: super::opened::close::liquidation::full::DexState) -> Self {
            FullLiquidation::new(value).into()
        }
    }

    impl From<super::opened::close::customer_close::partial::DexState> for State {
        fn from(value: super::opened::close::customer_close::partial::DexState) -> Self {
            PartialClose::new(value).into()
        }
    }

    impl From<super::opened::close::customer_close::full::DexState> for State {
        fn from(value: super::opened::close::customer_close::full::DexState) -> Self {
            FullClose::new(value).into()
        }
    }

    impl From<super::paid::Active> for State {
        fn from(value: super::paid::Active) -> Self {
            PaidActive::new(value).into()
        }
    }

    impl From<super::paid::transfer_in::DexState> for State {
        fn from(value: super::paid::transfer_in::DexState) -> Self {
            ClosingTransferIn::new(value).into()
        }
    }

    impl From<super::closed::Closed> for State {
        fn from(value: super::closed::Closed) -> Self {
            Closed::new(value).into()
        }
    }

    impl From<super::liquidated::Liquidated> for State {
        fn from(value: super::liquidated::Liquidated) -> Self {
            Liquidated::new(value).into()
        }
    }
}
