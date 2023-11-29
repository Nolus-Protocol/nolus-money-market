use currencies::Lpns;
use currency::{AnyVisitor, AnyVisitorResult, Currency, GroupVisit, Tickers};
use sdk::cosmwasm_std::DepsMut;

use crate::{
    error::ContractError,
    msg::{Config, SudoMsg},
    result::ContractResult,
    state::supported_pairs::SupportedPairs,
};

pub struct SudoWithOracleBase<'a> {
    deps: DepsMut<'a>,
    msg: SudoMsg,
}

impl<'a> SudoWithOracleBase<'a> {
    pub fn cmd(deps: DepsMut<'a>, msg: SudoMsg) -> ContractResult<<Self as AnyVisitor>::Output> {
        let visitor = Self { deps, msg };

        Config::load(visitor.deps.storage)
            .map_err(ContractError::LoadConfig)
            .and_then(|config: Config| Tickers.visit_any::<Lpns, _>(&config.base_asset, visitor))
    }
}

impl<'a> AnyVisitor for SudoWithOracleBase<'a> {
    type Output = ();
    type Error = ContractError;

    fn on<OracleBase>(self) -> AnyVisitorResult<Self>
    where
        OracleBase: Currency,
    {
        match self.msg {
            SudoMsg::SwapTree { tree } => SupportedPairs::<OracleBase>::new(tree.into_tree())?
                .validate_tickers()?
                .save(self.deps.storage),
            _ => unreachable!(),
        }
    }
}
