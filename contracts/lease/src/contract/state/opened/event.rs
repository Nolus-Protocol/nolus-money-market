use finance::liability::Level;
use platform::batch::{Emit, Emitter};
use sdk::cosmwasm_std::Env;

use crate::{
    api::DownpaymentCoin,
    contract::{
        cmd::{LiquidationDTO, OpenLoanRespResult, ReceiptDTO},
        state::event as state_event,
    },
    event::Type,
    lease::LeaseDTO,
};

pub(super) fn emit_lease_opened(
    env: &Env,
    lease: &LeaseDTO,
    loan: OpenLoanRespResult,
    downpayment: DownpaymentCoin,
) -> Emitter {
    Emitter::of_type(Type::OpenedActive)
        .emit_tx_info(env)
        .emit("id", &lease.addr)
        .emit("customer", lease.customer.clone())
        .emit_percent_amount(
            "air",
            loan.annual_interest_rate + lease.loan.annual_margin_interest(),
        )
        .emit("currency", lease.amount.ticker())
        .emit("loan-pool-id", lease.loan.lpp().addr())
        .emit_coin_dto("loan", &loan.principal)
        .emit_coin_dto("downpayment", &downpayment)
}

pub(super) fn emit_payment(env: &Env, lease: &LeaseDTO, receipt: &ReceiptDTO) -> Emitter {
    state_event::emit_payment_int(Type::PaidActive, env, &lease.addr, receipt)
}

pub(super) fn emit_liquidation_warning(lease: &LeaseDTO, level: &Level) -> Emitter {
    emit_lease(Emitter::of_type(Type::LiquidationWarning), lease)
        .emit_percent_amount("ltv", level.ltv())
        .emit_to_string_value("level", level.ordinal())
}

pub(super) fn emit_liquidation_start(lease: &LeaseDTO, liquidation: &LiquidationDTO) -> Emitter {
    let emitter = emit_lease(Emitter::of_type(Type::LiquidationStart), lease);
    state_event::emit_liquidation_info(emitter, liquidation.cause(), liquidation.amount(lease))
}

fn emit_lease(emitter: Emitter, lease: &LeaseDTO) -> Emitter {
    emitter
        .emit("customer", lease.customer.clone())
        .emit("lease", lease.addr.clone())
        .emit_currency_symbol("lease-asset", lease.amount.ticker())
}
