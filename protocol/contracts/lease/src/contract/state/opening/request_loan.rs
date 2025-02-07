use serde::{Deserialize, Serialize};

use finance::duration::Duration;
use platform::{
    batch::{Batch, Emit, Emitter},
    message::Response as MessageResponse,
    state_machine::Response as StateMachineResponse,
};
use sdk::cosmwasm_std::{Addr, Env, MessageInfo, QuerierWrapper, Reply, Timestamp};
use timealarms::stub::TimeAlarmsRef;

use crate::{
    api::{open::NewLeaseContract, query::StateResponse as QueryStateResponse, DownpaymentCoin},
    contract::{
        cmd::{OpenLoanReq, OpenLoanReqResult, OpenLoanResp},
        finalize::FinalizerRef,
        state::{Handler, Response},
    },
    error::ContractResult,
    event::Type,
    finance::{LppRef, OracleRef},
};

use super::buy_asset::DexState;

#[derive(Serialize, Deserialize)]
pub(crate) struct RequestLoan {
    new_lease: NewLeaseContract,
    downpayment: DownpaymentCoin,
    deps: (LppRef, OracleRef, TimeAlarmsRef, FinalizerRef),
}

impl RequestLoan {
    pub fn new(
        querier: QuerierWrapper<'_>,
        info: MessageInfo,
        spec: NewLeaseContract,
    ) -> ContractResult<(Batch, Self)> {
        let lpp = LppRef::try_new(spec.form.loan.lpp.clone(), querier)?;

        let oracle = OracleRef::try_from_base(spec.form.market_price_oracle.clone(), querier)?;

        let timealarms = TimeAlarmsRef::new(spec.form.time_alarms.clone(), querier)?;

        let finalizer = FinalizerRef::try_new(spec.finalizer.clone(), querier)?;

        let OpenLoanReqResult { batch, downpayment } = lpp.clone().execute_lender(
            OpenLoanReq::new(
                spec.form.position_spec,
                info.funds,
                spec.form.max_ltd,
                oracle.clone(),
                querier,
            ),
            querier,
        )?;
        Ok((batch, {
            Self {
                new_lease: spec,
                downpayment,
                deps: (lpp, oracle, timealarms, finalizer),
            }
        }))
    }

    fn on_response(
        self,
        querier: QuerierWrapper<'_>,
        env: Env,
        msg: Reply,
    ) -> ContractResult<Response> {
        let loan = self
            .deps
            .0
            .clone()
            .execute_lender(OpenLoanResp::new(msg), querier)?;

        let emitter = self.emit_ok(env.contract.address);

        let open_ica = super::buy_asset::start(
            self.new_lease,
            self.downpayment,
            loan,
            self.deps,
            env.block.time,
        );
        Ok(StateMachineResponse::from(
            MessageResponse::messages_with_events(open_ica.enter(), emitter),
            Into::<DexState>::into(open_ica),
        ))
    }

    fn emit_ok(&self, contract: Addr) -> Emitter {
        Emitter::of_type(Type::RequestLoan).emit("id", contract)
    }
}

impl Handler for RequestLoan {
    fn state(
        self,
        _now: Timestamp,
        _due_projection: Duration,
        _querier: QuerierWrapper<'_>,
    ) -> ContractResult<QueryStateResponse> {
        unreachable!()
    }

    fn reply(self, querier: QuerierWrapper<'_>, env: Env, msg: Reply) -> ContractResult<Response> {
        self.on_response(querier, env, msg)
    }
}
