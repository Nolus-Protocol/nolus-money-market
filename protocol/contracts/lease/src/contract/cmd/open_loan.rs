use serde::{Deserialize, Serialize};

use currency::{CurrencyDef, MemberOf};
use finance::{
    coin::{Coin, WithCoin, WithCoinResult},
    percent::Percent,
};
use lpp::stub::lender::{LppLender as LppLenderTrait, WithLppLender};
use oracle::stub::convert;
use platform::{bank, batch::Batch};
use sdk::cosmwasm_std::{Coin as CwCoin, QuerierWrapper, Reply};

use crate::{
    api::{open::PositionSpecDTO, DownpaymentCoin, LeasePaymentCurrencies},
    error::ContractError,
    finance::{LpnCoin, LpnCoinDTO, LpnCurrencies, LpnCurrency, OracleRef},
    position::Spec as PositionSpec,
};

pub struct OpenLoanReq<'a> {
    position_spec: PositionSpecDTO,
    funds_in: Vec<CwCoin>,
    max_ltd: Option<Percent>,
    oracle: OracleRef,
    querier: QuerierWrapper<'a>,
}

impl<'a> OpenLoanReq<'a> {
    pub fn new(
        position_spec: PositionSpecDTO,
        funds_in: Vec<CwCoin>,
        max_ltd: Option<Percent>,
        oracle: OracleRef,
        querier: QuerierWrapper<'a>,
    ) -> Self {
        Self {
            position_spec,
            funds_in,
            max_ltd,
            oracle,
            querier,
        }
    }
}

impl<'a> WithLppLender<LpnCurrency, LpnCurrencies> for OpenLoanReq<'a> {
    type Output = OpenLoanReqResult;

    type Error = ContractError;

    fn exec<LppLender>(self, lpp: LppLender) -> Result<Self::Output, Self::Error>
    where
        LppLender: LppLenderTrait<LpnCurrency, LpnCurrencies>,
    {
        let (downpayment, downpayment_lpn) = bank::may_received(
            &self.funds_in,
            DownpaymentHandler {
                oracle: self.oracle,
                querier: self.querier,
            },
        )
        .ok_or_else(Self::Error::NoPaymentError)??;

        if downpayment_lpn.is_zero() {
            return Err(Self::Error::InsufficientPayment(downpayment));
        }

        PositionSpec::try_from(self.position_spec)
            .and_then(|spec| spec.calc_borrow_amount(downpayment_lpn, self.max_ltd))
            .and_then(|borrow_lpn| lpp.open_loan_req(borrow_lpn).map_err(ContractError::from))
            .map(|batch| Self::Output { batch, downpayment })
    }
}

struct DownpaymentHandler<'a> {
    oracle: OracleRef,
    querier: QuerierWrapper<'a>,
}
impl<'a> WithCoin<LeasePaymentCurrencies> for DownpaymentHandler<'a> {
    type Output = (DownpaymentCoin, LpnCoin);

    type Error = ContractError;

    fn on<C>(self, in_amount: Coin<C>) -> WithCoinResult<LeasePaymentCurrencies, Self>
    where
        C: CurrencyDef,
        C::Group: MemberOf<LeasePaymentCurrencies>,
    {
        let downpayment_lpn = convert::to_quote::<
            C,
            LeasePaymentCurrencies,
            LpnCurrency,
            LpnCurrencies,
        >(self.oracle, in_amount, self.querier)?;

        let downpayment = in_amount.into();

        Ok((downpayment, downpayment_lpn))
    }
}

pub struct OpenLoanReqResult {
    pub(in crate::contract) batch: Batch,
    pub(in crate::contract) downpayment: DownpaymentCoin,
}

pub struct OpenLoanResp {
    reply: Reply,
}

impl OpenLoanResp {
    pub fn new(reply: Reply) -> Self {
        Self { reply }
    }
}

impl WithLppLender<LpnCurrency, LpnCurrencies> for OpenLoanResp {
    type Output = OpenLoanRespResult;

    type Error = ContractError;

    fn exec<LppLender>(self, lpp: LppLender) -> Result<Self::Output, Self::Error>
    where
        LppLender: LppLenderTrait<LpnCurrency, LpnCurrencies>,
    {
        let loan_resp = lpp.open_loan_resp(self.reply)?;

        #[cfg(debug_assertions)]
        {
            use lpp::stub::LppBatch;

            let LppBatch { lpp_ref: _, batch } = lpp.into();

            debug_assert_eq!(Batch::default(), batch);
        }

        Ok(OpenLoanRespResult {
            principal: loan_resp.principal_due.into(),
            annual_interest_rate: loan_resp.annual_interest_rate,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub struct OpenLoanRespResult {
    pub(in crate::contract) principal: LpnCoinDTO,
    pub(in crate::contract) annual_interest_rate: Percent,
}
